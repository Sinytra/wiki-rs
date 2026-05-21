use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum User {
    #[sea_orm(iden = "user_")]
    Table,
    Id,
}

#[derive(DeriveIden)]
enum DataImport {
    Table,
    Id,
    GameVersion,
    Loader,
    LoaderVersion,
    UserId,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DataImport::Table)
                    .col(big_pk_auto(DataImport::Id))
                    .col(string_len(DataImport::GameVersion, 255))
                    .col(string_len(DataImport::Loader, 255))
                    .col(string_len(DataImport::LoaderVersion, 255))
                    .col(text_null(DataImport::UserId))
                    .col(timestamp(DataImport::CreatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .from(DataImport::Table, DataImport::UserId)
                            .to(User::Table, User::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(DataImport::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
