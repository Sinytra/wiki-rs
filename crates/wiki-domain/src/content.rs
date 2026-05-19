use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ResourceLocation {
    pub namespace: String,
    pub path: String,
}

impl ResourceLocation {
    pub const DEFAULT_NAMESPACE: &'static str = "minecraft"; // TODO Deduplicate other declarations
    pub const COMMON_NAMESPACE: &'static str = "c";

    pub fn new(namespace: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            path: path.into(),
        }
    }

    pub fn minecraft(path: impl Into<String>) -> Self {
        Self::new(Self::DEFAULT_NAMESPACE, path)
    }

    pub fn parse(s: &str) -> Option<Self> {
        if !Self::validate(s) {
            return None;
        }
        let (namespace, path) = match s.split_once(':') {
            Some((ns, p)) => (ns.to_owned(), p.to_owned()),
            None => (Self::DEFAULT_NAMESPACE.to_owned(), s.to_owned()),
        };
        Some(Self { namespace, path })
    }

    pub fn validate(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let (namespace, path) = match s.split_once(':') {
            Some((ns, p)) => (ns, p),
            None => (Self::DEFAULT_NAMESPACE, s),
        };
        Self::is_valid_namespace(namespace) && Self::is_valid_path(path)
    }

    fn is_valid_namespace(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' || c == '.')
    }

    fn is_valid_path(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' || c == '.' || c == '/')
    }
}

impl fmt::Display for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ItemSlot {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")] // TODO Remove backwards compat
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct GameRecipeType {
    pub id: String,
    pub localized_name: Option<String>,
    pub background: String,
    pub input_slots: HashMap<String, ItemSlot>,
    pub output_slots: HashMap<String, ItemSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ResolvedItem {
    pub id: String,
    pub name: Option<String>,
    pub project: Option<String>,
    pub has_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ResolvedSlot {
    pub input: bool,
    pub slot: String,
    pub count: i32,
    pub items: Vec<ResolvedItem>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RecipeIngredientSummary {
    pub count: i32,
    pub item: ResolvedItem,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RecipeSummary {
    pub inputs: Vec<RecipeIngredientSummary>,
    pub outputs: Vec<RecipeIngredientSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ResolvedGameRecipe {
    pub id: String,
    pub r#type: String,
    pub inputs: Vec<ResolvedSlot>,
    pub outputs: Vec<ResolvedSlot>,
    pub summary: RecipeSummary,
}
