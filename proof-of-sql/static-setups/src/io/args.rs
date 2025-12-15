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
use handlebars::{Handlebars, Template};
use hex::FromHex;
use proof_of_sql::proof_primitive::dory::PublicParameters;
use proof_of_sql::proof_primitive::hyperkzg::{
    deserialize_flat_compressed_hyperkzg_public_setup_from_slice,
    HyperKZGPublicSetupOwned,
};
use rayon::iter::{IndexedParallelIterator, ParallelIterator};
use rayon::slice::ParallelSlice;
use reqwest::Response;
use serde::Serialize;
use sha2::{Digest, Sha256};
use snafu::Snafu;
use tokio::io::AsyncWriteExt;
use url::Url;

const DEFAULT_HYPER_KZG_PUBLIC_SETUP_BASE_URL_TEMPLATE: &str = "https://github.com/spaceandtimelabs/sxt-proof-of-sql/releases/download/ppot_0080_{{release_degree}}_compressed_elements/";

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

    /// Directory to download proof-of-sql hyper_kzg public setup files.
    ///
    /// If the files already exist, setup is loaded from the files without download.
    #[arg(long, env, default_value = ".")]
    pub hyper_kzg_public_setup_directory: PathBuf,

    /// Base url template to download proof-of-sql hyperkzg setup files.
    ///
    /// Variables available to the template:
    /// - release_degree
    #[arg(
        long,
        env,
        default_value = DEFAULT_HYPER_KZG_PUBLIC_SETUP_BASE_URL_TEMPLATE,
        value_parser = Template::compile
    )]
    pub hyper_kzg_public_setup_base_url_template: Template,
    /// Degree name of proof-of-sql hyperkzg setup files.
    #[arg(long, env, default_value = "final")]
    pub hyper_kzg_public_setup_release_degree: String,
    /// Sha256sum of hyper_kzg ptau to verify loaded file.
    #[arg(long,
        env,
        default_value = "c65198b7006b08652900d3dc4d282e2ad0bc71a04afffdbafa8fba7d956e478f",
        value_parser = |s: &str| <[u8; 32]>::from_hex(s)
    )]
    pub hyper_kzg_public_setup_sha256: [u8; 32],
}

/// Errors that can occur when loading proof-of-sql public setups.
#[derive(Debug, Snafu)]
pub enum LoadPublicSetupError {
    /// Failed to render setup url from template
    #[snafu(
        display("failed to render setup url from template: {source}"),
        context(false)
    )]
    RenderUrlTemplate {
        /// The source render error.
        source: handlebars::RenderError,
    },
    /// Failed to parse rendered url
    #[snafu(display("failed to parse rendered url: {source}"), context(false))]
    ParseRenderedUrl {
        /// The source parse error.
        source: url::ParseError,
    },
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

const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz";

const HYPER_KZG_POINT_COMPRESSED_SIZE: usize = 32;

const BASE_URL_TEMPLATE_NAME: &str = "base_url_template";

#[derive(Serialize)]
struct BaseUrlTemplateValues<'a> {
    release_degree: &'a str,
}

