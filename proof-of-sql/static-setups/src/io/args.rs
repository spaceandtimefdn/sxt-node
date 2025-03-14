use core::future::Future;
use core::pin::Pin;
use std::path::PathBuf;

use ark_serialize::{CanonicalDeserialize, Compress, Validate};
use clap::Args;
use hex::FromHex;
use proof_of_sql::proof_primitive::dory::PublicParameters;
use sha2::{Digest, Sha256};
use snafu::Snafu;
use url::Url;

/// CLI args for loading proof-of-sql public setups.
#[derive(Debug, Args)]
pub struct ProofOfSqlPublicSetupArgs {
    /// Path to load proof-of-sql ark-serialized dory PublicParameters file.
    ///
    /// If set, takes precedence over downloading.
    #[arg(long, env)]
    pub dory_public_setup_path: Option<PathBuf>,
    /// Url to download proof-of-sql ark-serialized dory PublicParameters file.
    #[arg(
        long,
        env,
        default_value = "https://github.com/spaceandtimelabs/sxt-proof-of-sql/releases/download/dory-prover-params-nu-16/public_parameters_nu_16.bin"
    )]
    pub dory_public_setup_url: Url,
    /// Sha256sum of dory PublicParameters to verify loaded file.
    #[arg(long,
        env,
        default_value = "e6a1bc5b6f1740623a65294553921fc408ee632035e700b92d73cf58f1384375",
        value_parser = |s: &str| <[u8; 32]>::from_hex(s)
    )]
    pub dory_public_setup_sha256: [u8; 32],
}

/// Errors that can occur when loading proof-of-sql public setups.
#[derive(Debug, Snafu)]
pub enum LoadPublicSetupError {
    /// Failed to load setup from url.
    #[snafu(display("failed to load setup from url: {source}"), context(false))]
    Url {
        /// The source reqwest error.
        source: reqwest::Error,
    },
    /// Failed to load setup from file.
    #[snafu(display("failed to load setup from file: {source}"), context(false))]
    Io {
        /// The source io error.
        source: std::io::Error,
    },
    /// Failed to verify setup against sha256sum.
    #[snafu(display("failed to verify setup against sha256sum"))]
    Verification,
    /// Failed to deserialize setup.
    #[snafu(display("failed to deserialize setup: {error}"))]
    Deserialize {
        /// The source deserialization error.
        error: ark_serialize::SerializationError,
    },
}

impl From<ark_serialize::SerializationError> for LoadPublicSetupError {
    fn from(error: ark_serialize::SerializationError) -> Self {
        LoadPublicSetupError::Deserialize { error }
    }
}

/// Returns dory PublicParameters loaded from either a file or url according to the arguments.
pub async fn load_dory_public_setup(
    args: &ProofOfSqlPublicSetupArgs,
) -> Result<PublicParameters, LoadPublicSetupError> {
    let bytes = args
        .dory_public_setup_path
        .as_ref()
        .map_or_else::<Pin<Box<dyn Future<Output = Result<Vec<u8>, LoadPublicSetupError>>>>, _, _>(
            || {
                Box::pin(async move {
                    Ok(reqwest::get(args.dory_public_setup_url.clone())
                        .await?
                        .error_for_status()?
                        .bytes()
                        .await?
                        .into())
                })
            },
            |path| Box::pin(async move { Ok(tokio::fs::read(path).await?) }),
        )
        .await?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual_sha256: [u8; 32] = hasher.finalize().into();

    if actual_sha256 != args.dory_public_setup_sha256 {
        Err(LoadPublicSetupError::Verification)?
    }

    Ok(PublicParameters::deserialize_with_mode(
        bytes.as_slice(),
        Compress::No,
        Validate::No,
    )?)
}

#[cfg(test)]
pub mod tests {
    use ark_serialize::CanonicalSerialize;
    use clap::Parser;

    use super::*;

    /// Test config that will load nu_1 setups from a file in this repository.
    pub fn sample_config_from_file() -> ProofOfSqlPublicSetupArgs {
        ProofOfSqlPublicSetupArgs {
            dory_public_setup_path: Some("public_parameters_nu_1".parse().unwrap()),
            dory_public_setup_url: "https://unused.com".parse().unwrap(),
            dory_public_setup_sha256: <[u8; 32]>::from_hex(
                b"ff917d588abb232ebf0192b84f0b40fcefa163e04abe0f37358c5a914098d2ad",
            )
            .unwrap(),
        }
    }

    #[derive(Debug, Parser)]
    struct TestParser {
        #[command(flatten)]
        setup_args: ProofOfSqlPublicSetupArgs,
    }

    #[tokio::test]
    async fn load_dory_public_setup_succeeds_by_default() {
        let TestParser { setup_args } = TestParser::parse();

        load_dory_public_setup(&setup_args).await.unwrap();
    }

    #[tokio::test]
    async fn we_can_load_dory_public_setup_from_file() {
        let setup_args = sample_config_from_file();

        let mut buffer = vec![];

        load_dory_public_setup(&setup_args)
            .await
            .unwrap()
            .serialize_with_mode(&mut buffer, Compress::No)
            .unwrap();

        assert_eq!(&include_bytes!("../../public_parameters_nu_1")[..], buffer,);
    }

    #[tokio::test]
    async fn we_cannot_load_public_setup_from_nonexistent_file() {
        let setup_args = ProofOfSqlPublicSetupArgs {
            dory_public_setup_path: Some("nonexistent".parse().unwrap()),
            ..sample_config_from_file()
        };

        let result = load_dory_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Io { .. })));
    }

    #[tokio::test]
    async fn we_cannot_load_public_setup_from_nonexistent_url() {
        let setup_args = ProofOfSqlPublicSetupArgs {
            dory_public_setup_path: None,
            dory_public_setup_url: "https://www.google.com/404".parse().unwrap(),
            ..sample_config_from_file()
        };

        let result = load_dory_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Url { .. })));
    }

    #[tokio::test]
    async fn we_cannot_verify_public_setup_against_zero_hash() {
        let setup_args = ProofOfSqlPublicSetupArgs {
            dory_public_setup_sha256: [0; 32],
            ..sample_config_from_file()
        };

        let result = load_dory_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Verification)));
    }
}
