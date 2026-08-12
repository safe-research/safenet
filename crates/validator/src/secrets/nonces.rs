//! Process-local background generation of FROST nonce chunks.

use crate::frost::{keygen::KeyShare, preprocess::NonceChunk};
use alloy::primitives::B256;
use rand::{CryptoRng, RngCore};
use std::{
    collections::{BTreeMap, btree_map},
    sync::{Arc, Mutex, PoisonError, mpsc},
    thread,
    time::Instant,
};
use tokio::sync::{Semaphore, oneshot};

/// A process-local producer of nonce chunks, with one stream per group.
///
/// Each stream eagerly computes one chunk, waits for a consumer to request it,
/// and starts computing its successor immediately after delivering it.
pub struct NonceGenerator {
    groups: Mutex<BTreeMap<B256, NonceStream>>,
}

impl NonceGenerator {
    /// Creates an empty nonce generator.
    pub fn new() -> Self {
        Self {
            groups: Mutex::new(BTreeMap::new()),
        }
    }

    /// Starts a background nonce stream for `group_id`, if one is not already
    /// running.
    pub fn start(&self, group_id: B256, key_share: Arc<KeyShare>) -> Result<(), Error> {
        self.start_with_sampler(group_id, Sampler::full(key_share))
    }

    fn start_with_sampler(&self, group_id: B256, sampler: Sampler) -> Result<(), Error> {
        let mut groups = self.groups.lock()?;
        let btree_map::Entry::Vacant(entry) = groups.entry(group_id) else {
            tracing::debug!(%group_id, "ignoring attempt to restart nonce generation worker for group");
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
    pub async fn next(&self, group_id: B256) -> Result<Option<NonceChunk>, Error> {
        let future = {
            let groups = self.groups.lock()?;
            let stream = groups.get(&group_id).ok_or(Error::Unavailable)?;
            stream.next()
        };
        future.await
    }

    /// Stops the background stream for `group_id`.
    ///
    /// An already accepted request may still receive its generated chunk while
    /// the worker drains the disconnected request channel.
    pub fn stop(&self, group_id: B256) {
        if let Ok(mut groups) = self.groups.lock() {
            groups.remove(&group_id);
        }
    }
}

/// Parameters for sampling random nonce chunks.
struct Sampler {
    key_share: Arc<KeyShare>,
    /// An optional size for the nonces chunk. Used only for unit testing.
    size: Option<u64>,
}

impl Sampler {
    fn full(key_share: Arc<KeyShare>) -> Self {
        Self {
            key_share,
            size: None,
        }
    }

    fn nonces_chunk<R>(&self, rng: &mut R) -> Result<NonceChunk, rand::Error>
    where
        R: RngCore + CryptoRng,
    {
        match self.size {
            None => NonceChunk::generate(&self.key_share, rng),
            Some(size) => NonceChunk::with_size(size, &self.key_share, rng),
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
                let _guard = span.enter();
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
        'outer: loop {
            let started = Instant::now();
            let mut nonces = match sampler.nonces_chunk(&mut rng) {
                Ok(nonces) => nonces,
                Err(err) => {
                    tracing::warn!(?err, "unexpected error generating nonces; aborting");
                    break;
                }
            };
            tracing::trace!(
                elapsed_ms = started.elapsed().as_millis(),
                "completed nonce tree sampling effect"
            );

            loop {
                let Ok(request) = requests.recv() else {
                    tracing::trace!("nonce stream shut down");
                    break 'outer;
                };
                match request.send(nonces) {
                    Ok(()) => continue 'outer,
                    Err(value) => {
                        tracing::debug!("nonces future was canceled; waiting for next request");
                        nonces = value;
                    }
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
        // context as it allows us to immediately release the `Mutex` lock held
        // for accessing the group-specific nonce stream in the
        // `NonceGenerator::groups` mapping.
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
    /// The nonce generator is poisoned.
    #[error("nonce generator is poisoned")]
    Poisoned,
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        Self::Poisoned
    }
}

#[cfg(test)]
mod tests {
    use tokio::task::JoinSet;

    use super::*;
    use std::assert_matches;

    #[tokio::test]
    async fn group_nonce_generation() {
        let generator = Arc::new(NonceGenerator::new());
        let group_id = B256::repeat_byte(0xa1);
        let key_share = Arc::new(KeyShare::dummy());

        // Requesting nonces from a not started group is an error.
        assert_matches!(generator.next(group_id).await, Err(Error::Unavailable));

        generator
            .start_with_sampler(
                group_id,
                Sampler {
                    key_share,
                    size: Some(4),
                },
            )
            .unwrap();

        // Exactly one concurrent request produces a nonce chunk.
        let concurrent = {
            let mut set = JoinSet::new();
            for _ in 0..3 {
                let generator = generator.clone();
                set.spawn(async move { generator.next(group_id).await.unwrap() });
            }
            set.join_all().await
        };
        let count = concurrent.into_iter().flatten().count();
        assert_eq!(count, 1);

        // Can produces additional chunks after the first request completed.
        let more = generator.next(group_id).await.unwrap().is_some();
        assert!(more);

        // Stopping unpolled requests fails the request.
        let pending = generator.next(group_id);
        generator.stop(group_id);
        assert_matches!(pending.await, Err(Error::Unavailable));

        // Subsequent requests for the stopped group error.
        assert_matches!(generator.next(group_id).await, Err(Error::Unavailable));
    }
}
