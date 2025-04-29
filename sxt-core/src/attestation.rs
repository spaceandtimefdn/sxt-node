//! Attestation types and functionality for the SxT chain.
//!
//! This module provides utilities for creating and verifying attestations,
//! particularly Ethereum-style attestations using ECDSA signatures.

use alloc::vec::Vec;

use codec::{Decode, Encode, MaxEncodedLen};
use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
use scale_info::TypeInfo;
use serde::{Serialize, Serializer};
use sha3::digest::core_api::CoreWrapper;
use sha3::{Digest, Keccak256, Keccak256Core};
use snafu::{ResultExt, Snafu};
pub use sp_core::hashing::{blake2_128, blake2_256};
use sp_core::{Bytes, ConstU32};
pub use sp_core::{RuntimeDebug, H256};
use sp_runtime::{format, BoundedVec};

/// Hex serialization function.
///
/// Can be used in `#[serde(serialize_with = "")]` attributes for any `AsRef<[u8]>` type.
fn serialize_bytes_hex<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    Bytes(bytes.to_vec()).serialize(serializer)
}

/// Represents an Ethereum-style ECDSA signature, broken into its components.
///
/// Wrapper around the [`k256::ecdsa::Signature`] type.
#[derive(
    Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Serialize,
)]
pub struct EthereumSignature {
    /// The `r` component of the signature.
    #[serde(serialize_with = "serialize_bytes_hex")]
    pub r: [u8; 32],
    /// The `s` component of the signature.
    #[serde(serialize_with = "serialize_bytes_hex")]
    pub s: [u8; 32],
    /// The recovery ID, usually 27 or 28 for Ethereum.
    pub v: u8,
}

impl EthereumSignature {
    /// Creates a new `EthereumSignature`.
    ///
    /// If the recovery ID (`v`) is not provided, it defaults to `28`.
    pub fn new(r: [u8; 32], s: [u8; 32], v: Option<u8>) -> Self {
        Self {
            r,
            s,
            v: v.unwrap_or(28),
        }
    }
}

/// Represents the registration of an external address for cryptographic attestation.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RegisterExternalAddress {
    /// Registration for an Ethereum address.
    ///
    /// The registration involves:
    /// 1. An ECDSA signature of the account ID.
    /// 2. The public key corresponding to the Ethereum address.
    ///
    /// The signature is verified by recovering the public key from the signature
    /// and comparing it to the provided public key.
    EthereumAddress {
        /// The ECDSA signature.
        signature: EthereumSignature,
        /// The public key in SEC1 format (33 bytes).
        proposed_pub_key: [u8; 33],
        /// The 20 byte ethereum address
        address20: Address20,
    },
}

/// Types of attestation keys available on-chain.
///
/// Each key type includes the necessary information to verify attestations
/// produced by it.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum AttestationKey {
    /// An Ethereum-style ECDSA attestation key.
    EthereumKey {
        /// A `k256` verifying key in SEC1 format (33 bytes).
        pub_key: [u8; 33],
        /// The 20 byte ethereum address
        address20: Address20,
    },
}

/// Top-level error type for the attestation module.
#[derive(Debug, Snafu)]
pub enum AttestationError {
    /// Error during verification.
    #[snafu(display("Verification error: {:?}", source))]
    VerificationError {
        /// Source of the error.
        source: VerificationError,
    },
    /// Error related to signing or verifying signatures.
    #[snafu(display("Signature error: {:?}", source))]
    SignatureError {
        /// Source of the error.
        source: SignatureError,
    },
    /// Error parsing the public key.
    #[snafu(display("Public key parsing error"))]
    PublicKeyError,

    /// The public key was not in the correct format
    #[snafu(display("Invalid uncompressed public key: must be 65 bytes and start with 0x04"))]
    InvalidPublicKey,

    /// The ethereum address did not fit into the bounded vector
    #[snafu(display("Failed to convert Ethereum address to BoundedVec"))]
    ConversionError,
}

/// Specialized `Result` type for the attestation module.
type Result<T, E = AttestationError> = core::result::Result<T, E>;

/// Errors that can occur during verification.
#[derive(Debug, Snafu)]
pub enum VerificationError {
    /// The recovery ID does not match the Ethereum specification.
    #[snafu(display("Invalid recovery ID: {:?}", recovery_id))]
    InvalidRecoveryIdError {
        /// The recovery id that caused the error
        recovery_id: u8,
    },
    /// The public key could not be recovered.
    #[snafu(display("Key recovery error"))]
    KeyRecoveryError,
    /// The public key could not be parsed.
    #[snafu(display("Public key parsing error"))]
    PublicKeyParsingError,
    /// The signature could not be recovered.
    #[snafu(display("Signature recovery error"))]
    SignatureRecoveryError,

