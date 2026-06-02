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
                r#"
CREATE MATERIALIZED VIEW project_item_page_best AS
WITH page_counts AS (SELECT pi.version_id        AS version_id,
                            pip.project_page_ref AS ref,
                            COUNT(*)             AS item_count
                     FROM project_item_page pip
                              JOIN project_item pi ON pi.id = pip.project_item_id
                     GROUP BY pi.version_id, pip.project_page_ref)
SELECT DISTINCT ON (pi.version_id, pip.project_item_id) pi.version_id        AS version_id,
                                                        pip.project_item_id  AS project_item_id,
                                                        pip.project_page_ref AS project_page_ref
FROM project_item_page pip
         JOIN project_item pi ON pi.id = pip.project_item_id
         JOIN project_page pp ON pp.version_id = pi.version_id AND pp.ref = pip.project_page_ref
         JOIN page_counts c ON c.version_id = pi.version_id AND c.ref = pip.project_page_ref
ORDER BY pi.version_id,
         pip.project_item_id,
         (c.item_count = 1) DESC,
         pip.project_page_ref;
"#,
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            // language=postgresql
            .execute_unprepared("DROP MATERIALIZED VIEW IF EXISTS project_item_page_best")
            .await
            .map(|_| ())
    }
}
