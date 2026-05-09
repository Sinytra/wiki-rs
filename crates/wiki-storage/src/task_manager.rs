use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use futures::future::{BoxFuture, Shared};
use futures::FutureExt;

use crate::error::StorageError;

type TaskResult = Result<(), Arc<StorageError>>;
type SharedTask = Shared<BoxFuture<'static, TaskResult>>;

#[derive(Clone, Default)]
pub struct TaskManager {
    inner: Arc<Mutex<HashMap<String, SharedTask>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_pending(&self, id: &str) -> bool {
        let map = self.inner.lock().unwrap();
        map.contains_key(id)
    }

    pub async fn run_or_join<F, Fut>(&self, id: String, task_fn: F) -> Result<(), StorageError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), StorageError>> + Send + 'static,
    {
        let shared = {
            let mut map = self.inner.lock().unwrap();

            if let Some(existing) = map.get(&id) {
                tracing::info!(id = %id, "joining in-flight task");
                existing.clone()
            } else {
                tracing::info!(id = %id, "starting new task");

                let handle = tokio::spawn(task_fn());

                let fut: BoxFuture<'static, TaskResult> = async move {
                    match handle.await {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(e)) => Err(Arc::new(e)),
                        Err(join_err) => Err(Arc::new(StorageError::TaskPanic(join_err.to_string()))),
                    }
                }
                .boxed();

                let shared = fut.shared();
                map.insert(id.clone(), shared.clone());
                shared
            }
        };

        let result = shared.await;

        {
            let mut map = self.inner.lock().unwrap();
            if let Some(entry) = map.get(&id) {
                if entry.peek().is_some() {
                    map.remove(&id);
                }
            }
        }

        result.map_err(|arc| {
            Arc::try_unwrap(arc).unwrap_or_else(|arc| StorageError::Internal(arc.to_string()))
        })
    }
}
