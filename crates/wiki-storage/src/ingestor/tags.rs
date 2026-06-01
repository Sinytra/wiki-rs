use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, de};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tracing::{debug, trace, warn};
use walkdir::WalkDir;
use wiki_db::query;
use wiki_domain::content::ResourceLocation;

use crate::error::StorageResult;
use crate::format::JSON_EXT;
use crate::ingestor::issues::FileIssues;
use crate::ingestor::{IngestContext, JsonSource, PreparationResult, SubIngestor, parse_json_path};

const ALLOWED_TYPES: &[&str] = &["item"];

pub const INGESTOR_MOD_TAGS: &str = "Tags";

#[derive(Debug, Clone)]
pub struct TagValue(pub String);

impl<'de> Deserialize<'de> for TagValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TagValueVisitor;

        impl<'de> Visitor<'de> for TagValueVisitor {
            type Value = TagValue;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a tag id string or an object with an 'id' field")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<TagValue, E> {
                Ok(TagValue(v.to_owned()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<TagValue, E> {
                Ok(TagValue(v))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<TagValue, A::Error> {
                #[derive(Deserialize)]
                struct ObjectForm {
                    id: String,
                }

                let obj = ObjectForm::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(TagValue(obj.id))
            }
        }

        deserializer.deserialize_any(TagValueVisitor)
    }
}

#[derive(Debug, Deserialize)]
struct TagFile {
    #[serde(default)]
    values: Vec<TagValue>,
}

#[derive(Default)]
pub struct TagsSubIngestor {
    tag_ids: BTreeSet<ResourceLocation>,
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
        let allowed_ns = [ResourceLocation::BUILTIN_NAMESPACES, &[modid]].concat();

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

                    let Some(id) = issues.loc_from_relative(&namespace, &type_dir, path) else {
                        continue;
                    };

                    let Some(parsed): Option<TagFile> =
                        parse_json_path("tag", path, &issues).map(JsonSource::value)
                    else {
                        continue;
                    };

                    self.tag_ids.insert(id.clone());

                    for TagValue(value_id) in &parsed.values {
                        if let Some(stripped) = value_id.strip_prefix('#') {
                            if let Some(loc) = issues.parse_resloc(stripped) {
                                self.tag_ids.insert(loc);
                            }
                        } else {
                            if let Some(loc) = issues.parse_resloc(value_id)
                                && loc.namespace == modid
                            {
                                result.items.insert(value_id.clone());
                            }
                        }

                        self.tag_entries
                            .entry(id.to_string())
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
            trace!(tag = %tag, "Registering tag");
            ctx.repo.add_project_tag(conn, &tag.to_string()).await?;
        }

        debug!("Registering tag entries");
        for (parent, values) in &self.tag_entries {
            for entry in values {
                if let Some(child) = entry.strip_prefix('#') {
                    ctx.repo.add_tag_tag_entry(conn, parent, child).await?;
                } else {
                    ctx.repo.add_tag_item_entry(conn, parent, entry).await?;
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