    /// Signature mismatch error
    #[snafu(display("The recovered and expected signatures did not match"))]
    SignatureMismatchError,
}

/// Errors related to signature generation and validation.
#[derive(Debug, Snafu)]
pub enum SignatureError {
    /// Error parsing the private key into the correct format.
    #[snafu(display("Error creating signing key from private key"))]
    CreateSigningKeyError,
}

/// Creates an Ethereum attestation registration.
///
/// # Arguments
/// * `account_id` - The account ID as a byte slice.
/// * `private_key` - The private Ethereum key.
/// * `public_key` - The corresponding public Ethereum key.
///
/// Returns the registration information if successful.
pub fn create_ethereum_attestation_registration(
    account_id: &[u8],
    private_key: &[u8],
    public_key: &[u8],
) -> Result<RegisterExternalAddress> {
    let signature = sign_eth_message(private_key, account_id)?;

    let address20 = uncompressed_public_key_to_address(public_key)?;

    let public_key: [u8; 33] = public_key
        .try_into()
        .map_err(|_| AttestationError::PublicKeyError)?;

    Ok(RegisterExternalAddress::EthereumAddress {
        signature,
        proposed_pub_key: public_key,
        address20,
    })
}

/// Convert a uncompressed public key to an Ethereum address.
///
/// # Parameters
/// - `public_key`: A slice of bytes representing the uncompressed public key.
///
/// # Returns
/// - `Address20`: A bounded vector containing the last 20 bytes of the Keccak256 hash of the public key.
pub fn uncompressed_public_key_to_address(public_key: &[u8]) -> Result<Address20> {
    let verifying_key = VerifyingKey::from_sec1_bytes(public_key)
        .map_err(|_| AttestationError::InvalidPublicKey)?;

    let encoded_point = verifying_key.to_encoded_point(false); // Uncompressed format
    let verifying_key_bytes = encoded_point.as_bytes();
    let public_key = &verifying_key_bytes[1..]; // Skip the 0x04 prefix
    let eth_address = Keccak256::digest(public_key).to_vec();
    let eth_address = &eth_address[12..]; // Take the last 20 bytes

    // Convert to BoundedVec
    Address20::try_from(eth_address.to_vec()).map_err(|_| AttestationError::ConversionError)
}

/// Verifies an Ethereum ECDSA signature.
///
/// # Arguments
/// * `msg` - The message that was signed.
/// * `scalars` - The signature components.
/// * `pub_key` - The public key to verify against.
///
/// Returns `true` if the signature is valid.
pub fn verify_eth_signature(msg: &[u8], scalars: &EthereumSignature, pub_key: &[u8]) -> Result<()> {
    let signature = Signature::from_scalars(scalars.r, scalars.s)
        .map_err(|_| VerificationError::SignatureRecoveryError)
        .context(VerificationSnafu)?;

    let recovery_id = RecoveryId::try_from(scalars.v)
        .map_err(|_| VerificationError::InvalidRecoveryIdError {
            recovery_id: scalars.v,
        })
        .context(VerificationSnafu)?;

    let digest = hash_eth_msg(msg);

    let recovered_pub_key = VerifyingKey::recover_from_digest(digest, &signature, recovery_id)
        .map_err(|_| VerificationError::KeyRecoveryError)
        .context(VerificationSnafu)?;

    let expected_key = VerifyingKey::from_sec1_bytes(pub_key)
        .map_err(|_| VerificationError::PublicKeyParsingError)
        .context(VerificationSnafu)?;

    if recovered_pub_key == expected_key {
        Ok(())
    } else {
        Err(VerificationError::SignatureMismatchError).context(VerificationSnafu)
    }
}

/// Hashes a message with the Ethereum-specific prefix.
///
/// # Arguments
/// * `message` - The message to hash.
///
/// Returns the hashed message.
fn hash_eth_msg(message: &[u8]) -> CoreWrapper<Keccak256Core> {
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    let mut hasher = Keccak256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(message);
    hasher
}

/// Signs a message with a private Ethereum key.
///
/// # Arguments
/// * `private_key` - The private key as a byte slice.
/// * `message` - The message to sign.
///
/// Returns the signature if successful.
pub fn sign_eth_message(private_key: &[u8], message: &[u8]) -> Result<EthereumSignature> {
    let signing_key = SigningKey::from_bytes(private_key.into())
        .map_err(|_| SignatureError::CreateSigningKeyError)
        .context(SignatureSnafu)?;

    let digest = hash_eth_msg(message);

    let (signature, recovery_id) = signing_key.sign_digest_recoverable(digest).unwrap();

    let r = slice_to_scalar(&signature.r().to_bytes()).unwrap();
    let s = slice_to_scalar(&signature.s().to_bytes()).unwrap();

    Ok(EthereumSignature::new(r, s, Some(recovery_id.into())))
}

