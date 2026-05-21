use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use serde::{Deserialize, Deserializer};
use tracing::{debug, trace, warn};
use walkdir::WalkDir;
use wiki_db::query;
use wiki_domain::content::ResourceLocation;
use wiki_domain::error::ProjectError;

use crate::error::StorageResult;
use crate::format::JSON_EXT;
use crate::ingestor::issues::FileIssues;
use crate::ingestor::{parse_json_path, IngestContext, PreparationResult, SubIngestor};

const ALLOWED_TYPES: &[&str] = &["item"];

pub const INGESTOR_MOD_TAGS: &str = "Tags"; 

// TODO Validation
#[derive(Debug, Clone)]
pub struct TagValue(pub String);

impl<'de> Deserialize<'de> for TagValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Str(String),
            Obj { id: String },
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Str(s) => TagValue(s),
            Raw::Obj { id } => TagValue(id),
        })
    }
}

#[derive(Debug, Deserialize)]
struct TagFile {
    #[serde(default)]
    values: Vec<TagValue>,
}

#[derive(Default)]
pub struct TagsSubIngestor {
    tag_ids: BTreeSet<String>,
    tag_entries: BTreeMap<String, BTreeSet<String>>,
}

#[async_trait]
impl SubIngestor for TagsSubIngestor {
    fn name(&self) -> &'static str {
        INGESTOR_MOD_TAGS
    }

    async fn prepare(&mut self, ctx: &IngestContext<'_>) -> StorageResult<PreparationResult> {
        let mut result = PreparationResult::default();

        let data_root = ctx.format.data_root();
        if !data_root.exists() {
            return Ok(result);
        }

        let modid = ctx.modid;
        let allowed_ns = [
            ResourceLocation::DEFAULT_NAMESPACE,
            ResourceLocation::COMMON_NAMESPACE,
            modid,
        ];

        // [namespace]
        for ns_entry in std::fs::read_dir(&data_root)? {
            let ns_entry = ns_entry?;
            if !ns_entry.file_type()?.is_dir() {
                continue;
            }
            let namespace = ns_entry.file_name().to_string_lossy().to_string();
            let tags_root = ns_entry.path().join("tags"); // TODO Use format
            if !tags_root.exists() {
                continue;
            }
            if !allowed_ns.contains(&namespace.as_str()) {
                warn!(namespace, "Skipping ignored tag namespace");
                continue;
            }

            // [namespace]/tags/[type]
            for type_entry in std::fs::read_dir(&tags_root)? {
                let type_entry = type_entry?;
                let type_name = type_entry.file_name().to_string_lossy().to_string();
                if !ALLOWED_TYPES.contains(&type_name.as_str()) {
                    warn!(r#type = %type_name, "Skipping ignored tag type");
                    continue;
                }
                let type_dir = type_entry.path();

                for f in WalkDir::new(&type_dir).into_iter().filter_map(Result::ok) {
                    if !f.file_type().is_file() {
                        continue;
                    }
                    if f.path().extension().and_then(|e| e.to_str()) != Some(JSON_EXT) {
                        continue;
                    }
                    let path = f.path();
                    let issues = FileIssues::new(&*ctx.issues, path.to_owned());

                    // TODO Extract function (loc_from_relative)?
                    let rel = path.strip_prefix(&type_dir).unwrap_or(path);
                    let rel_str = rel.to_string_lossy().to_string();
                    let stem = match rel_str.rfind('.') {
                        Some(i) => &rel_str[..i],
                        None => &rel_str,
                    };
                    let id = format!("{namespace}:{stem}");

                    if !ResourceLocation::validate(&id) {
                        issues.ingestor_error(ProjectError::InvalidResloc, id.clone());
                        continue;
                    }

                    let Some(parsed): Option<TagFile> = parse_json_path("tag", path, &issues)
                    else {
                        continue;
                    };

                    self.tag_ids.insert(id.clone());

                    for TagValue(value_id) in &parsed.values {
                        if let Some(stripped) = value_id.strip_prefix('#') {
                            self.tag_ids.insert(stripped.to_owned());
                        } else if let Some(loc) = ResourceLocation::parse(value_id)
                            && loc.namespace == modid
                        {
                            result.items.insert(value_id.clone());
                        }
                        self.tag_entries
                            .entry(id.clone())
                            .or_default()
                            .insert(value_id.clone());
                    }
                }
            }
        }

        Ok(result)
    }

    async fn execute(
        &mut self,
        ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        debug!(count = self.tag_ids.len(), "Registering tags");

        for tag in &self.tag_ids {
            if ResourceLocation::parse(tag).is_none() {
                continue;
            }
            trace!(tag, "Registering tag");
            query::ingestor::add_project_tag(conn, ctx.version_id, tag).await?;
        }

        debug!("Registering tag entries");
        for (parent, values) in &self.tag_entries {
            for entry in values {
                if let Some(child) = entry.strip_prefix('#') {
                    query::ingestor::add_tag_tag_entry(
                        conn,
                        ctx.version_id,
                        parent,
                        child,
                    )
                    .await?;
                } else {
                    query::ingestor::add_tag_item_entry(
                        conn,
                        ctx.version_id,
                        parent,
                        entry,
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }

    async fn finish(
        &mut self,
        _ctx: &IngestContext<'_>,
        conn: &DatabaseTransaction,
    ) -> StorageResult<()> {
        if !self.tag_ids.is_empty() {
            debug!("Refreshing flat tag->item view");
            query::ingestor::refresh_flat_tag_item_view(conn).await?;
        }
        Ok(())
    }
}
