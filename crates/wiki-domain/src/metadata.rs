use std::collections::HashMap;

use garde::Validate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ProjectSchemaVersion {
    #[serde(rename = "0")]
    #[default]
    Legacy,
    #[serde(rename = "1")]
    V1
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectMetadataStub {
    #[serde(default)]
    pub schema: ProjectSchemaVersion,
}

// TODO per-format metadata parsing with exclusive fields
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[garde(allow_unvalidated)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectMetadata {
    #[serde(default)]
    pub schema: ProjectSchemaVersion,

    #[garde(length(min = 2, max = 126), pattern("^[a-z]+[a-z0-9-]+$"))]
    pub id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modid: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(custom(|_, _| check_cross_fields(&self)))]
    pub platforms: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<HashMap<String, String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(inner(custom(check_legacy_platform)))]
    pub platform: Option<String>, // TODO Deprecate, remove

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>, // TODO Deprecate, remove

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owners: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub licenses: Option<Licenses>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Licenses {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(dive)]
    pub project: Option<License>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[garde(allow_unvalidated)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct License {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(custom(|_, _| check_license_id_xor_name(&self.id, &self.name)))]
    pub id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("validate error: {0}")]
    Validate(String),
}

fn check_legacy_platform(value: &str, _: &()) -> garde::Result {
    if matches!(value, "curseforge" | "modrinth") {
        Ok(())
    } else {
        Err(garde::Error::new("must be 'curseforge' or 'modrinth'"))
    }
}

fn check_cross_fields(meta: &ProjectMetadata) -> garde::Result {
    let has_legacy = meta.platform.is_some() && meta.slug.is_some();
    let has_modern = meta.platforms.as_ref().is_some_and(|m| !m.is_empty());
    if !has_legacy && !has_modern {
        return Err(garde::Error::new(
            "platforms: either 'platforms' or both 'platform' and 'slug' must be provided",
        ));
    }
    Ok(())
}

fn check_license_id_xor_name(id: &Option<String>, name: &Option<String>) -> garde::Result {
    match (id.is_some(), name.is_some()) {
        (false, false) => Err(garde::Error::new("one of 'id' or 'name' is required")),
        _ => Ok(()),
    }
}
