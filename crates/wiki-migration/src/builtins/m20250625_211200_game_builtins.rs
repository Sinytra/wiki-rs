use sea_orm::entity::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sea_orm_migration::prelude::*;
use std::collections::HashMap;
use wiki_db::entity::prelude::*;
use wiki_db::entity::{project, project_version, recipe_type};
use wiki_domain::project::ProjectType;

#[derive(DeriveMigrationName)]
pub struct Migration;

const MC_PROJECT_ID: &str = "minecraft";

const RECIPE_TYPES: &[&str] = &[
    "minecraft:crafting_shaped",
    "minecraft:crafting_shapeless",
    "minecraft:smelting",
    "minecraft:blasting",
    "minecraft:campfire_cooking",
    "minecraft:smoking",
    "minecraft:stonecutting",
    "minecraft:smithing_transform",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let project = project::ActiveModel {
            id: Set(MC_PROJECT_ID.to_owned()),
            name: Set("Minecraft".to_owned()),
            source_path: Set("".to_owned()),
            source_repo: Set("".to_owned()),
            source_branch: Set("".to_owned()),
            is_community: Set(false),
            r#type: Set(ProjectType::Mod),
            platforms: Set(project::Platforms(HashMap::default())),
            is_public: Set(false),
            is_virtual: Set(true),
            ..Default::default()
        };
        Project::insert(project).exec(db).await?;

        let project_ver = project_version::ActiveModel {
            project_id: Set(MC_PROJECT_ID.to_owned()),
            name: NotSet,
            branch: Set("".to_owned()),
            ..Default::default()
        };
        let version = ProjectVersion::insert(project_ver)
            .exec_with_returning(db)
            .await?;

        let recipe_types: Vec<recipe_type::ActiveModel> = RECIPE_TYPES
            .iter()
            .map(|loc| recipe_type::ActiveModel {
                loc: Set((*loc).to_owned()),
                version_id: Set(version.id),
                ..Default::default()
            })
            .collect();
        RecipeType::insert_many(recipe_types).exec(db).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let Some(version) = ProjectVersion::find()
            .filter(project_version::Column::ProjectId.eq(MC_PROJECT_ID))
            .filter(project_version::Column::Name.is_null())
            .one(db)
            .await?
        else {
            return Ok(());
        };

        RecipeType::delete_many()
            .filter(recipe_type::Column::VersionId.eq(version.id))
            .filter(recipe_type::Column::Loc.is_in(RECIPE_TYPES.iter().copied()))
            .exec(db)
            .await?;

        Project::delete_by_id(MC_PROJECT_ID).exec(db).await?;

        Ok(())
    }
}
