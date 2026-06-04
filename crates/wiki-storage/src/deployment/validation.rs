use crate::error::StorageResult;
use crate::format::{ProjectFormat, create_project_format};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct ProjectSetupData {
    pub format: Arc<dyn ProjectFormat>,
    pub versions: HashMap<String, String>,
}

pub fn determine_project_type(root: &Path) -> StorageResult<ProjectSetupData> {
    // Currently only one format exists
    let format = create_project_format(root.to_owned(), None);
    let metadata = format.read_metadata()?;

    Ok(ProjectSetupData {
        format,
        versions: metadata.versions.unwrap_or_default(),
    })
}
