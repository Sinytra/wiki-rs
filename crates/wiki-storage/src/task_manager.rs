use crate::error::StorageError;
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

type SharedTask = Shared<BoxFuture<'static, Arc<dyn Any + Send + Sync>>>;

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

    pub async fn run_or_join<T, F, Fut>(&self, id: String, task_fn: F) -> Result<T, StorageError>
    where
        T: Send + Sync + Clone + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, StorageError>> + Send + 'static,
    {
        let shared = {
            let mut map = self.inner.lock().unwrap();

            if let Some(existing) = map.get(&id) {
                tracing::debug!(id = %id, "joining in-flight task");
                existing.clone()
            } else {
                tracing::debug!(id = %id, "starting new task");

                let handle = tokio::spawn(task_fn());

                let fut: BoxFuture<'static, Arc<dyn Any + Send + Sync>> = async move {
                    let result: Result<T, Arc<StorageError>> = match handle.await {
                        Ok(Ok(value)) => Ok(value),
                        Ok(Err(e)) => Err(Arc::new(e)),
                        Err(join_err) => {
                            Err(Arc::new(StorageError::TaskPanic(join_err.to_string())))
                        }
                    };
                    Arc::new(result) as Arc<dyn Any + Send + Sync>
                }
                .boxed();

                let shared = fut.shared();
                map.insert(id.clone(), shared.clone());
                shared
            }
        };

        let erased = shared.await;

        {
            let mut map = self.inner.lock().unwrap();
            if let Some(entry) = map.get(&id)
                && entry.peek().is_some()
            {
                map.remove(&id);
            }
        }

        let typed = erased
            .downcast_ref::<Result<T, Arc<StorageError>>>()
            .ok_or_else(|| {
                StorageError::Internal(format!(
                    "task '{id}' has different return type than requested"
                ))
            })?
            .clone();

        typed.map_err(|arc| {
            Arc::try_unwrap(arc).unwrap_or_else(|arc| StorageError::Internal(arc.to_string()))
        })
    }
}
