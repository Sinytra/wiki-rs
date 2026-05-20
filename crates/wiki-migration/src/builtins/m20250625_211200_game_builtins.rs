use std::collections::{HashMap, HashSet};
use sea_orm::entity::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sea_orm_migration::prelude::*;
use wiki_db::entity::prelude::*;
use wiki_db::entity::{
    item, project, project_item, project_version, recipe_type, recipe_workbench,
};
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

const WORKBENCHES: &[(&str, &[&str])] = &[
    (
        "minecraft:crafting_shaped",
        &["minecraft:crafting_table", "minecraft:crafter"],
    ),
    (
        "minecraft:crafting_shapeless",
        &["minecraft:crafting_table", "minecraft:crafter"],
    ),
    ("minecraft:smelting", &["minecraft:furnace"]),
    ("minecraft:blasting", &["minecraft:blast_furnace"]),
    (
        "minecraft:campfire_cooking",
        &["minecraft:campfire", "minecraft:soul_campfire"],
    ),
    ("minecraft:smoking", &["minecraft:smoker"]),
    ("minecraft:stonecutting", &["minecraft:stonecutter"]),
    (
        "minecraft:smithing_transform",
        &["minecraft:smithing_table"],
    ),
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
            project_id: Set("minecraft".to_owned()),
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
                loc: Set(Some((*loc).to_owned())),
                version_id: Set(Some(version.id)),
                ..Default::default()
            })
            .collect();
        RecipeType::insert_many(recipe_types).exec(db).await?;

        let types = RecipeType::find()
            .filter(recipe_type::Column::VersionId.eq(version.id))
            .filter(recipe_type::Column::Loc.is_in(RECIPE_TYPES.iter().copied()))
            .all(db)
            .await?;

        let mut unique_items: HashSet<String> = HashSet::new();
        for (_type_loc, item_locs) in WORKBENCHES {
            for &item in item_locs.iter() {
                unique_items.insert(item.to_owned());
            }
        }
        for item in unique_items {
            let db_item = item::ActiveModel {
                loc: Set(item),
                ..Default::default()
            };
            let inserted = Item::insert(db_item).exec_with_returning(db).await?;

            let proj_item = project_item::ActiveModel {
                item_id: Set(inserted.id),
                version_id: Set(version.id),
                ..Default::default()
            };
            ProjectItem::insert(proj_item).exec(db).await?;
        }

        let mut workbenches: Vec<recipe_workbench::ActiveModel> = Vec::new();
        for (type_loc, item_locs) in WORKBENCHES {
            let type_id = types
                .iter()
                .find(|t| t.loc.as_deref() == Some(*type_loc))
                .ok_or_else(|| DbErr::Custom(format!("recipe_type {type_loc} not found")))?
                .id;

            let item_ids: Vec<i64> = Item::find()
                .filter(item::Column::Loc.is_in(item_locs.iter().copied()))
                .all(db)
                .await?
                .into_iter()
                .map(|i| i.id)
                .collect();

            if item_ids.is_empty() {
                continue;
            }

            let project_items = ProjectItem::find()
                .filter(project_item::Column::ItemId.is_in(item_ids))
                .all(db)
                .await?;

            for pi in project_items {
                workbenches.push(recipe_workbench::ActiveModel {
                    type_id: Set(type_id),
                    item_id: Set(pi.id),
                });
            }
        }

        if !workbenches.is_empty() {
            RecipeWorkbench::insert_many(workbenches).exec(db).await?;
        }

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
