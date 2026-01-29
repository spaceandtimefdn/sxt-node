//! Contains [`DeleteDynamicDoryCommitmentsLazyMigration`], disabling dynamic dory commitments for
//! current tables.

mod migration;
pub use migration::DeleteDynamicDoryCommitmentsLazyMigration;

pub mod weights;

mod benchmarks;

mod tests;
