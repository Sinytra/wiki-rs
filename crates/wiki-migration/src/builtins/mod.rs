use sea_orm_migration::prelude::*;

mod m20250625_211200_game_builtins;

// Tracked in a separate table so these provided-data migrations don't
// interleave with schema migrations and only run when explicitly invoked.
pub struct BuiltinsMigrator;

impl MigratorTrait for BuiltinsMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20250625_211200_game_builtins::Migration)]
    }

    fn migration_table_name() -> sea_orm::DynIden {
        Alias::new("seaql_builtins_migrations").into_iden()
    }
}
