//! Contains static public setups that can be initialized with io operations.
mod args;
pub use args::{LoadPublicSetupError, ProofOfSqlPublicSetupArgs};

mod cells;
pub use cells::{
    initialize_from_config,
    initialize_from_file_unchecked,
    InitializePublicSetupError,
    PublicSetupAlreadyInitialized,
    PUBLIC_SETUPS,
};

#[cfg(test)]
mod test_directory;
