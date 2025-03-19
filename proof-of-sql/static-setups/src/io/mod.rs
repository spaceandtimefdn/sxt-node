//! Contains static public setups that can be initialized with io operations.
mod args;
pub use args::{LoadPublicSetupError, ProofOfSqlPublicSetupArgs};

mod cells;
pub use cells::{
    get_or_init_from_files_unchecked,
    get_or_init_from_files_with_four_points_unchecked,
    initialize_from_config,
    InitializePublicSetupError,
    PublicSetupAlreadyInitialized,
    PUBLIC_SETUPS,
};

#[cfg(test)]
mod test_directory;
