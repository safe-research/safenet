//! Process-local background generation of FROST nonce chunks.

use crate::frost::{keygen::KeyShare, preprocess::NonceChunk};
use alloy::primitives::B256;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex as AsyncMutex, Notify, OwnedMutexGuard, oneshot};

/// A process-local producer of nonce chunks, with one stream per group.
///
/// Each stream eagerly computes one chunk, waits for a consumer to request it,
/// and starts computing its successor immediately after delivering it.
pub(super) struct NonceGenerator {
    groups: Mutex<BTreeMap<B256, Arc<GroupGenerator>>>,
}

impl NonceGenerator {
    /// Creates an empty nonce generator.
    pub(super) fn new() -> Self {
        Self {
            groups: Mutex::new(BTreeMap::new()),
        }
    }

    /// Starts a background nonce stream for `group_id`, if one is not already
    /// running.
    ///
    /// Requests are allowed to arrive before this method. The waiting request
    /// is notified as soon as the worker has been installed.
    pub(super) fn start(&self, group_id: B256, key_share: Arc<KeyShare>) -> Result<(), Error> {
        let mut groups = lock(&self.groups);
        let group = groups
            .entry(group_id)
            .or_insert_with(|| Arc::new(GroupGenerator::new()))
            .clone();
        let mut worker = lock(&group.worker);

        if matches!(*worker, Worker::Running(_)) {
            return Ok(());
        }

        let (requests, receiver) = mpsc::channel();
        let result = thread::Builder::new()
            .name("nonce-generator".into())
            .spawn(move || generate(group_id, key_share, receiver));
        match result {
            Ok(_) => {
                *worker = Worker::Running(requests);
                group.changed.notify_waiters();
                Ok(())
            }
            Err(err) => {
                *worker = Worker::Failed;
                group.changed.notify_waiters();
                Err(Error::Spawn(err))
            }
        }
    }

    /// Takes the next generated chunk for `group_id`.
    ///
    /// Only one request per group may be outstanding. Concurrent duplicate
    /// requests return `Ok(None)` instead of waiting for another chunk.
    pub(super) async fn next(&self, group_id: B256) -> Result<Option<GeneratedNonceChunk>, Error> {
        let group = {
            let mut groups = lock(&self.groups);
            groups
                .entry(group_id)
                .or_insert_with(|| Arc::new(GroupGenerator::new()))
                .clone()
        };
        let Ok(request) = Arc::clone(&group.request).try_lock_owned() else {
            return Ok(None);
        };

        let requests = loop {
            // Register for the notification before inspecting the state so a
            // concurrent start cannot be missed between the check and await.
            let changed = group.changed.notified();
            let state = match &*lock(&group.worker) {
                Worker::Pending => WorkerStatus::Pending,
                Worker::Running(requests) => WorkerStatus::Running(requests.clone()),
                Worker::Failed | Worker::Stopped => WorkerStatus::Unavailable,
            };
            match state {
                WorkerStatus::Pending => changed.await,
                WorkerStatus::Running(requests) => break requests,
                WorkerStatus::Unavailable => return Err(Error::Unavailable(group_id)),
            }
        };

        let (respond, response) = oneshot::channel();
        if requests.send(respond).is_err() {
            *lock(&group.worker) = Worker::Failed;
            group.changed.notify_waiters();
            return Err(Error::Unavailable(group_id));
        }
        let chunk = response.await.map_err(|_| Error::Unavailable(group_id))?;

        Ok(Some(GeneratedNonceChunk { chunk, request }))
    }

    /// Stops the background stream for `group_id` and waits for an accepted
    /// request to finish before returning.
    pub(super) async fn stop(&self, group_id: B256) {
        let group = lock(&self.groups).remove(&group_id);
        if let Some(group) = group {
            *lock(&group.worker) = Worker::Stopped;
            group.changed.notify_waiters();

            // A generated chunk keeps this lease until it has been persisted.
            // Waiting here ensures group pruning cannot race that write.
            let _request = group.request.clone().lock_owned().await;
        }
    }
}

/// A generated chunk and the lease debouncing other requests for its group.
pub(super) struct GeneratedNonceChunk {
    chunk: NonceChunk,
    request: OwnedMutexGuard<()>,
}

impl GeneratedNonceChunk {
    /// Separates the generated material from its request lease. The caller must
    /// retain the lease until it finishes persisting the chunk.
    pub(super) fn into_parts(self) -> (NonceChunk, OwnedMutexGuard<()>) {
        (self.chunk, self.request)
    }
}

struct GroupGenerator {
    worker: Mutex<Worker>,
    changed: Notify,
    request: Arc<AsyncMutex<()>>,
}

impl GroupGenerator {
    fn new() -> Self {
        Self {
            worker: Mutex::new(Worker::Pending),
            changed: Notify::new(),
            request: Arc::new(AsyncMutex::new(())),
        }
    }
}

enum Worker {
    Pending,
    Running(Sender<Request>),
    Failed,
    Stopped,
}

enum WorkerStatus {
    Pending,
    Running(Sender<Request>),
    Unavailable,
}

type Request = oneshot::Sender<NonceChunk>;

/// Generates one chunk ahead, then hands it to the next requester. If a
/// requester is cancelled, retain the chunk for the following request rather
/// than discarding already-generated secret material.
fn generate(group_id: B256, key_share: Arc<KeyShare>, requests: Receiver<Request>) {
    loop {
        let started = Instant::now();
        let mut chunk = loop {
            let result = NonceChunk::generate(&key_share, &mut rand::thread_rng());
            match result {
                Ok(chunk) => break chunk,
                Err(err) => {
                    tracing::warn!(%group_id, %err, "failed to generate nonce chunk; retrying");
                    thread::sleep(Duration::from_secs(1));
                }
            }
        };
        tracing::trace!(
            %group_id,
            elapsed_ms = started.elapsed().as_millis(),
            "generated nonce chunk"
        );

        loop {
            let Ok(request) = requests.recv() else {
                return;
            };
            match request.send(chunk) {
                Ok(()) => break,
                Err(returned) => chunk = returned,
            }
        }
    }
}

/// An error starting or communicating with a nonce worker.
#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    /// The operating system failed to start the background thread.
    #[error("failed to spawn nonce generator thread")]
    Spawn(#[source] std::io::Error),
    /// The group's worker stopped before fulfilling the request.
    #[error("nonce generator for group {0} is unavailable")]
    Unavailable(B256),
}

/// Acquires a lock, recovering its contents if another thread panicked while
/// holding it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUP: B256 = B256::repeat_byte(0xa1);

    #[tokio::test]
    async fn waits_for_start_debounces_and_streams_chunks() {
        let generator = NonceGenerator::new();

        // Requests and starts are separate effects, whose execution order is
        // intentionally unspecified. Poll the request first to exercise that
        // ordering and acquire its per-group lease.
        let mut first = Box::pin(generator.next(GROUP));
        tokio::select! {
            biased;
            _ = &mut first => panic!("request completed before its stream started"),
            () = tokio::task::yield_now() => {}
        }
        assert!(generator.next(GROUP).await.unwrap().is_none());

        generator.start(GROUP, Arc::new(KeyShare::dummy())).unwrap();
        let first = first.await.unwrap().unwrap();
        let (first, lease) = first.into_parts();
        let first_root = first.commitment;
        drop(lease);

        // Delivery immediately causes the worker to prepare a distinct next
        // chunk for the following request.
        let second = generator.next(GROUP).await.unwrap().unwrap();
        let (second, lease) = second.into_parts();
        assert_ne!(second.commitment, first_root);
        drop(lease);

        generator.stop(GROUP).await;
    }
}
