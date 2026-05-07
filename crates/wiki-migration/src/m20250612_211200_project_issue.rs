use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ProjectIssue {
    Table,
    Id,
    Level,
    DeploymentId,
    #[sea_orm(iden = "type")]
    Type,
    Subject,
    Details,
    File,
    VersionName,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Deployment {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ProjectIssue::Table)
                    .col(string_len(ProjectIssue::Id, 28).primary_key())
                    .col(string_len(ProjectIssue::Level, 255))
                    .col(string_len(ProjectIssue::DeploymentId, 28))
                    .col(string_len(ProjectIssue::Type, 255))
                    .col(string_len(ProjectIssue::Subject, 255))
                    .col(text_null(ProjectIssue::Details))
                    .col(text_null(ProjectIssue::File))
                    .col(string_len_null(ProjectIssue::VersionName, 255))
                    .col(timestamp(ProjectIssue::CreatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .from(ProjectIssue::Table, ProjectIssue::DeploymentId)
                            .to(Deployment::Table, Deployment::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE OR REPLACE TRIGGER project_issue_set_id \
                     BEFORE INSERT ON project_issue \
                     FOR EACH ROW EXECUTE FUNCTION set_random_id(28);",
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ProjectIssue::Table).if_exists().to_owned())
            .await
    }
}
