use std::time::Duration;

use fred::clients::Pool as RedisPool;
use fred::interfaces::KeysInterface;
use fred::types::Expiration;
use fred::types::scan::Scanner;
use futures::StreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{SystemError, SystemResult};

const SCAN_COUNT: u32 = 100;

#[derive(Clone)]
pub struct MemoryCache {
    pool: RedisPool,
}

impl MemoryCache {
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &RedisPool {
        &self.pool
    }

    pub async fn exists(&self, key: &str) -> SystemResult<bool> {
        let count: i64 = self.pool.exists(key).await?;
        Ok(count > 0)
    }

    pub async fn get(&self, key: &str) -> SystemResult<Option<String>> {
        let value: Option<String> = self.pool.get(key).await?;
        Ok(value)
    }

    pub async fn set(&self, key: &str, value: &str, expire: Duration) -> SystemResult<()> {
        let secs = expire.as_secs() as i64;
        let expiration = if secs > 0 { Some(Expiration::EX(secs)) } else { None };
        let _: () = self
            .pool
            .set(key, value, expiration, None, false)
            .await?;
        Ok(())
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> SystemResult<Option<T>> {
        let Some(raw) = self.get(key).await? else {
            return Ok(None);
        };
        let value = serde_json::from_str(&raw)?;
        Ok(Some(value))
    }

    pub async fn set_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        expire: Duration,
    ) -> SystemResult<()> {
        let raw = serde_json::to_string(value)?;
        self.set(key, &raw, expire).await
    }

    pub async fn erase(&self, key: &str) -> SystemResult<()> {
        let _: i64 = self.pool.del(key).await?;
        Ok(())
    }

    pub async fn erase_all(&self, prefix: &str) -> SystemResult<()> {
        let pattern = format!("{prefix}*");
        let client = self.pool.next();
        let mut scanner = client.scan(pattern, Some(SCAN_COUNT), None);
        let mut keys: Vec<String> = Vec::new();
        while let Some(result) = scanner.next().await {
            let mut page = result.map_err(SystemError::Redis)?;
            if let Some(found) = page.take_results() {
                for k in found {
                    if let Some(s) = k.into_string() {
                        keys.push(s);
                    }
                }
            }
            page.next();
        }
        if !keys.is_empty() {
            let _: i64 = self.pool.del(keys).await?;
        }
        Ok(())
    }
}
