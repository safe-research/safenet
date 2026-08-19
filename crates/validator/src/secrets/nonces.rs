//! Process-local background generation of FROST nonce chunks.

use crate::frost::{keygen::KeyShare, preprocess::NonceChunk};
use alloy::primitives::B256;
use rand::rngs::ThreadRng;
use std::{
    collections::{BTreeMap, btree_map},
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};
use tokio::sync::{Semaphore, oneshot};

/// A process-local producer of nonce chunks, with one stream per group.
///
/// Each stream eagerly computes one chunk, waits for a consumer to request it,
/// and starts computing its successor immediately after delivering it.
pub struct NonceGenerator {
    groups: BTreeMap<B256, NonceStream>,
}

impl NonceGenerator {
    /// Creates an empty nonce generator.
    pub fn new() -> Self {
        Self {
            groups: BTreeMap::new(),
        }
    }

    /// Starts a background nonce stream for `group_id`, if one is not already
    /// running.
    pub fn start(&mut self, group_id: B256, key_share: Arc<KeyShare>) -> Result<(), Error> {
        self.start_with_sampler(group_id, Sampler::Full(key_share))
    }

    fn start_with_sampler(&mut self, group_id: B256, sampler: Sampler) -> Result<(), Error> {
        let btree_map::Entry::Vacant(entry) = self.groups.entry(group_id) else {
            return Ok(());
        };

        tracing::debug!(%group_id, "starting nonce stream for group");
        let span = tracing::debug_span!("nonce_generator", %group_id);
        let stream = NonceStream::new(sampler, span)?;
        entry.insert(stream);
        Ok(())
    }

    /// Takes the next generated chunk for `group_id`.
    ///
    /// Only one request per group may be outstanding. Concurrent duplicate
    /// requests return `Ok(None)` instead of waiting for another chunk. Returns
    /// [`Error::Unavailable`] if the group's stream has not been started.
    pub fn next(
        &self,
        group_id: B256,
    ) -> impl Future<Output = Result<Option<NonceChunk>, Error>> + 'static {
        let next = self.groups.get(&group_id).map(|stream| stream.next());
        async move { next.ok_or(Error::Unavailable)?.await }
    }

    /// Stops the background stream of every group that does not match the
    /// `keep` predicate.
    ///
    /// Outstanding requests for the stopped groups fail with
    /// [`Error::Unavailable`].
    pub fn retain(&mut self, mut keep: impl FnMut(&B256) -> bool) {
        self.groups.retain(|group_id, _| {
            let keep = keep(group_id);
            if !keep {
                tracing::debug!(%group_id, "stopping nonce stream for group");
            }
            keep
        });
    }
}

/// Parameters for sampling random nonce chunks.
enum Sampler {
    /// Generate a full chunk of nonces using the specified key share.
    Full(Arc<KeyShare>),
    /// Custom function for generating a nonces chunk.
    ///
    /// Used only for unit testing.
    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    Custom(Box<dyn Fn(&mut ThreadRng) -> Result<NonceChunk, rand::Error> + Send + Sync + 'static>),
}

impl Sampler {
    fn nonces_chunk(&self, rng: &mut ThreadRng) -> Result<NonceChunk, rand::Error> {
        match self {
            Self::Full(key_share) => NonceChunk::generate(key_share, rng),
            #[cfg(test)]
            Self::Custom(generate) => generate(rng),
        }
    }
}

/// A stream of nonces for a given key share.
struct NonceStream {
    _worker: thread::JoinHandle<()>,
    pending: Arc<Semaphore>,
    requests: mpsc::Sender<oneshot::Sender<NonceChunk>>,
}

impl NonceStream {
    fn new(sampler: Sampler, span: tracing::Span) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .spawn(move || {
                let _guard = span.entered();
                Self::stream(sampler, receiver);
            })
            .map_err(Error::Spawn)?;

        Ok(Self {
            _worker: worker,
            pending: Arc::new(Semaphore::new(1)),
            requests: sender,
        })
    }

    fn stream(sampler: Sampler, requests: mpsc::Receiver<oneshot::Sender<NonceChunk>>) {
        let mut rng = rand::thread_rng();
        loop {
            let started = Instant::now();
            let nonces = match sampler.nonces_chunk(&mut rng) {
                Ok(nonces) => nonces,
                Err(err) => {
                    tracing::error!(?err, "unexpected error generating nonces; aborting");
                    break;
                }
            };
            tracing::trace!(
                elapsed_ms = started.elapsed().as_millis(),
                "completed nonce tree sampling effect"
            );

            if !Self::send_nonces(&requests, nonces) {
                break;
            }
        }
    }

