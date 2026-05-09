use std::path::PathBuf;

use async_trait::async_trait;

use wiki_db::entity::{project, project_version};
use wiki_domain::ids::ProjectId;
use wiki_domain::project::Project;

pub struct LocalProject {
    id: ProjectId,
    record: project::Model,
    version: project_version::Model,
    checkout_path: PathBuf,
}

impl LocalProject {
    pub fn new(
        record: project::Model,
        version: project_version::Model,
        checkout_path: PathBuf,
    ) -> Self {
        let id = ProjectId::new(&record.id);
        Self {
            id,
            record,
            version,
            checkout_path,
        }
    }

    pub fn record(&self) -> &project::Model {
        &self.record
    }

    pub fn version(&self) -> &project_version::Model {
        &self.version
    }

    pub fn checkout_path(&self) -> &PathBuf {
        &self.checkout_path
    }
}

#[async_trait]
impl Project for LocalProject {
    fn id(&self) -> &ProjectId {
        &self.id
    }
}
