pub use sea_orm_migration::prelude::*;

mod m20251115_init;
mod m20251128_deny_list;
mod m20251128_nullifiers;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251115_init::Migration),
            Box::new(m20251128_deny_list::Migration),
            Box::new(m20251128_nullifiers::Migration),
        ]
    }
}
