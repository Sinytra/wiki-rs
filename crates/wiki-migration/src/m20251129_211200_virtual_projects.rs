use sea_orm::entity::*;
use sea_orm_migration::prelude::*;
use wiki_db::entity::project;
use wiki_db::entity::project_version;
use wiki_db::entity::prelude::*;
use wiki_domain::project::ProjectType;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let project = project::ActiveModel {
            id: Set("minecraft".to_owned()),
            name: Set("Minecraft".to_owned()),
            source_path: Set("".to_owned()),
            source_repo: Set("".to_owned()),
            source_branch: Set("".to_owned()),
            is_community: Set(false),
            r#type: Set(ProjectType::Mod),
            platforms: Set("{}".to_owned()),
            is_public: Set(false),
            is_virtual: Set(true),
            ..Default::default()
        };
        Project::insert(project).exec(db).await?;

        let project_ver = project_version::ActiveModel {
            project_id: Set("minecraft".to_owned()),
            name: NotSet,
            branch: Set("".to_owned()),
            ..Default::default()
        };
        ProjectVersion::insert(project_ver).exec(db).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        Project::delete_by_id("minecraft").exec(db).await?;

        Ok(())
    }
}
