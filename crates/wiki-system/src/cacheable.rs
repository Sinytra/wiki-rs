use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use futures::future::{BoxFuture, Shared};

// TODO Deduplicate with task_manager
pub struct TaskCoordinator<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + Send + 'static,
{
    inner: Arc<Mutex<HashMap<K, Shared<BoxFuture<'static, V>>>>>,
}

impl<K, V> Clone for TaskCoordinator<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + Send + 'static,
{
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<K, V> Default for TaskCoordinator<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> TaskCoordinator<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone + Send + 'static,
{
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub async fn run_or_join<F, Fut>(&self, key: K, task: F) -> V
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = V> + Send + 'static,
    {
        let shared = {
            let mut map = self.inner.lock().unwrap();
            if let Some(existing) = map.get(&key) {
                existing.clone()
            } else {
                let handle = tokio::spawn(task());
                let fut: BoxFuture<'static, V> = async move {
                    handle.await.expect("cacheable task panicked")
                }
                .boxed();
                let shared = fut.shared();
                map.insert(key.clone(), shared.clone());
                shared
            }
        };

        let result = shared.await;

        {
            let mut map = self.inner.lock().unwrap();
            if let Some(entry) = map.get(&key)
                && entry.peek().is_some()
            {
                map.remove(&key);
            }
        }

        result
    }
}
