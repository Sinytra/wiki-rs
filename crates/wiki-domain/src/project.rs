use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use crate::content::{GameRecipeType, ResolvedGameRecipe, ResolvedItem, ResourceLocation};
use crate::error::DomainResult;
use crate::pagination::{PaginatedData, TableQueryParams};
use crate::response::{ProjectInfo, ProjectVersionData};
use async_trait::async_trait;
use sea_orm::prelude::StringLen;
use sea_orm::{DeriveActiveEnum, EnumIter};
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumString};
use crate::pages::links::ResolvedLink;
use crate::pages::metadata::Frontmatter;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, EnumString, AsRefStr, EnumIter, DeriveActiveEnum,
)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[sea_orm(
    rs_type = "String",
    db_type = "String(StringLen::N(255))",
    rename_all = "lowercase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ProjectType {
    Mod,
    ResourcePack,
    DataPack,
    Shader,
    ModPack,
    Plugin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum FileType {
    Dir,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FileTreeEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub path: String,
    pub r#type: FileType,
    pub children: Vec<FileTreeEntry>,
}

pub type FileTree = Vec<FileTreeEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ContentFileTreeEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub path: String,
    pub r#type: FileType,
    pub children: Vec<ContentFileTreeEntry>,
}

pub type ContentFileTree = Vec<ContentFileTreeEntry>;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectPage {
    pub frontmatter: Frontmatter,
    pub content: String,
    pub edit_url: Option<String>,
    pub properties: HashMap<String, HashMap<String, serde_json::Value>>,
    pub links: HashMap<String, ResolvedLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ItemContentPage {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FullItemData {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FullTagData {
    pub id: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct FullRecipeData {
    pub id: String,
    pub data: ResolvedGameRecipe,
}

pub type DynProject = Arc<dyn Project>;

#[async_trait]
pub trait Project: Send + Sync {
    fn id(&self) -> &str;

    fn locale(&self) -> &str;
    fn locales(&self) -> BTreeSet<String>;

    async fn available_versions(&self) -> DomainResult<HashMap<String, String>>;
    async fn has_version(&self, version: &str) -> DomainResult<bool>;

    // Pages
    async fn read_page(&self, path: &str) -> DomainResult<(ProjectPage, Frontmatter)>;
    async fn read_content_page(&self, p_ref: &str) -> DomainResult<ProjectPage>;

    // Game content
    async fn item_content_pages(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ItemContentPage>>;
    async fn tags(&self, params: TableQueryParams) -> DomainResult<PaginatedData<FullTagData>>;
    async fn tag_items(
        &self,
        tag: &str,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullItemData>>;
    async fn recipes(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<FullRecipeData>>;
    async fn versions(
        &self,
        params: TableQueryParams,
    ) -> DomainResult<PaginatedData<ProjectVersionData>>;

    async fn item_name(&self, loc: &str) -> DomainResult<FullItemData>;
    async fn read_lang_key(&self, namespace: &str, key: &str) -> DomainResult<Option<String>>;

    async fn recipe_type(
        &self,
        location: &ResourceLocation,
    ) -> DomainResult<Option<GameRecipeType>>;
    async fn recipe_type_workbenches(
        &self,
        location: &ResourceLocation,
    ) -> DomainResult<Vec<ResolvedItem>>;
    async fn recipe(&self, id: &str) -> DomainResult<Option<ResolvedGameRecipe>>;
    async fn recipes_for_page(&self, page_ref: &str) -> DomainResult<Vec<ResolvedGameRecipe>>;
    async fn obtainable_items_by(&self, page_ref: &str) -> DomainResult<Vec<ResolvedItem>>;

    // Info
    async fn project_info(&self) -> DomainResult<ProjectInfo>;

    // Files / assets
    async fn directory_tree(&self) -> DomainResult<FileTree>;
    async fn project_contents(&self) -> DomainResult<ContentFileTree>;
    fn asset(&self, location: &ResourceLocation) -> Option<PathBuf>;
}
