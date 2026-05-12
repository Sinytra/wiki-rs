#[allow(unused_imports)]
use std::collections::{BTreeSet, HashMap};
#[allow(unused_imports)]
use std::path::PathBuf;
use std::sync::Arc;

use crate::ids::ProjectId;
use async_trait::async_trait;
use sea_orm::{DeriveActiveEnum, EnumIter};
use sea_orm::prelude::StringLen;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumString};

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, AsRefStr,  EnumIter, DeriveActiveEnum)]
#[strum(serialize_all = "lowercase")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "lowercase"
)]
pub enum ProjectType {
    Mod,
    ResourcePack,
    DataPack,
    Shader,
    ModPack,
    Plugin,
    #[strum(disabled)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum FileType {
    Dir,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileTreeEntry {
    pub id: Option<String>,
    pub name: String,
    pub icon: Option<String>,
    pub path: String,
    pub r#type: FileType,
    pub children: Vec<FileTreeEntry>,
}

pub type FileTree = Vec<FileTreeEntry>;

#[derive(Debug, Clone)]
pub struct ProjectPage {
    pub content: String,
    pub edit_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Frontmatter {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemContentPage {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FullItemData {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FullTagData {
    pub id: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FullRecipeData {
    pub id: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemData {
    pub name: String,
    pub path: Option<String>,
}

pub type DynProject = Arc<dyn Project>;

#[async_trait]
pub trait Project: Send + Sync {
    fn id(&self) -> &ProjectId;
    // fn locale(&self) -> &str;
    // fn locales(&self) -> &BTreeSet<String>;
    //
    // async fn available_versions(&self) -> Result<HashMap<String, String>, DomainError>;
    // async fn has_version(&self, version: &str) -> Result<bool, DomainError>;
    //
    // // Pages
    // async fn read_page(&self, path: &str) -> Result<ProjectPage, DomainError>;
    // async fn read_content_page(&self, id: &str) -> Result<ProjectPage, DomainError>;
    // fn page_attributes(&self, path: &str) -> Option<Frontmatter>;
    // fn page_path(&self, path: &str) -> Option<String>;
    // fn page_title(&self, path: &str) -> Option<String>;
    //
    // // Game content
    // async fn item_content_pages(
    //     &self,
    //     params: TableQueryParams,
    // ) -> Result<PaginatedData<ItemContentPage>, DomainError>;
    // async fn tags(
    //     &self,
    //     params: TableQueryParams,
    // ) -> Result<PaginatedData<FullTagData>, DomainError>;
    // async fn tag_items(
    //     &self,
    //     tag: &str,
    //     params: TableQueryParams,
    // ) -> Result<PaginatedData<FullItemData>, DomainError>;
    // async fn recipes(
    //     &self,
    //     params: TableQueryParams,
    // ) -> Result<PaginatedData<FullRecipeData>, DomainError>;
    // async fn item_name(&self, loc: &str) -> Result<ItemData, DomainError>;
    // async fn read_item_properties(&self, id: &str) -> Result<serde_json::Value, DomainError>;
    // async fn read_lang_key(&self, namespace: &str, key: &str) -> Result<Option<String>, DomainError>;
    // async fn recipe_type(
    //     &self,
    //     location: &ResourceLocation,
    // ) -> Result<Option<GameRecipeType>, DomainError>;
    // async fn recipe(&self, id: &str) -> Result<Option<ResolvedGameRecipe>, DomainError>;
    //
    // // Files / assets
    // async fn directory_tree(&self) -> Result<FileTree, DomainError>;
    // async fn project_contents(&self) -> Result<FileTree, DomainError>;
    // fn asset(&self, location: &ResourceLocation) -> Option<PathBuf>;
    //
    // // Serialization to API JSON
    // async fn to_json(&self, full: bool) -> Result<serde_json::Value, DomainError>;
    // async fn to_json_verbose(&self) -> Result<serde_json::Value, DomainError>;
}
