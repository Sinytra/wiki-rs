use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

pub trait ContentFs {
    fn read_to_string(&self, path: &str) -> Option<String>;

    fn read_dir(&self, path: &str) -> Option<Vec<DirEntry>>;

    fn exists(&self, path: &str) -> bool;
}
