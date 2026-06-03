use std::borrow::Cow;

use serde::Serialize;
use typesense::Client;
use typesense::models::{
    CollectionSchema, DeleteDocumentsParameters, ImportDocumentsParameters, IndexAction,
};

use crate::error::{ExternalError, ExternalResult};

pub struct Typesense {
    client: Client,
}

impl Typesense {
    pub fn new(url: String, api_key: String) -> ExternalResult<Self> {
        let client = Client::builder()
            .api_key(api_key)
            .nodes([url])
            .build()
            .map_err(|e| ExternalError::Typesense(e.to_owned()))?;
        Ok(Self { client })
    }

    pub async fn ensure_collection(&self, schema: CollectionSchema<'_>) -> ExternalResult<()> {
        let name = schema.name.to_string();
        if self
            .client
            .collection_schemaless(&name)
            .retrieve()
            .await
            .is_ok()
        {
            tracing::debug!(collection = %name, "Typesense collection already exists");
            return Ok(());
        }

        tracing::info!(collection = %name, "Creating Typesense collection");
        self.client
            .collections()
            .create(schema)
            .await
            .map_err(|e| ExternalError::Typesense(e.to_string()))?;
        Ok(())
    }

    pub async fn import_upsert<T: Serialize>(
        &self,
        collection: &str,
        documents: &[T],
    ) -> ExternalResult<()> {
        if documents.is_empty() {
            return Ok(());
        }

        let mut jsonl = String::new();
        for doc in documents {
            jsonl.push_str(&serde_json::to_string(doc)?);
            jsonl.push('\n');
        }

        let params = ImportDocumentsParameters {
            action: Some(IndexAction::Upsert),
            ..Default::default()
        };
        let result = self
            .client
            .collection_schemaless(collection)
            .documents()
            .import_jsonl(jsonl, params)
            .await
            .map_err(|e| ExternalError::Typesense(e.to_string()))?;

        let failures = result
            .lines()
            .filter(|line| line.contains("\"success\":false"))
            .count();
        if failures > 0 {
            tracing::warn!(collection, failures, "Typesense import reported failures");
        }
        Ok(())
    }

    pub async fn delete_by_filter(&self, collection: &str, filter: &str) -> ExternalResult<i32> {
        let params = DeleteDocumentsParameters {
            ignore_not_found: Some(true),
            ..DeleteDocumentsParameters::new(Cow::Borrowed(filter))
        };
        let response = self
            .client
            .collection_schemaless(collection)
            .documents()
            .delete(params)
            .await
            .map_err(|e| ExternalError::Typesense(e.to_string()))?;
        Ok(response.num_deleted)
    }
}
