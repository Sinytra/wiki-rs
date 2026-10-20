use serde::{Deserialize, Serialize};

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
    /// Icon specified in folder metadata - for display in file tree
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub path: String,
    pub r#type: FileType,
    /// Icon specified in frontmatter - for display in search
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_icon: Option<String>,
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
    pub item_ids: Vec<String>,
    pub children: Vec<ContentFileTreeEntry>,
}

pub type ContentFileTree = Vec<ContentFileTreeEntry>;
