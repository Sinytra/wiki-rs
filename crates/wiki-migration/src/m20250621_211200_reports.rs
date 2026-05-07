use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Report {
    Table,
    Id,
    #[sea_orm(iden = "type")]
    Type,
    Reason,
    Body,
    Status,
    SubmitterId,
    ProjectId,
    Path,
    Locale,
    VersionId,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Report::Table)
                    .col(string_len(Report::Id, 28).primary_key())
                    .col(string_len(Report::Type, 255))
                    .col(text(Report::Reason))
                    .col(text(Report::Body))
                    .col(string_len(Report::Status, 255))
                    .col(text(Report::SubmitterId))
                    .col(text(Report::ProjectId))
                    .col(text_null(Report::Path))
                    .col(text_null(Report::Locale))
                    .col(big_integer_null(Report::VersionId))
                    .col(timestamp(Report::CreatedAt).default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE OR REPLACE TRIGGER report_set_id \
                     BEFORE INSERT ON report \
                     FOR EACH ROW EXECUTE FUNCTION set_random_id(28);",
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Report::Table).if_exists().to_owned())
            .await
    }
}
