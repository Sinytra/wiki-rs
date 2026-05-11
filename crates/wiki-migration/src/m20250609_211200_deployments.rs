use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Deployment {
    Table,
    Id,
    ProjectId,
    Revision,
    Status,
    Active,
    UserId,
    SourceRepo,
    SourceBranch,
    SourcePath,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Project {
    Table,
    Id,
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
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                r#"
CREATE OR REPLACE FUNCTION set_random_id()
    RETURNS TRIGGER AS
$$
DECLARE
    _len   int := COALESCE((TG_ARGV[0])::int, 28);
    _id    text;
    _taken bool;
BEGIN
    LOOP
        SELECT string_agg(
                       substr('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789',
                              ceil(random() * 62)::int, 1), '')
        INTO _id
        FROM generate_series(1, _len);

        EXECUTE format('SELECT EXISTS (SELECT 1 FROM %I.%I WHERE id = $1)',
                       TG_TABLE_SCHEMA, TG_TABLE_NAME)
            USING _id
            INTO _taken;

        EXIT WHEN NOT _taken;
    END LOOP;

    NEW.id := _id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#,
            )
            .await
            .map(|_| ())?;

        manager
            .create_table(
                Table::create()
                    .table(Deployment::Table)
                    .col(string_len(Deployment::Id, 28).primary_key())
                    .col(text(Deployment::ProjectId))
                    .col(json_binary_null(Deployment::Revision))
                    .col(string_len(Deployment::Status, 255))
                    .col(boolean(Deployment::Active).default(false))
                    .col(text_null(Deployment::UserId))
                    .col(text(Deployment::SourceRepo))
                    .col(text(Deployment::SourceBranch))
                    .col(text(Deployment::SourcePath))
                    .col(timestamp(Deployment::CreatedAt).default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .from(Deployment::Table, Deployment::ProjectId)
                            .to(Project::Table, Project::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Deployment::Table, Deployment::UserId)
                            .to(User::Table, User::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                "CREATE OR REPLACE TRIGGER deployment_set_id \
                     BEFORE INSERT ON deployment \
                     FOR EACH ROW EXECUTE FUNCTION set_random_id();\n\
                 CREATE UNIQUE INDEX single_active_deployment \
                     ON deployment (project_id, active) WHERE active;",
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Deployment::Table).if_exists().to_owned())
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared("DROP FUNCTION IF EXISTS set_random_id;")
            .await
            .map(|_| ())
    }
}
