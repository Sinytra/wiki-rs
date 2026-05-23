pub use sea_orm_migration::prelude::*;

pub mod builtins;

pub use builtins::BuiltinsMigrator;

mod m20241115_210200_create_project;
mod m20250126_210200_users;
mod m20250131_211200_game_content;
mod m20250525_211200_system_info;
mod m20250609_211200_deployments;
mod m20250612_211200_project_issue;
mod m20250621_211200_reports;
mod m20250723_211200_keys;
mod m20260507_143811_item_page;

pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20241115_210200_create_project::Migration),
            Box::new(m20250126_210200_users::Migration),
            Box::new(m20250131_211200_game_content::Migration),
            Box::new(m20250525_211200_system_info::Migration),
            Box::new(m20250609_211200_deployments::Migration),
            Box::new(m20250612_211200_project_issue::Migration),
            Box::new(m20250621_211200_reports::Migration),
            Box::new(m20250723_211200_keys::Migration),
            Box::new(m20260507_143811_item_page::Migration),
        ]
    }
}
