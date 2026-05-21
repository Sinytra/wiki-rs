use std::path::{Path, PathBuf};

use crate::error::StorageResult;

const LATEST_VERSION: &str = "latest";
const TEMP_DIR: &str = ".temp";

pub struct ProjectStore {
    base_path: PathBuf,
}

impl ProjectStore {
    pub fn new(base_path: PathBuf) -> StorageResult<Self> {
        if !base_path.exists() {
            std::fs::create_dir_all(&base_path)?;
        }
        Ok(Self { base_path })
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn deployment_root(&self, project_id: &str, deployment_id: &str) -> PathBuf {
        self.base_path.join(project_id).join(deployment_id)
    }

    pub fn deployment_versioned_path(
        &self,
        project_id: &str,
        deployment_id: &str,
        version: Option<&str>,
    ) -> PathBuf {
        let root = self.deployment_root(project_id, deployment_id);
        root.join(version.unwrap_or(LATEST_VERSION))
    }

    pub fn temp_clone_path(&self, project_id: &str, deployment_id: &str) -> PathBuf {
        let short_id = &deployment_id[..deployment_id.len().min(9)];
        self.base_path
            .join(TEMP_DIR)
            .join(format!("{project_id}-{short_id}"))
    }

    pub fn project_dir(&self, project_id: &str) -> PathBuf {
        self.base_path.join(project_id)
    }

    pub async fn remove_deployment(
        &self,
        project_id: &str,
        deployment_id: &str,
    ) -> StorageResult<()> {
        let path = self.deployment_root(project_id, deployment_id);
        if path.exists() {
            tokio::fs::remove_dir_all(&path).await?;
        }
        Ok(())
    }

    pub async fn remove_project(&self, project_id: &str) -> StorageResult<()> {
        let path = self.project_dir(project_id);
        if path.exists() {
            tokio::fs::remove_dir_all(&path).await?;
        }
        Ok(())
    }

    pub async fn remove_temp_clone(
        &self,
        project_id: &str,
        deployment_id: &str,
    ) -> StorageResult<()> {
        let path = self.temp_clone_path(project_id, deployment_id);
        if path.exists() {
            tokio::fs::remove_dir_all(&path).await?;
        }
        Ok(())
    }
}
