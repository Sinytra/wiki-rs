use sea_orm_migration::prelude::*;

#[tokio::main]
async fn main() {
    match std::env::var("MIGRATION_GROUP").as_deref() {
        Ok("builtins") => cli::run_cli(migration::BuiltinsMigrator).await,
        _ => cli::run_cli(migration::Migrator).await,
    }
}
