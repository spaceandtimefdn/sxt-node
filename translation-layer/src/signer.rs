use hex::FromHex;
use snafu::ResultExt;
use subxt_signer::sr25519::Keypair;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::error::*;

/// load a hex encoded sr25519 key from a file
pub async fn load_substrate_key(file_path: &str) -> Result<Keypair> {
    let mut file = File::open(file_path).await.context(KeyFileReadSnafu {
        path: file_path.to_string(),
    })?;

    let mut hex_string = String::new();
    file.read_to_string(&mut hex_string)
        .await
        .context(KeyFileReadSnafu {
            path: file_path.to_string(),
        })?;

    let key_bytes = Vec::from_hex(hex_string.trim()).context(KeyParseSnafu)?;

    if key_bytes.len() != 32 {
        return Err(Error::InvalidKeyLength {
            length: key_bytes.len(),
        });
    }

    let key_bytes: [u8; 32] =
        key_bytes
            .clone()
            .try_into()
            .map_err(|_| Error::InvalidKeyLength {
                length: key_bytes.len(),
            })?;

    Keypair::from_secret_key(key_bytes).map_err(|_| Error::KeypairCreationError)
}
