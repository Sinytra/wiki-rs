use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::{big_integer, text};
use sea_orm_migration::sea_orm::ConnectionTrait;
use sea_orm_migration::sea_query::{Expr, OnConflict, Query};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ProjectPage {
    Table,
    VersionId,
    Ref,
    Path,
}

#[derive(DeriveIden)]
enum ProjectVersion {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProjectItemPage {
    Table,
    ItemId,
    Path,
    ProjectItemId,
    ProjectPageRef,
}

#[derive(DeriveIden)]
enum ProjectItem {
    Table,
    Id,
    ItemId,
    VersionId,
}

#[derive(DeriveIden)]
enum Item {
    Table,
    Id,
    Loc,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectPage::Table)
                    .col(big_integer(ProjectPage::VersionId))
                    .col(text(ProjectPage::Ref))
                    .col(text(ProjectPage::Path))
                    .primary_key(
                        Index::create()
                            .col(ProjectPage::VersionId)
                            .col(ProjectPage::Ref),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectPage::Table, ProjectPage::VersionId)
                            .to(ProjectVersion::Table, ProjectVersion::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        let split_loc = Expr::cust_with_exprs(
            "split_part($1::text, ':', 2)",
            [Expr::col((Item::Table, Item::Loc))],
        );

        let select = Query::select()
            .column((ProjectItem::Table, ProjectItem::VersionId))
            .expr(split_loc.clone())
            .column((ProjectItemPage::Table, ProjectItemPage::Path))
            .from(ProjectItemPage::Table)
            .inner_join(
                ProjectItem::Table,
                Expr::col((ProjectItemPage::Table, ProjectItemPage::ItemId))
                    .equals((ProjectItem::Table, ProjectItem::Id)),
            )
            .inner_join(
                Item::Table,
                Expr::col((ProjectItem::Table, ProjectItem::ItemId))
                    .equals((Item::Table, Item::Id)),
            )
            .to_owned();

        let mut insert = Query::insert()
            .into_table(ProjectPage::Table)
            .columns([
                ProjectPage::VersionId,
                ProjectPage::Ref,
                ProjectPage::Path,
            ])
            .select_from(select)
            .map_err(|e| DbErr::Custom(e.to_string()))?
            .to_owned();
        insert.on_conflict(OnConflict::new().do_nothing().to_owned());
        manager.exec_stmt(insert).await?;

        let update = Query::update()
            .table(ProjectItemPage::Table)
            .value(ProjectItemPage::Path, split_loc)
            .from(ProjectItem::Table)
            .from(Item::Table)
            .and_where(
                Expr::col((ProjectItemPage::Table, ProjectItemPage::ItemId))
                    .equals((ProjectItem::Table, ProjectItem::Id)),
            )
            .and_where(
                Expr::col((ProjectItem::Table, ProjectItem::ItemId))
                    .equals((Item::Table, Item::Id)),
            )
            .to_owned();
        manager.exec_stmt(update).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ProjectItemPage::Table)
                    .rename_column(ProjectItemPage::ItemId, ProjectItemPage::ProjectItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ProjectItemPage::Table)
                    .rename_column(ProjectItemPage::Path, ProjectItemPage::ProjectPageRef)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ProjectItemPage::Table)
                    .rename_column(ProjectItemPage::ProjectPageRef, ProjectItemPage::Path)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ProjectItemPage::Table)
                    .rename_column(ProjectItemPage::ProjectItemId, ProjectItemPage::ItemId)
                    .to_owned(),
            )
            .await?;

        let update = Query::update()
            .table(ProjectItemPage::Table)
            .value(
                ProjectItemPage::Path,
                Expr::col((ProjectPage::Table, ProjectPage::Path)),
            )
            .from(ProjectItem::Table)
            .from(ProjectPage::Table)
            .and_where(
                Expr::col((ProjectItemPage::Table, ProjectItemPage::ItemId))
                    .equals((ProjectItem::Table, ProjectItem::Id)),
            )
            .and_where(
                Expr::col((ProjectPage::Table, ProjectPage::VersionId))
                    .equals((ProjectItem::Table, ProjectItem::VersionId)),
            )
            .and_where(
                Expr::col((ProjectPage::Table, ProjectPage::Ref))
                    .equals((ProjectItemPage::Table, ProjectItemPage::Path)),
            )
            .to_owned();
        manager.exec_stmt(update).await?;

        manager
            .drop_table(
                Table::drop()
                    .table(ProjectPage::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
