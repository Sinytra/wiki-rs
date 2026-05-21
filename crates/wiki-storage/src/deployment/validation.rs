use crate::error::StorageResult;
use crate::format::ProjectFormat;
use std::collections::HashMap;
use std::path::Path;

pub struct ProjectSetupData {
    pub format: ProjectFormat,
    pub versions: HashMap<String, String>,
}

pub fn determine_project_type(root: &Path) -> StorageResult<ProjectSetupData> {
    // Currently only one format exists
    let format = ProjectFormat::new(root.to_owned());
    let metadata = format.read_metadata()?;

    Ok(ProjectSetupData {
        format,
        versions: metadata.versions.unwrap_or_default()
    })
}