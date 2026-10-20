use garde::Validate;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use wiki_domain::metadata::ProjectMetadata;

use crate::json::{JsonError, parse_json};

pub const NO_ICON: &str = "_none";

#[derive(Debug, thiserror::Error)]
pub enum ProjectMetadataError {
    #[error(transparent)]
    Json(#[from] JsonError),
    #[error("validate error: {0}")]
    Validate(String),
}

pub fn parse_project_metadata(text: &str) -> Result<ProjectMetadata, ProjectMetadataError> {
    let metadata: ProjectMetadata = parse_json("project metadata", text)?;
    metadata
        .validate()
        .map_err(|e| ProjectMetadataError::Validate(e.to_string()))?;
    Ok(metadata)
}

#[derive(Debug, Default, Serialize)]
pub struct FolderMetadata {
    pub entries: Vec<FolderMetadataEntry>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderMetadataEntry {
    pub key: String,
    pub name: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawMetaValue {
    String(String),
    Object {
        #[serde(default)]
        name: Option<String>,
        #[serde(default, deserialize_with = "present")]
        icon: Option<Value>,
    },
}

fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

pub fn parse_folder_metadata(text: &str) -> Result<FolderMetadata, JsonError> {
    let raw: serde_json::Map<String, Value> = parse_json("folder metadata", text)?;
    let mut meta = FolderMetadata::default();

    for (key, val) in raw {
        let (name, icon) = match serde_json::from_value(val) {
            Ok(RawMetaValue::String(name)) => (Some(name), None),
            Ok(RawMetaValue::Object { name, icon }) => {
                let icon = match icon {
                    Some(Value::Null) => Some(NO_ICON.to_owned()),
                    Some(Value::String(s)) => Some(s),
                    _ => None,
                };
                (name, icon)
            }
            Err(_) => {
                meta.errors.push(format!(
                    "'{key}': expected a string or an object with 'name' and 'icon'"
                ));
                continue;
            }
        };

        meta.entries.push(FolderMetadataEntry {
            key,
            name: name.filter(|n| !n.is_empty()),
            icon,
        });
    }
    Ok(meta)
}
