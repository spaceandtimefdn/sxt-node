use std::path::PathBuf;
use std::sync::OnceLock;

use ark_serialize::{CanonicalDeserialize, Compress, Validate};
use proof_of_sql::proof_primitive::dory;
use proof_of_sql::proof_primitive::hyperkzg::{
    deserialize_flat_compressed_hyperkzg_public_setup_from_reader,
    HyperKZGPublicSetupOwned,
};
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::PerCommitmentScheme;
use snafu::Snafu;

use super::args::{
    load_dory_public_setup,
    load_hyper_kzg_public_setup,
    LoadPublicSetupError,
    ProofOfSqlPublicSetupArgs,
};

/// Dory public parameters.
static DORY_PUBLIC_PARAMETERS: OnceLock<dory::PublicParameters> = OnceLock::new();

/// Dory prover setup.
static DORY_PROVER_SETUP: OnceLock<dory::ProverSetup<'static>> = OnceLock::new();

static HYPERKZG_PUBLIC_SETUP: OnceLock<HyperKZGPublicSetupOwned> = OnceLock::new();

/// Proof-of-sql public setups for all commitment schemes.
pub static PUBLIC_SETUPS: OnceLock<PerCommitmentScheme<AssociatedPublicSetupType<'static>>> =
    OnceLock::new();

/// Error that can occur when trying to intialize [`PUBLIC_SETUPS`], if it is already initialized.
#[derive(Debug, Snafu)]
#[snafu(display("tried to initialize PUBLIC_SETUPS, but they are already initialized"))]
pub struct PublicSetupAlreadyInitialized;

/// Initializes [`DORY_PROVER_SETUP`] and [`PUBLIC_SETUPS`].
fn get_or_init_public_setups_with(
    dory_public_parameters: &'static dory::PublicParameters,
    hyper_kzg_public_setup: &'static HyperKZGPublicSetupOwned,
) -> &'static PerCommitmentScheme<AssociatedPublicSetupType<'static>> {
    let dory_public_setup =
        DORY_PROVER_SETUP.get_or_init(|| dory::ProverSetup::from(dory_public_parameters));

    PUBLIC_SETUPS.get_or_init(|| PerCommitmentScheme {
        hyper_kzg: hyper_kzg_public_setup,
        dynamic_dory: dory_public_setup,
    })
}

/// Initializes [`PUBLIC_SETUPS`] from a file.
///
/// Does not compare the file to a sha256sum, is intended only for testing.
/// Use [`initialize_from_config`] for production use cases.
pub fn get_or_init_from_files_unchecked(
    dory_public_setup_path: &PathBuf,
    hyper_kzg_public_setup_path: &PathBuf,
) -> &'static PerCommitmentScheme<AssociatedPublicSetupType<'static>> {
    let dory_parameters = DORY_PUBLIC_PARAMETERS.get_or_init(|| {
        dory::PublicParameters::deserialize_with_mode(
            std::fs::read(dory_public_setup_path).unwrap().as_slice(),
            Compress::No,
            Validate::No,
        )
        .unwrap()
    });

    let hyper_kzg_setup = HYPERKZG_PUBLIC_SETUP.get_or_init(|| {
        deserialize_flat_compressed_hyperkzg_public_setup_from_reader(
            std::fs::File::open(hyper_kzg_public_setup_path).unwrap(),
            Validate::No,
        )
        .unwrap()
    });

    get_or_init_public_setups_with(dory_parameters, hyper_kzg_setup)
}

/// Initializes [`PUBLIC_SETUPS`] from small files in the sxt-node repository.
///
/// Does not compare the file to a sha256sum, is intended only for testing.
/// Use [`initialize_from_config`] for production use cases.
pub fn get_or_init_from_files_with_four_points_unchecked(
) -> &'static PerCommitmentScheme<AssociatedPublicSetupType<'static>> {
    let manifest_dir: PathBuf = std::env::var("CARGO_WORKSPACE_DIR")
        .unwrap()
        .parse()
        .unwrap();
    let static_setups_dir = manifest_dir.join("proof-of-sql/static-setups");
    get_or_init_from_files_unchecked(
        &static_setups_dir.join("public_parameters_nu_2.bin"),
        &static_setups_dir.join("ppot_0080_02.bin"),
    )
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

    HYPERKZG_PUBLIC_SETUP
        .set(load_hyper_kzg_public_setup(config).await?)
        .map_err(|_| PublicSetupAlreadyInitialized)?;

    get_or_init_public_setups_with(
        DORY_PUBLIC_PARAMETERS.get().unwrap(),
        HYPERKZG_PUBLIC_SETUP.get().unwrap(),
    );

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