    fn send_nonces(
        requests: &mpsc::Receiver<oneshot::Sender<NonceChunk>>,
        mut nonces: NonceChunk,
    ) -> bool {
        loop {
            let Ok(mut request) = requests.recv() else {
                tracing::trace!("nonce stream shut down");
                return false;
            };

            // The stream guarantees that we only ever have a single active
            // request at a time, meaning that only the last request from the
            // `requests` channel is active (i.e. the receiving end has not
            // been closed). Furthermore, multi-producer single-consumer
            // channels like `request` will only report disconnection after
            // all pending messages are handled. As such, we need to drain the
            // `requests` channel and only consider the last request to
            // correctly detect stream shutdown.
            loop {
                match requests.try_recv() {
                    Ok(new_request) => {
                        tracing::debug!("nonces future was canceled; using next request");
                        request = new_request;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // The `requests` queue is empty - this means that we
                        // finished draining it with `request` being the active
                        // one _and_ the sending end has not been dropped,
                        // indicating the nonces stream is still alive and we
                        // should send over our computed nonce chunk.
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        tracing::debug!("nonce stream closed mid-request; failing ongoing request");
                        return false;
                    }
                }
            }

            match request.send(nonces) {
                Ok(()) => return true,
                Err(unsent) => {
                    tracing::debug!("nonces future was canceled; waiting for next request");
                    nonces = unsent;
                }
            }
        }
    }

    fn next(&self) -> impl Future<Output = Result<Option<NonceChunk>, Error>> + 'static {
        let permit = self.pending.clone().try_acquire_owned().ok();
        let request = permit.is_some().then(|| {
            let (sender, receiver) = oneshot::channel();
            self.requests
                .send(sender)
                .map(|()| receiver)
                .map_err(|_| Error::Unavailable)
        });

        // We return an `async` block instead of making this an async function,
        // which allows us to express that the future does not capture `&self`
        // and continues to live past the reference. This is useful in our
        // context as it allows callers to immediately release any lock guarding
        // the `NonceGenerator` instead of holding it for as long as it takes to
        // generate the nonce chunk.
        async move {
            let _permit = permit;
            if let Some(receiver) = request.transpose()? {
                let nonces = receiver.await.map_err(|_| Error::Unavailable)?;
                Ok(Some(nonces))
            } else {
                Ok(None)
            }
        }
    }
}

/// An error starting or communicating with a nonce worker.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The operating system failed to start the background worker.
    #[error("failed to spawn nonce generator thread")]
    Spawn(#[source] std::io::Error),
    /// The group's worker stopped before fulfilling the request.
    #[error("nonce generator is unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use tokio::task::JoinSet;

    use super::*;
    use std::{assert_matches, sync::Barrier};

    #[tokio::test]
    async fn group_nonce_generation() {
        let mut generator = NonceGenerator::new();
        let group_id = B256::repeat_byte(0xa1);
        let key_share = Arc::new(KeyShare::dummy());

        // Requesting nonces from a not started group is an error.
        assert_matches!(generator.next(group_id).await, Err(Error::Unavailable));

        generator
            .start_with_sampler(
                group_id,
                Sampler::Custom(Box::new(move |rng| {
                    NonceChunk::with_size(4, &key_share, rng)
                })),
            )
            .unwrap();

        // Exactly one concurrent request produces a nonce chunk.
        let concurrent = {
            let mut set = JoinSet::new();
            for _ in 0..3 {
                set.spawn(generator.next(group_id));
            }
            set.join_all().await
        };
        let count = concurrent
            .into_iter()
            .filter_map(|result| result.unwrap())
            .count();
        assert_eq!(count, 1);

        // Can produce additional chunks after the first request completed.
        let more = generator.next(group_id).await.unwrap().is_some();
        assert!(more);

        // Stopping a group causes subsequent requests to fail.
        generator.retain(|_| false);
        assert_matches!(generator.next(group_id).await, Err(Error::Unavailable));
    }

    #[tokio::test]
    async fn cancel_then_rerequest() {
        let mut generator = NonceGenerator::new();
        let group_id = B256::repeat_byte(0xa1);
        let key_share = Arc::new(KeyShare::dummy());

        let barrier = Arc::new(Barrier::new(2));
        generator
            .start_with_sampler(
                group_id,
                Sampler::Custom(Box::new({
                    let barrier = barrier.clone();
                    move |rng| {
                        barrier.wait();
                        NonceChunk::with_size(4, &key_share, rng)
                    }
                })),
            )
            .unwrap();

        // Cancel a request while the worker is still generating its chunk, then
        // issue a fresh one.
        drop(generator.next(group_id));
        let next = generator.next(group_id);

        barrier.wait();
        assert_matches!(next.await, Ok(Some(_)));
    }

    #[tokio::test]
    async fn stops_ongoing_generations() {
        let mut generator = NonceGenerator::new();
        let group_id = B256::repeat_byte(0xa1);
        let key_share = Arc::new(KeyShare::dummy());

        let barrier = Arc::new(Barrier::new(2));
        generator
            .start_with_sampler(
                group_id,
                Sampler::Custom(Box::new({
                    let barrier = barrier.clone();
                    move |rng| {
                        barrier.wait();
                        NonceChunk::with_size(4, &key_share, rng)
                    }
                })),
            )
            .unwrap();

        // Start and poll a chunk request.
        let next = generator.next(group_id);
        tokio::pin!(next);
        tokio::select! {
            biased;
            _ = next.as_mut() => panic!("nonce chunk future should not complete"),
            _ = tokio::task::yield_now() => {},
        };

        // Stop the group.
        generator.retain(|_| false);

        // Simulate the ongoing nonce chunk generation completing, and ensure
        // that the nonce resolves to the chunk being unavailable as the group
        // was stopped.
        barrier.wait();
        assert_matches!(next.await, Err(Error::Unavailable));
    }
}
