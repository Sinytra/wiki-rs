use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "project_item_page_best")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub version_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub project_item_id: i64,
    #[sea_orm(column_type = "Text")]
    pub project_page_ref: String,

    #[sea_orm(belongs_to, from = "project_item_id", to = "id")]
    pub project_item: HasOne<super::project_item::Entity>,

    #[sea_orm(
        belongs_to,
        from = "(version_id, project_page_ref)",
        to = "(version_id, ref)"
    )]
    pub project_page: HasOne<super::project_page::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
