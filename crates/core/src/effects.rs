//! Effect handling for state transitions.

use std::{
    convert::Infallible,
    future::{self, Future},
    marker::PhantomData,
    sync::Arc,
};
use tokio::task::JoinSet;

/// An effect handler that can be used for handling effects for a state
/// transition.
pub trait EffectHandler<Effect, Resume>: Send + Sync + 'static {
    /// Performs an effect and returns its result to resume a state machine.
    ///
    /// Note that this method explicitly does not fail, and is expected to return
    /// some result that indicates an error so the state machine can handle the
    /// effect error internally.
    ///
    /// The same effect may be performed more than once for the same chain
    /// message. For consumptive resources such as pre-committed nonces, handlers
    /// should encode outcomes like "already used" in `Resume`; state transitions
    /// remain pure because they consume only the resume value.
    fn perform_effect(&self, effect: Effect) -> impl Future<Output = Resume> + Send;
}

/// Executes effects concurrently and yields their resumes as they complete.
///
/// The manager owns all spawned effect tasks. Dropping it aborts any effects
/// that are still in progress.
pub struct EffectManager<Handler, Effect, Resume> {
    handler: Arc<Handler>,
    tasks: JoinSet<Resume>,
    effect: PhantomData<fn(Effect)>,
}

impl<Handler, Effect, Resume> EffectManager<Handler, Effect, Resume>
where
    Handler: EffectHandler<Effect, Resume>,
    Effect: Send + 'static,
    Resume: Send + 'static,
{
    /// Creates an effect manager for `handler`.
    pub fn new(handler: Handler) -> Self {
        Self {
            handler: Arc::new(handler),
            tasks: JoinSet::new(),
            effect: PhantomData,
        }
    }

    /// Spawns an effect task.
    pub fn spawn(&mut self, effect: Effect) {
        let handler = Arc::clone(&self.handler);
        self.tasks
            .spawn(async move { handler.perform_effect(effect).await });
    }

    /// Waits for and returns the next successfully completed effect.
    ///
    /// Task failures are logged and skipped. If no effect tasks are in
    /// progress, this remains pending until the caller cancels the future.
    ///
    /// # Cancel Safety
    ///
    /// This method is cancel safe. If `next` is used as the event in a
    /// `tokio::select!` statement and some other branch completes first, it is
    /// guaranteed that no effect tasks were consumed.
    pub async fn next(&mut self) -> Resume {
        loop {
            match self.tasks.join_next().await {
                Some(Ok(resume)) => return resume,
                Some(Err(err)) => tracing::error!(?err, "unexpected effect task failure"),
                // This may seem counter-intuitive, but we want to block forever
                // in case there are no pending effects to wait for, this
                // ensures that the calling `tokio::select!` will wait until
                // another branch is taken.
                None => future::pending().await,
            }
        }
    }
}

/// An effect handler for pure state machines without any side-effects.
pub struct Pure;

impl<Resume> EffectHandler<Infallible, Resume> for Pure {
    async fn perform_effect(&self, effect: Infallible) -> Resume {
        match effect {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{self, Duration, Instant};

    enum TestEffect {
        Complete { after: Duration, resume: usize },
        Count,
        Panic,
    }

    struct TestHandler {
        calls: AtomicUsize,
    }

    impl EffectHandler<TestEffect, usize> for TestHandler {
        async fn perform_effect(&self, effect: TestEffect) -> usize {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            match effect {
                TestEffect::Complete { after, resume } => {
                    time::sleep(after).await;
                    resume
                }
                TestEffect::Count => call,
                TestEffect::Panic => panic!("effect task panicked"),
            }
        }
    }

    fn manager() -> EffectManager<TestHandler, TestEffect, usize> {
        EffectManager::new(TestHandler {
            calls: AtomicUsize::new(0),
        })
    }

    #[tokio::test(start_paused = true)]
    async fn runs_effects_concurrently_and_yields_in_completion_order() {
        let mut manager = manager();
        let started = Instant::now();
        manager.spawn(TestEffect::Complete {
            after: Duration::from_secs(2),
            resume: 2,
        });
        manager.spawn(TestEffect::Complete {
            after: Duration::from_secs(1),
            resume: 1,
        });

        assert_eq!(manager.next().await, 1);
        assert_eq!(manager.next().await, 2);
        assert_eq!(started.elapsed(), Duration::from_secs(2));
    }

    #[tokio::test(start_paused = true)]
    async fn remains_pending_when_empty() {
        let mut manager = manager();

        let result = time::timeout(Duration::from_secs(1), manager.next()).await;

        assert!(result.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn next_is_cancel_safe() {
        let mut manager = manager();
        manager.spawn(TestEffect::Complete {
            after: Duration::from_secs(2),
            resume: 42,
        });

        tokio::select! {
            resume = manager.next() => panic!("effect completed unexpectedly: {resume}"),
            () = time::sleep(Duration::from_secs(1)) => {}
        }

        assert_eq!(manager.next().await, 42);
    }

    #[tokio::test]
    async fn shares_one_handler_between_tasks() {
        let mut manager = manager();
        manager.spawn(TestEffect::Count);
        manager.spawn(TestEffect::Count);

        let mut calls = [manager.next().await, manager.next().await];
        calls.sort_unstable();

        assert_eq!(calls, [1, 2]);
    }

    #[tokio::test]
    async fn accepts_more_effects_after_completing_all_tasks() {
        let mut manager = manager();
        manager.spawn(TestEffect::Complete {
            after: Duration::ZERO,
            resume: 1,
        });
        assert_eq!(manager.next().await, 1);

        manager.spawn(TestEffect::Complete {
            after: Duration::ZERO,
            resume: 2,
        });
        assert_eq!(manager.next().await, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn skips_panicked_tasks_without_losing_successful_resumes() {
        let mut manager = manager();
        manager.spawn(TestEffect::Panic);
        manager.spawn(TestEffect::Complete {
            after: Duration::from_secs(1),
            resume: 42,
        });

        assert_eq!(manager.next().await, 42);
    }
}
