use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum AccessKey {
    Table,
    Id,
    Name,
    Value,
    UserId,
    ExpiresAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum User {
    #[sea_orm(iden = "user_")]
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AccessKey::Table)
                    .col(big_pk_auto(AccessKey::Id))
                    .col(text(AccessKey::Name))
                    .col(text(AccessKey::Value).unique_key())
                    .col(text_null(AccessKey::UserId))
                    .col(timestamp_null(AccessKey::ExpiresAt))
                    .col(timestamp(AccessKey::CreatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .from(AccessKey::Table, AccessKey::UserId)
                            .to(User::Table, User::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AccessKey::Table).if_exists().to_owned())
            .await
    }
}