/// Converts a slice into a fixed-size array.
///
/// Returns `None` if the slice is not of the expected length.
fn slice_to_scalar(slice: &[u8]) -> Option<[u8; 32]> {
    slice.try_into().ok()
}

/// Creates an attestation message by concatenating the state root and block number.
///
/// # Arguments
/// * `state_root` - A reference to the state root, typically a cryptographic hash.
/// * `block_number` - The block number associated with this attestation.
///
/// # Returns
/// A `Vec<u8>` containing the serialized attestation message.
///
pub fn create_attestation_message<BN: Into<u64>>(
    state_root: impl AsRef<[u8]>,
    block_number: BN,
) -> Vec<u8> {
    let mut msg = Vec::with_capacity(state_root.as_ref().len() + core::mem::size_of::<u64>());
    msg.extend_from_slice(state_root.as_ref());
    msg.extend_from_slice(&block_number.into().to_be_bytes());
    msg
}

/// The attested state root of the account and commitment merkle trie
pub type AttestationStateRoot = BoundedVec<u8, ConstU32<64>>;

/// Represents attestations stored on-chain.
#[derive(
    Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Serialize,
)]
#[serde(untagged)]
pub enum Attestation<BH> {
    /// An Ethereum-style attestation.
    #[serde(rename_all = "camelCase")]
    EthereumAttestation {
        /// The signature.
        signature: EthereumSignature,
        /// The public key used to sign the attestation.
        #[serde(serialize_with = "serialize_bytes_hex")]
        proposed_pub_key: [u8; 33],
        /// The ethereum address for this public key
        #[serde(serialize_with = "serialize_bytes_hex")]
        address20: Address20,
        /// The state root included in the attestation.
        #[serde(serialize_with = "serialize_bytes_hex")]
        state_root: AttestationStateRoot,
        /// The block number that was attested
        block_number: u32,
        /// The hash of the block that was attested
        block_hash: BH,
    },
}

/// An ethereum 20 byte address
pub type Address20 = BoundedVec<u8, ConstU32<20>>;

#[cfg(test)]
mod tests {
    use frame_support::assert_ok;
    use k256::elliptic_curve::rand_core::OsRng;

    use super::*;

    fn generate_keypair() -> (SigningKey, VerifyingKey) {
        let signing_key = SigningKey::random(&mut OsRng);
        (signing_key.clone(), *signing_key.verifying_key())
    }

    #[test]
    fn sign_and_verify_ethereum_signature() {
        let (signing_key, verifying_key) = generate_keypair();
        let private_key = signing_key.to_bytes();
        let message = b"5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";

        let signature = sign_eth_message(&private_key, message).unwrap();
        assert_ok!(verify_eth_signature(
            message,
            &signature,
            &verifying_key.to_sec1_bytes()
        ));
    }

    #[test]
    fn verify_eth_signature_detects_different_signer() {
        let (signing_key, verifying_key) = generate_keypair();
        let private_key = signing_key.to_bytes();
        let message = b"5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";

        // Sign the message with the first keypair.
        let signature = sign_eth_message(&private_key, message).unwrap();

        // First verification should succeed.
        assert_ok!(verify_eth_signature(
            message,
            &signature,
            &verifying_key.to_sec1_bytes()
        ));

        // Generate a second keypair and sign the same message.
        let (_, second_public) = generate_keypair();

        // Verify with the second public key. This should fail with VerificationError::SignatureMismatchError.
        let result = verify_eth_signature(message, &signature, &second_public.to_sec1_bytes());

        // Check that the result is an error, and match the wrapped error type.
        assert!(
            matches!(
                result,
                Err(AttestationError::VerificationError {
                    source: VerificationError::SignatureMismatchError
                })
            ),
            "Expected SignatureMismatchError, but got a different error or success"
        );
    }

    #[test]
    fn we_can_serialize_bytes_as_hex() {
        #[derive(Serialize)]
        struct TestSerialize {
            #[serde(serialize_with = "serialize_bytes_hex")]
            array: [u8; 3],
            #[serde(serialize_with = "serialize_bytes_hex")]
            vec: Vec<u8>,
        }

        let actual = serde_json::to_value(TestSerialize {
            array: [0, 1, 2],
            vec: vec![1, 255],
        })
        .unwrap();

        let expected = serde_json::json!({
            "array": "0x000102",
            "vec": "0x01ff",
        });

        assert_eq!(actual, expected);
    }
}
