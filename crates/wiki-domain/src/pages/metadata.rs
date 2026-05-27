use crate::util::string_or_seq;
use serde::de::{MapAccess, Visitor};
use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum GameContentType {
    Block,
    Item,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFrontmatter {
    #[serde(default, deserialize_with = "string_or_seq")]
    pub id: Vec<String>,
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub infobox: Option<Infobox>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<GameContentType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Changelog>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Frontmatter {
    pub id: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infobox: Option<Infobox>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<GameContentType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Changelog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Infobox {
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<InfoboxTab>>,
    #[serde(default, deserialize_with = "string_or_seq")]
    pub inventory: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct InfoboxTab {
    pub name: String,
    pub display: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ChangelogEntry {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    pub changes: Vec<String>,
}

pub type Changelog = Vec<ChangelogEntry>;

impl From<RawFrontmatter> for Frontmatter {
    fn from(value: RawFrontmatter) -> Self {
        Self {
            id: value.id,
            title: value.title,
            r#type: value.r#type,
            custom: value.custom,
            infobox: value.infobox,
            history: value.history,
        }
    }
}

impl<'de> Deserialize<'de> for ChangelogEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ChangelogEntryVisitor;

        impl<'de> Visitor<'de> for ChangelogEntryVisitor {
            type Value = ChangelogEntry;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a changelog entry: either a full form with 'version' \
                / 'changes' or a short form mapping a version string to a single change")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ChangelogEntry, A::Error> {
                let value: serde_json::Value =
                    Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))?;

                if value.get("version").is_some() {
                    #[derive(Deserialize)]
                    struct FullForm {
                        version: String,
                        #[serde(default)]
                        date: Option<String>,
                        changes: Vec<String>,
                    }

                    let full: FullForm =
                        serde_json::from_value(value).map_err(de::Error::custom)?;
                    return Ok(ChangelogEntry {
                        version: full.version,
                        date: full.date,
                        changes: full.changes,
                    });
                }

                let obj = value
                    .as_object()
                    .ok_or_else(|| de::Error::custom("expected an object for changelog entry"))?;

                let mut iter = obj.iter();
                let (version, change) = iter
                    .next()
                    .ok_or_else(|| de::Error::custom("changelog entry is empty"))?;

                if iter.next().is_some() {
                    return Err(de::Error::custom(
                        "short-form changelog entry must have exactly one key/value pair",
                    ));
                }

                let change_str = change.as_str().ok_or_else(|| {
                    de::Error::custom(format!(
                        "short-form changelog value for '{version}' must be a string"
                    ))
                })?;

                Ok(ChangelogEntry {
                    version: version.clone(),
                    date: None,
                    changes: vec![change_str.to_owned()],
                })
            }
        }

        deserializer.deserialize_map(ChangelogEntryVisitor)
    }
}
