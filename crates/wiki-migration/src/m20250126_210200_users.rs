use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum User {
    #[sea_orm(iden = "user_")]
    Table,
    Id,
    ModrinthId,
    CreatedAt,
    Role,
}

#[derive(DeriveIden)]
enum UserProject {
    Table,
    UserId,
    ProjectId,
    Role,
}

#[derive(DeriveIden)]
enum Project {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(User::Table)
                    .col(text(User::Id).primary_key())
                    .col(text_null(User::ModrinthId))
                    .col(timestamp(User::CreatedAt).default(Expr::current_timestamp()))
                    .col(string_len(User::Role, 255).default("user"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UserProject::Table)
                    .col(text(UserProject::UserId))
                    .col(text(UserProject::ProjectId))
                    .col(string_len(UserProject::Role, 255))
                    .primary_key(
                        Index::create()
                            .col(UserProject::UserId)
                            .col(UserProject::ProjectId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserProject::Table, UserProject::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserProject::Table, UserProject::ProjectId)
                            .to(Project::Table, Project::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(UserProject::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(User::Table).if_exists().to_owned())
            .await
    }
}
