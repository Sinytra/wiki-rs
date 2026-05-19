use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Project {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProjectVersion {
    Table,
    Id,
    ProjectId,
    Name,
    Branch,
}

#[derive(DeriveIden)]
enum Item {
    Table,
    Id,
    Loc,
}

#[derive(DeriveIden)]
enum ProjectItem {
    Table,
    Id,
    ItemId,
    VersionId,
}

#[derive(DeriveIden)]
enum ProjectItemPage {
    Table,
    ItemId,
    Path,
}

#[derive(DeriveIden)]
enum Tag {
    Table,
    Id,
    Loc,
}

#[derive(DeriveIden)]
enum ProjectTag {
    Table,
    Id,
    TagId,
    VersionId,
}

#[derive(DeriveIden)]
enum TagItem {
    Table,
    TagId,
    ItemId,
}

#[derive(DeriveIden)]
enum TagTag {
    Table,
    Parent,
    Child,
}

#[derive(DeriveIden)]
enum RecipeType {
    Table,
    Id,
    Loc,
    VersionId,
}

#[derive(DeriveIden)]
enum Recipe {
    Table,
    Id,
    VersionId,
    Loc,
    TypeId,
}

#[derive(DeriveIden)]
enum RecipeIngredientTag {
    Table,
    RecipeId,
    TagId,
    Slot,
    Count,
    Input,
}

#[derive(DeriveIden)]
enum RecipeIngredientItem {
    Table,
    RecipeId,
    ItemId,
    Slot,
    Count,
    Input,
}

#[derive(DeriveIden)]
enum RecipeWorkbench {
    Table,
    TypeId,
    ItemId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectVersion::Table)
                    .col(big_pk_auto(ProjectVersion::Id))
                    .col(text(ProjectVersion::ProjectId))
                    .col(string_len_null(ProjectVersion::Name, 255))
                    .col(string_len(ProjectVersion::Branch, 255))
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectVersion::Table, ProjectVersion::ProjectId)
                            .to(Project::Table, Project::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("project_version_project_id_name_branch_key")
                    .table(ProjectVersion::Table)
                    .col(ProjectVersion::ProjectId)
                    .col(ProjectVersion::Name)
                    .col(ProjectVersion::Branch)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Item::Table)
                    .col(big_pk_auto(Item::Id))
                    .col(custom(Item::Loc, "resource_location").unique_key())
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared("ALTER TABLE item ADD CHECK (loc <> '')")
            .await
            .map(|_| ())?;

        manager
            .create_table(
                Table::create()
                    .table(ProjectItem::Table)
                    .col(big_pk_auto(ProjectItem::Id))
                    .col(big_integer(ProjectItem::ItemId))
                    .col(big_integer(ProjectItem::VersionId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectItem::Table, ProjectItem::ItemId)
                            .to(Item::Table, Item::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectItem::Table, ProjectItem::VersionId)
                            .to(ProjectVersion::Table, ProjectVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("project_item_item_id_version_id_key")
                    .table(ProjectItem::Table)
                    .col(ProjectItem::ItemId)
                    .col(ProjectItem::VersionId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProjectItemPage::Table)
                    .col(big_integer(ProjectItemPage::ItemId))
                    .col(text(ProjectItemPage::Path))
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectItemPage::Table, ProjectItemPage::ItemId)
                            .to(ProjectItem::Table, ProjectItem::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Tag::Table)
                    .col(big_pk_auto(Tag::Id))
                    .col(custom(Tag::Loc, "resource_location").unique_key())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ProjectTag::Table)
                    .col(big_pk_auto(ProjectTag::Id))
                    .col(big_integer(ProjectTag::TagId))
                    .col(big_integer_null(ProjectTag::VersionId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectTag::Table, ProjectTag::TagId)
                            .to(Tag::Table, Tag::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectTag::Table, ProjectTag::VersionId)
                            .to(ProjectVersion::Table, ProjectVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("project_tag_tag_id_version_id_key")
                    .table(ProjectTag::Table)
                    .col(ProjectTag::TagId)
                    .col(ProjectTag::VersionId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                "CREATE UNIQUE INDEX unique_tag_no_project ON project_tag (tag_id) WHERE version_id IS NULL",
            )
            .await
            .map(|_| ())?;

        manager
            .create_table(
                Table::create()
                    .table(TagItem::Table)
                    .col(big_integer(TagItem::TagId))
                    .col(big_integer(TagItem::ItemId))
                    .primary_key(
                        Index::create()
                            .col(TagItem::TagId)
                            .col(TagItem::ItemId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagItem::Table, TagItem::TagId)
                            .to(ProjectTag::Table, ProjectTag::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagItem::Table, TagItem::ItemId)
                            .to(ProjectItem::Table, ProjectItem::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TagTag::Table)
                    .col(big_integer(TagTag::Parent))
                    .col(big_integer(TagTag::Child))
                    .primary_key(
                        Index::create()
                            .col(TagTag::Parent)
                            .col(TagTag::Child),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagTag::Table, TagTag::Parent)
                            .to(ProjectTag::Table, ProjectTag::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(TagTag::Table, TagTag::Child)
                            .to(ProjectTag::Table, ProjectTag::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                r#"
ALTER TABLE tag_tag ADD CHECK (parent <> child);

CREATE FUNCTION tags_insert_trigger_func() RETURNS trigger AS
$BODY$
DECLARE
    results bigint;
BEGIN
    WITH RECURSIVE p(id) AS (SELECT parent
                             FROM tag_tag
                             WHERE child = NEW.parent
                             UNION
                             SELECT parent
                             FROM p,
                                  tag_tag d
                             WHERE p.id = d.child)
    SELECT *
    INTO results
    FROM p
    WHERE id = NEW.child;

    IF FOUND THEN
        RAISE EXCEPTION 'Circular dependencies are not allowed.';
    END IF;
    RETURN NEW;
END;
$BODY$ LANGUAGE plpgsql;

CREATE TRIGGER before_insert_tag_tag_trg
    BEFORE INSERT
    ON tag_tag
    FOR EACH ROW
EXECUTE PROCEDURE tags_insert_trigger_func();
"#,
            )
            .await
            .map(|_| ())?;

        manager
            .create_table(
                Table::create()
                    .table(RecipeType::Table)
                    .col(big_pk_auto(RecipeType::Id))
                    .col(custom_null(RecipeType::Loc, "resource_location"))
                    .col(big_integer_null(RecipeType::VersionId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(RecipeType::Table, RecipeType::VersionId)
                            .to(ProjectVersion::Table, ProjectVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("recipe_type_id_version_id_key")
                    .table(RecipeType::Table)
                    .col(RecipeType::Id)
                    .col(RecipeType::VersionId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                "CREATE UNIQUE INDEX unique_recipe_type_no_project ON recipe_type (loc) WHERE version_id IS NULL",
            )
            .await
            .map(|_| ())?;

        manager
            .create_table(
                Table::create()
                    .table(Recipe::Table)
                    .col(big_pk_auto(Recipe::Id))
                    .col(big_integer_null(Recipe::VersionId))
                    .col(custom(Recipe::Loc, "resource_location"))
                    .col(big_integer(Recipe::TypeId))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Recipe::Table, Recipe::VersionId)
                            .to(ProjectVersion::Table, ProjectVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Recipe::Table, Recipe::TypeId)
                            .to(RecipeType::Table, RecipeType::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("recipe_version_id_loc_key")
                    .table(Recipe::Table)
                    .col(Recipe::VersionId)
                    .col(Recipe::Loc)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                "CREATE UNIQUE INDEX unique_recipe_no_project ON recipe (loc) WHERE version_id IS NULL",
            )
            .await
            .map(|_| ())?;

        manager
            .create_table(
                Table::create()
                    .table(RecipeIngredientTag::Table)
                    .col(big_integer(RecipeIngredientTag::RecipeId))
                    .col(big_integer(RecipeIngredientTag::TagId))
                    .col(string_len(RecipeIngredientTag::Slot, 255))
                    .col(integer(RecipeIngredientTag::Count))
                    .col(boolean(RecipeIngredientTag::Input))
                    .primary_key(
                        Index::create()
                            .col(RecipeIngredientTag::RecipeId)
                            .col(RecipeIngredientTag::TagId)
                            .col(RecipeIngredientTag::Slot)
                            .col(RecipeIngredientTag::Input),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RecipeIngredientTag::Table, RecipeIngredientTag::RecipeId)
                            .to(Recipe::Table, Recipe::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RecipeIngredientTag::Table, RecipeIngredientTag::TagId)
                            .to(Tag::Table, Tag::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RecipeIngredientItem::Table)
                    .col(big_integer(RecipeIngredientItem::RecipeId))
                    .col(big_integer(RecipeIngredientItem::ItemId))
                    .col(string_len(RecipeIngredientItem::Slot, 255))
                    .col(integer(RecipeIngredientItem::Count))
                    .col(boolean(RecipeIngredientItem::Input))
                    .primary_key(
                        Index::create()
                            .col(RecipeIngredientItem::RecipeId)
                            .col(RecipeIngredientItem::ItemId)
                            .col(RecipeIngredientItem::Slot)
                            .col(RecipeIngredientItem::Input),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RecipeIngredientItem::Table, RecipeIngredientItem::RecipeId)
                            .to(Recipe::Table, Recipe::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RecipeIngredientItem::Table, RecipeIngredientItem::ItemId)
                            .to(Item::Table, Item::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(RecipeWorkbench::Table)
                    .col(big_integer(RecipeWorkbench::TypeId))
                    .col(big_integer(RecipeWorkbench::ItemId))
                    .primary_key(
                        Index::create()
                            .col(RecipeWorkbench::TypeId)
                            .col(RecipeWorkbench::ItemId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RecipeWorkbench::Table, RecipeWorkbench::TypeId)
                            .to(RecipeType::Table, RecipeType::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RecipeWorkbench::Table, RecipeWorkbench::ItemId)
                            .to(ProjectItem::Table, ProjectItem::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                r#"
CREATE MATERIALIZED VIEW tag_item_flat AS
SELECT *
FROM (WITH RECURSIVE tag_hierarchy AS (SELECT tp.id                            AS parent,
                                              tc.id                            AS child,
                                              array [ tp.id, tc.id ]::bigint[] AS track
                                       FROM tag_tag
                                                JOIN project_tag tp ON tag_tag.parent = tp.id
                                                JOIN project_tag tc ON tag_tag.child = tc.id
                                       UNION ALL
                                       SELECT tp.id AS parent, tc.id AS child, tag_hierarchy.track || tc.id
                                       FROM tag_tag n
                                                JOIN project_tag tp ON n.parent = tp.id
                                                JOIN project_tag tc ON n.child = tc.id
                                                JOIN tag_hierarchy ON tp.id = tag_hierarchy.child)
      SELECT th.parent AS parent, project_item.id AS child
      FROM tag_hierarchy th
               JOIN project_tag ON project_tag.id = th.child
               JOIN tag_item ti ON project_tag.id = ti.tag_id
               JOIN project_item ON project_item.id = ti.item_id

      UNION ALL

      SELECT project_tag.id AS parent, project_item.id AS child
      FROM tag_item
               JOIN project_tag on project_tag.id = tag_item.tag_id
               JOIN project_item ON project_item.id = tag_item.item_id) as subq;
"#,
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                "DROP MATERIALIZED VIEW IF EXISTS tag_item_flat;\n\
                 DROP TRIGGER IF EXISTS before_insert_tag_tag_trg ON tag_tag;\n\
                 DROP FUNCTION IF EXISTS tags_insert_trigger_func;",
            )
            .await
            .map(|_| ())?;

        for table in [
            "recipe_workbench",
            "recipe_ingredient_item",
            "recipe_ingredient_tag",
            "recipe",
            "recipe_type",
            "tag_tag",
            "tag_item",
            "project_tag",
            "tag",
            "project_item_page",
            "project_item",
            "item",
            "project_version",
        ] {
            manager
                .drop_table(Table::drop().table(table).if_exists().to_owned())
                .await?;
        }

        Ok(())
    }
}
