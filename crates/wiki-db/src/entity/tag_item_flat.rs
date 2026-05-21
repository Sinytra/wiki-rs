use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tag_item_flat")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub parent: i64,
    pub child: i64,

    #[sea_orm(belongs_to, from = "parent", to = "id")]
    pub project_tag: HasOne<super::project_tag::Entity>,

    #[sea_orm(belongs_to, from = "child", to = "id")]
    pub project_item: HasOne<super::project_item::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
