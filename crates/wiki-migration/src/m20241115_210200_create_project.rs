use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Project {
    Table,
    Id,
    Name,
    SourcePath,
    SourceRepo,
    SourceBranch,
    IsCommunity,
    #[sea_orm(iden = "type")]
    Type,
    Platforms,
    SearchVector,
    CreatedAt,
    IsPublic,
    #[sea_orm(iden = "modid")]
    ModId,
    IsVirtual,
    Visibility,
    Flags
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Project::Table)
                    .if_not_exists()
                    .col(text(Project::Id).primary_key())
                    .col(text(Project::Name))
                    .col(text(Project::SourcePath))
                    .col(text(Project::SourceRepo))
                    .col(text(Project::SourceBranch))
                    .col(boolean(Project::IsCommunity).default(false))
                    .col(string_len(Project::Type, 255))
                    .col(text(Project::Platforms))
                    .col(custom_null(Project::SearchVector, "tsvector"))
                    .col(timestamp(Project::CreatedAt).default(Expr::current_timestamp()))
                    .col(boolean(Project::IsPublic).default(false))
                    .col(string_len_null(Project::ModId, 255))
                    .col(boolean(Project::IsVirtual).default(false))
                    .col(string_len(Project::Visibility, 255).default("public"))
                    .col(text_null(Project::Flags))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("project_source_repo_source_path_key")
                    .table(Project::Table)
                    .col(Project::SourceRepo)
                    .col(Project::SourcePath)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("project_search_vector_idx")
                    .table(Project::Table)
                    .col(Project::SearchVector)
                    .index_type(IndexType::Custom(Alias::new("GIN").into_iden()))
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                r#"
CREATE OR REPLACE FUNCTION update_search_vector()
    RETURNS TRIGGER AS
$$
BEGIN
    NEW.search_vector := to_tsvector('simple', NEW.id || ' ' || NEW.name);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_search_vector
    BEFORE INSERT
    ON project
    FOR EACH ROW
EXECUTE FUNCTION update_search_vector();

CREATE DOMAIN resource_location AS varchar(255)
    CHECK ( VALUE ~* '^([a-z0-9_.-]+:)?[a-z0-9_.-\/]+$' );
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
                "DROP TRIGGER IF EXISTS set_search_vector ON project;\n\
                 DROP FUNCTION IF EXISTS update_search_vector;",
            )
            .await
            .map(|_| ())?;

        manager
            .drop_table(Table::drop().table(Project::Table).to_owned())
            .await?;

        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared("DROP DOMAIN IF EXISTS resource_location;")
            .await
            .map(|_| ())
    }
}
