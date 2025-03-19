use std::path::PathBuf;
use std::sync::OnceLock;

use ark_serialize::{CanonicalDeserialize, Compress, Validate};
use proof_of_sql::proof_primitive::dory;
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::PerCommitmentScheme;
use snafu::Snafu;

use super::args::{load_dory_public_setup, LoadPublicSetupError, ProofOfSqlPublicSetupArgs};

/// Dory public parameters.
static DORY_PUBLIC_PARAMETERS: OnceLock<dory::PublicParameters> = OnceLock::new();

/// Dory prover setup.
static DORY_PROVER_SETUP: OnceLock<dory::ProverSetup<'static>> = OnceLock::new();

/// Proof-of-sql public setups for all commitment schemes.
pub static PUBLIC_SETUPS: OnceLock<PerCommitmentScheme<AssociatedPublicSetupType<'static>>> =
    OnceLock::new();

/// Error that can occur when trying to intialize [`PUBLIC_SETUPS`], if it is already initialized.
#[derive(Debug, Snafu)]
#[snafu(display("tried to initialize PUBLIC_SETUPS, but they are already initialized"))]
pub struct PublicSetupAlreadyInitialized;

/// Initializes [`DORY_PROVER_SETUP`] and [`PUBLIC_SETUPS`].
fn initialize_setups_after_load() -> Result<(), PublicSetupAlreadyInitialized> {
    let dory_prover_setup = DORY_PROVER_SETUP
        .get_or_init(|| dory::ProverSetup::from(DORY_PUBLIC_PARAMETERS.get().unwrap()));

    PUBLIC_SETUPS
        .set(PerCommitmentScheme {
            ipa: (),
            dynamic_dory: dory_prover_setup,
        })
        .map_err(|_| PublicSetupAlreadyInitialized)?;

    Ok(())
}

/// Initializes [`PUBLIC_SETUPS`] from a file.
///
/// Does not compare the file to a sha256sum, is intended only for testing.
/// Use [`initialize_from_config`] for production use cases.
pub fn initialize_from_file_unchecked(
    dory_public_setup_path: &PathBuf,
) -> Result<(), PublicSetupAlreadyInitialized> {
    DORY_PUBLIC_PARAMETERS
        .set(
            dory::PublicParameters::deserialize_with_mode(
                std::fs::read(dory_public_setup_path).unwrap().as_slice(),
                Compress::No,
                Validate::No,
            )
            .unwrap(),
        )
        .map_err(|_| PublicSetupAlreadyInitialized)?;

    initialize_setups_after_load()
}

/// Errors that can occur when initializing public setups from config.
#[derive(Debug, Snafu)]
pub enum InitializePublicSetupError {
    /// Failed to load public setups from config.
    #[snafu(display("{source}"), context(false))]
    Load {
        /// Source load error.
        source: LoadPublicSetupError,
    },
    /// Setups already initialized.
    #[snafu(display("{source}"), context(false))]
    AlreadyInitialized {
        /// Source already-initialized error.
        source: PublicSetupAlreadyInitialized,
    },
}

/// Initializes [`PUBLIC_SETUPS`] from config.
pub async fn initialize_from_config(
    config: &ProofOfSqlPublicSetupArgs,
) -> Result<(), InitializePublicSetupError> {
    DORY_PUBLIC_PARAMETERS
        .set(load_dory_public_setup(config).await?)
        .map_err(|_| PublicSetupAlreadyInitialized)?;

    initialize_setups_after_load()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::args::tests::sample_config_from_file;
    use crate::io::test_directory::TestDirectory;

    async fn we_cannot_initialize_public_setups_that_fail_to_load() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = ProofOfSqlPublicSetupArgs {
            dory_public_setup_sha256: [0; 32],
            ..sample_config_from_file(&test_directory)
        };

        let result = initialize_from_config(&setup_args).await;

        assert!(matches!(
            result,
            Err(InitializePublicSetupError::Load { .. })
        ));
    }

    async fn we_can_initialize_public_setups() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = sample_config_from_file(&test_directory);

        initialize_from_config(&setup_args).await.unwrap();

        PUBLIC_SETUPS
            .get()
            .expect("PUBLIC SETUPS should be initialized");
    }

    async fn we_cannot_initialize_public_setups_twice() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = sample_config_from_file(&test_directory);

        let result = initialize_from_config(&setup_args).await;

        assert!(matches!(
            result,
            Err(InitializePublicSetupError::AlreadyInitialized { .. })
        ));
    }

    // we need to run the above tests in a specific order due to their usage of global state
    #[tokio::test]
    async fn test_public_setup_initialization() {
        we_cannot_initialize_public_setups_that_fail_to_load().await;
        we_can_initialize_public_setups().await;
        we_cannot_initialize_public_setups_twice().await;
    }
}
