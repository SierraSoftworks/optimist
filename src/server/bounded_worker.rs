use std::{sync::Arc, time::Duration};

use thiserror::Error;
use tokio::sync::Semaphore;

use crate::project::ProjectError;

#[derive(Clone)]
pub(super) struct BoundedWorker {
    permits: Arc<Semaphore>,
    queue_timeout: Duration,
    execution_timeout: Duration,
}

#[derive(Debug, Error)]
pub(super) enum BoundedWorkerError {
    #[error("analysis capacity is currently busy")]
    Busy,
    #[error("analysis exceeded its execution deadline")]
    TimedOut,
    #[error("analysis worker stopped unexpectedly")]
    Failed,
    #[error(transparent)]
    Project(#[from] ProjectError),
}

impl BoundedWorker {
    pub(super) fn new(
        concurrency: usize,
        queue_timeout: Duration,
        execution_timeout: Duration,
    ) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(concurrency)),
            queue_timeout,
            execution_timeout,
        }
    }

    pub(super) async fn run<T, F>(&self, operation: F) -> Result<T, BoundedWorkerError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, ProjectError> + Send + 'static,
    {
        let permit = tokio::time::timeout(
            self.queue_timeout,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        .map_err(|_| BoundedWorkerError::Busy)?
        .map_err(|_| BoundedWorkerError::Failed)?;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation()
        });
        tokio::time::timeout(self.execution_timeout, task)
            .await
            .map_err(|_| BoundedWorkerError::TimedOut)?
            .map_err(|_| BoundedWorkerError::Failed)?
            .map_err(BoundedWorkerError::Project)
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::{BoundedWorker, BoundedWorkerError};

    fn worker() -> BoundedWorker {
        BoundedWorker::new(1, Duration::from_millis(20), Duration::from_millis(20))
    }

    #[tokio::test]
    async fn rejects_queued_work_when_capacity_is_busy() {
        let worker = Arc::new(worker());
        let (started, running_started) = tokio::sync::oneshot::channel();
        let running = {
            let worker = Arc::clone(&worker);
            tokio::spawn(async move {
                worker
                    .run(move || {
                        let _ = started.send(());
                        std::thread::sleep(Duration::from_millis(80));
                        Ok(())
                    })
                    .await
            })
        };
        running_started.await.unwrap();
        let queued = worker.run(|| Ok(())).await;
        assert!(matches!(queued, Err(BoundedWorkerError::Busy)));
        assert!(matches!(
            running.await.unwrap(),
            Err(BoundedWorkerError::TimedOut)
        ));
    }

    #[tokio::test]
    async fn returns_completed_work_and_project_errors() {
        assert_eq!(worker().run(|| Ok(42)).await.unwrap(), 42);
        let error = worker()
            .run::<(), _>(|| Err(crate::project::ProjectError::EmptyName))
            .await;
        assert!(matches!(
            error,
            Err(BoundedWorkerError::Project(
                crate::project::ProjectError::EmptyName
            ))
        ));
    }
}
