use core::future::Future;
use core::pin::Pin;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ark_serialize::{CanonicalDeserialize, Compress, Validate};
use clap::Args;
use futures::future::try_join_all;
use futures::lock::Mutex;
use futures::stream::StreamExt;
use futures::TryStreamExt;
use hex::FromHex;
use proof_of_sql::proof_primitive::dory::PublicParameters;
use reqwest::Response;
use sha2::{Digest, Sha256};
use snafu::Snafu;
use tokio::io::AsyncWriteExt;
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
    /// Failed to parallelize task.
    #[snafu(display("failed to parallelize task: {source}"), context(false))]
    Threading {
        /// The source tokio error.
        source: tokio::task::JoinError,
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

const PROOF_OF_SQL_RELEASE_DOWNLOADS_URL: &str =
    "https://github.com/spaceandtimelabs/sxt-proof-of-sql/releases/download/";

const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";

/// Doing 2 tasks per cpu ensures that the temporary memory cost of deserialization/decompression
/// never exceeds 125% of the ultimate decompressed setup.
const DESERIALIZATION_TASKS_PER_CPU: usize = 2;

/// Download hyperkzg public setup files to the given directory if they do not exist.
async fn download_hyperkzg_public_setup_files(
    directory: &Path,
    release_degree: &str,
) -> Result<Vec<PathBuf>, LoadPublicSetupError> {
    let release_downloads_url: Url = format!(
        "{PROOF_OF_SQL_RELEASE_DOWNLOADS_URL}ppot_0080_{release_degree}_compressed_elements/",
    )
    .parse()
    .unwrap();

    let http_client = reqwest::Client::new();

    let single_file_name = format!("ppot_0080_{release_degree}.bin");

    let file_names = if http_client
        .head(
            release_downloads_url
                .clone()
                .join(&single_file_name)
                .unwrap(),
        )
        .send()
        .await
        .and_then(Response::error_for_status)
        .is_ok()
    {
        vec![single_file_name]
    } else {
        futures::stream::iter(ALPHABET.chars().flat_map(|first_char| {
            ALPHABET.chars().map(move |second_char| {
                format!("ppot_0080_{release_degree}_chunk_{first_char}{second_char}.bin")
            })
        }))
        .take_while(|file_name| {
            let file_name = file_name.clone();
            let http_client = &http_client;
            let release_downloads_url = release_downloads_url.clone();
            async move {
                http_client
                    .head(release_downloads_url.join(&file_name).unwrap())
                    .send()
                    .await
                    .and_then(Response::error_for_status)
                    .is_ok()
            }
        })
        .collect::<Vec<String>>()
        .await
    };

    let file_paths = file_names
        .iter()
        .map(|file_name| directory.join(file_name))
        .collect::<Vec<_>>();

    let urls_and_paths = file_names
        .iter()
        .zip(file_paths.clone())
        .filter_map(|(file_name, file_path)| {
            file_path
                .try_exists()
                .map(|exists| {
                    (!exists).then_some((
                        release_downloads_url.clone().join(file_name).unwrap(),
                        file_path,
                    ))
                })
                .transpose()
        })
        .collect::<std::io::Result<Vec<_>>>()?;

    try_join_all(urls_and_paths.into_iter().map(|(url, path)| {
        let http_client = http_client.clone();
        async move {
            tokio::spawn(async move {
                let file = Arc::new(Mutex::new(tokio::fs::File::create(path).await?));

                log::info!("downloading hyperkzg setup chunk: {url}");
                http_client
                    .get(url.clone())
                    .send()
                    .await?
                    .error_for_status()?
                    .bytes_stream()
                    .map_err(LoadPublicSetupError::from)
                    .try_for_each(|bytes| {
                        let file = file.clone();
                        async move { Ok(file.lock().await.write_all(&bytes).await?) }
                    })
                    .await?;

                log::info!("finished downloading hyperkzg setup chunk: {url}");

                Result::<_, LoadPublicSetupError>::Ok(())
            })
            .await?
        }
    }))
    .await?;

    Ok(file_paths)
}

#[cfg(test)]
pub mod tests {
    use ark_serialize::CanonicalSerialize;
    use clap::Parser;

    use super::*;
    use crate::io::test_directory::TestDirectory;

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
    async fn we_can_download_hyper_kzg_public_setup_files() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());

        let expected_file_path: PathBuf = test_directory.path.join("ppot_0080_03.bin");

        // file does not exist yet
        assert!(!tokio::fs::try_exists(&expected_file_path).await.unwrap());

        // file gets downloaded
        let file_names = download_hyperkzg_public_setup_files(&test_directory.path, "03")
            .await
            .unwrap();
        assert_eq!(file_names, vec![expected_file_path.clone()]);
        assert!(tokio::fs::try_exists(&expected_file_path).await.unwrap());

        // file name still emitted and file still exists if file is already downloaded
        let file_names = download_hyperkzg_public_setup_files(&test_directory.path, "03")
            .await
            .unwrap();
        assert_eq!(file_names, vec![expected_file_path.clone()]);
        assert!(tokio::fs::try_exists(&expected_file_path).await.unwrap());
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
