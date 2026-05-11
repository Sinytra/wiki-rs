use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                "ALTER TABLE project_item_page ADD CONSTRAINT project_item_page_pk PRIMARY KEY (item_id, path);"
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared(
                "ALTER TABLE project_item_page DROP CONSTRAINT project_item_page_pk;",
            )
            .await?;

        Ok(())
    }
}