/// Download hyperkzg public setup files to the given directory if they do not exist.
async fn download_hyperkzg_public_setup_files(
    directory: &Path,
    base_url_template: Template,
    release_degree: &str,
) -> Result<Vec<PathBuf>, LoadPublicSetupError> {
    let values = BaseUrlTemplateValues { release_degree };

    let handlebars = {
        let mut handlebars = Handlebars::new();
        handlebars.register_template(BASE_URL_TEMPLATE_NAME, base_url_template);
        handlebars
    };

    let release_downloads_url: Url = handlebars
        .render(BASE_URL_TEMPLATE_NAME, &values)?
        .parse()?;

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

/// Returns hyper_kzg public setup loaded from a file, downloaded if necessary.
pub async fn load_hyper_kzg_public_setup(
    args: &ProofOfSqlPublicSetupArgs,
) -> Result<HyperKZGPublicSetupOwned, LoadPublicSetupError> {
    let file_paths = download_hyperkzg_public_setup_files(
        &args.hyper_kzg_public_setup_directory,
        args.hyper_kzg_public_setup_base_url_template.clone(),
        &args.hyper_kzg_public_setup_release_degree,
    )
    .await?;

    let bytes_by_file = try_join_all(file_paths.into_iter().map(tokio::fs::read)).await?;

    log::info!("verifying sha256sum...");
    let hasher = bytes_by_file
        .iter()
        .fold(Sha256::new(), |mut hasher, bytes| {
            hasher.update(bytes);
            hasher
        });
    let actual_sha256: [u8; 32] = hasher.finalize().into();

    if actual_sha256 != args.hyper_kzg_public_setup_sha256 {
        Err(LoadPublicSetupError::Verification)?
    }

    tokio::task::spawn_blocking(move || {
        Ok(bytes_by_file
            // don't parallelize between files
            // decompressing has a temporary memory cost of 1.5x the decompressed size
            // so, consuming and compressing the bytes one file at a time limits this cost to
            // 1.5x the size of a single file instead of 1.5x the size of the entire setup
            .into_iter()
            .enumerate()
            .map(|(i, file_bytes)| {
                let num_points_in_file = file_bytes.len() / HYPER_KZG_POINT_COMPRESSED_SIZE;
                let num_points_per_par_chunk = num_points_in_file.div_ceil(num_cpus::get());
                let par_chunk_size = num_points_per_par_chunk * HYPER_KZG_POINT_COMPRESSED_SIZE;

                log::info!("decompressing and deserializing hyperkzg file {i}");
                let result = file_bytes
                    // instead parallelize within files
                    .par_chunks(par_chunk_size)
                    .enumerate()
                    .map(|(j, chunk_bytes)| {
                        log::debug!("decompressing and deserializing hyperkzg chunk {i}-{j}");
                        let result = deserialize_flat_compressed_hyperkzg_public_setup_from_slice(
                            chunk_bytes,
                            Validate::No,
                        );
                        log::debug!(
                            "finished decompressing and deserializing hyperkzg chunk {i}-{j}"
                        );
                        Ok(result?)
                    })
                    .collect::<Result<Vec<_>, LoadPublicSetupError>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                log::info!("finished decompressing and deserializing hyperkzg file {i}");

                Ok(result)
            })
            .collect::<Result<Vec<HyperKZGPublicSetupOwned>, LoadPublicSetupError>>()?
            .into_iter()
            .flatten()
            .collect())
    })
    .await?
}

#[cfg(test)]
pub mod tests {
    use ark_serialize::CanonicalSerialize;
    use clap::Parser;

    use super::*;
    use crate::io::test_directory::TestDirectory;

    /// Test config that will load nu_1 setups from a file in this repository.
    pub fn sample_config_from_file(test_directory: &TestDirectory) -> ProofOfSqlPublicSetupArgs {
        ProofOfSqlPublicSetupArgs {
            dory_public_setup_path: Some("public_parameters_nu_1".parse().unwrap()),
            dory_public_setup_url: "https://unused.com".parse().unwrap(),
            dory_public_setup_sha256: <[u8; 32]>::from_hex(
                b"ff917d588abb232ebf0192b84f0b40fcefa163e04abe0f37358c5a914098d2ad",
            )
            .unwrap(),
            hyper_kzg_public_setup_directory: test_directory.path.clone(),
            hyper_kzg_public_setup_base_url_template: Template::compile(
                DEFAULT_HYPER_KZG_PUBLIC_SETUP_BASE_URL_TEMPLATE,
            )
            .unwrap(),
            hyper_kzg_public_setup_release_degree: "02".parse().unwrap(),
            hyper_kzg_public_setup_sha256: <[u8; 32]>::from_hex(
                b"1821173e2452afb5ad77ff8ef740140cd5e57b9b847d8b6edb81e04897b1efe4",
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
        let file_names = download_hyperkzg_public_setup_files(
            &test_directory.path,
            Template::compile(DEFAULT_HYPER_KZG_PUBLIC_SETUP_BASE_URL_TEMPLATE).unwrap(),
            "03",
        )
        .await
        .unwrap();
        assert_eq!(file_names, vec![expected_file_path.clone()]);
        assert!(tokio::fs::try_exists(&expected_file_path).await.unwrap());

        // file name still emitted and file still exists if file is already downloaded
        let file_names = download_hyperkzg_public_setup_files(
            &test_directory.path,
            Template::compile(DEFAULT_HYPER_KZG_PUBLIC_SETUP_BASE_URL_TEMPLATE).unwrap(),
            "03",
        )
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
    async fn we_can_load_small_hyper_kzg_public_setup_from_url() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = ProofOfSqlPublicSetupArgs {
            hyper_kzg_public_setup_release_degree: "03".to_string(),
            hyper_kzg_public_setup_sha256: <[u8; 32]>::from_hex(
                b"68c22caac883f8b569d11a7bab024c87f53a14f956c23f52fe6722473218721d",
            )
            .unwrap(),
            ..sample_config_from_file(&test_directory)
        };

        let loaded_setup = load_hyper_kzg_public_setup(&setup_args).await.unwrap();

        assert_eq!(loaded_setup.len(), 8);
    }

    #[tokio::test]
    async fn we_can_load_public_setups_from_files() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = sample_config_from_file(&test_directory);

        let mut buffer = vec![];

        load_dory_public_setup(&setup_args)
            .await
            .unwrap()
            .serialize_with_mode(&mut buffer, Compress::No)
            .unwrap();

        assert_eq!(&include_bytes!("../../public_parameters_nu_1")[..], buffer,);

        let hyper_kzg_setup = load_hyper_kzg_public_setup(&setup_args).await.unwrap();

        assert_eq!(hyper_kzg_setup.len(), 4);
    }

    #[tokio::test]
    async fn we_cannot_load_public_setup_with_bad_rendered_url() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = ProofOfSqlPublicSetupArgs {
            hyper_kzg_public_setup_base_url_template: Template::compile(
                "https:::://///bad.url.com/{{release_degree}}",
            )
            .unwrap(),
            ..sample_config_from_file(&test_directory)
        };

        let result = load_hyper_kzg_public_setup(&setup_args).await;

        if let Err(e) = &result {
            dbg!(e);
        }

        assert!(matches!(
            result,
            Err(LoadPublicSetupError::ParseRenderedUrl { .. })
        ));
    }

    #[tokio::test]
    async fn we_cannot_load_public_setup_from_nonexistent_file() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let nonexistent_path: PathBuf = "nonexistent".parse().unwrap();
        let setup_args = ProofOfSqlPublicSetupArgs {
            dory_public_setup_path: Some(nonexistent_path.clone()),
            hyper_kzg_public_setup_directory: nonexistent_path,
            ..sample_config_from_file(&test_directory)
        };

        let result = load_dory_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Io { .. })));

        let result = load_hyper_kzg_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Io { .. })));
    }

    #[tokio::test]
    async fn we_cannot_load_public_setup_from_nonexistent_url() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = ProofOfSqlPublicSetupArgs {
            dory_public_setup_path: None,
            dory_public_setup_url: "https://www.google.com/404".parse().unwrap(),
            ..sample_config_from_file(&test_directory)
        };

        let result = load_dory_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Url { .. })));
    }

    #[tokio::test]
    async fn we_cannot_verify_public_setup_against_zero_hash() {
        let test_directory = TestDirectory::random(&mut rand::thread_rng());
        let setup_args = ProofOfSqlPublicSetupArgs {
            dory_public_setup_sha256: [0; 32],
            hyper_kzg_public_setup_sha256: [0; 32],
            ..sample_config_from_file(&test_directory)
        };

        let result = load_dory_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Verification)));

        let result = load_hyper_kzg_public_setup(&setup_args).await;

        assert!(matches!(result, Err(LoadPublicSetupError::Verification)));
    }
}
