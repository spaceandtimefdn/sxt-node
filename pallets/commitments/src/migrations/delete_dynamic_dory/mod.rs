//! Contains [`DeleteDynamicDoryCommitmentsLazyMigration`], disabling dynamic dory commitments for
//! current tables.

mod migration;
pub use migration::{DeleteDynamicDoryCommitmentsLazyMigration, MAX_DELETIONS_PER_BLOCK};

pub mod weights;

mod benchmarks;

mod tests;
