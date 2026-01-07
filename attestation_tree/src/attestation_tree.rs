use std::error::Error;

use eth_merkle_tree::tree::MerkleTree;
use eth_merkle_tree::utils::keccak::keccak256;
use polkadot_sdk::frame_support::traits::StorageInstance;
use polkadot_sdk::pallet_balances;
use snafu::Snafu;

use crate::prefix_foliate::{encode_key_value_leaf, encode_prefix_leaves};
use crate::{CommitmentMapPrefixFoliate, DecodeStorageError, HashAndKeyTuple, PrefixFoliate};

/// Errors that can occur when attempting to generate a proof for an attestation tree.
#[derive(Debug, Snafu)]
pub enum AttestationTreeProofError {
    /// Failed to hash leaf.
    #[snafu(display("failed to hash leaf: {source}"), context(false))]
    HashLeaf {
        /// The source hashing error.
        source: eth_merkle_tree::utils::errors::BytesError,
    },
    /// Attempted to prove leaf that does not exist in attestation tree.
    #[snafu(display("attempted to prove leaf that does not exist in attestation tree"))]
    NoSuchLeaf,
}

/// Returns the merkle proof that the given attestation tree contains the given key-value pair.
pub fn prove_leaf_pair<PF>(
    attestation_tree: &MerkleTree,
    key_tuple: <PF::HashAndKeyTuple as HashAndKeyTuple>::KeyTuple,
    value: PF::Value,
) -> Result<Vec<String>, AttestationTreeProofError>
where
    PF: PrefixFoliate,
{
    let bytes = encode_key_value_leaf::<PF>(key_tuple, value);
    let hash = keccak256(&keccak256(&hex::encode(bytes))?)?;

    let leaf_index = attestation_tree
        .locate_leaf(&hash)
        .ok_or(AttestationTreeProofError::NoSuchLeaf)?;

    Ok(attestation_tree.generate_proof(leaf_index))
}

/// Errors that can occur when creating an attestation tree.
#[derive(Debug, Snafu)]
pub enum AttestationTreeError {
    /// Failed to decode storage bytes.
    #[snafu(display("failed to decode storage bytes: {source}"), context(false))]
    DecodeStorage {
        /// The source [`DecodeStorageError`].
        source: DecodeStorageError,
    },
    /// Failed to pre-hash leaf.
    #[snafu(display("failed to pre-hash leaf: {source}"), context(false))]
    PreHashLeaf {
        /// The source hashing error.
        source: eth_merkle_tree::utils::errors::BytesError,
    },
    /// Failed to create merkle tree from leaves.
    #[snafu(display("failed to create merkle tree from leaves: {error}"))]
    CreateTreeFromLeaves {
        /// The source error from `eth_merkle_tree`.
        error: Box<dyn Error>,
    },
}

/// Returns the attestation tree for all [`PrefixFoliate`]s, given raw storage key-value iters.
pub fn attestation_tree_from_prefixes<C, T>(
    commitment_prefix_iter: C,
) -> Result<MerkleTree, AttestationTreeError>
where
    C: IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
    T: pallet_commitments::Config + pallet_balances::Config<(), Balance = u128>,
{
    let pre_hashed_leaves =
        encode_prefix_leaves::<CommitmentMapPrefixFoliate<T>, _>(commitment_prefix_iter)?
            .into_iter()
            // we want to double-keccack hash the leaves
            // the `MerkleTree::new` constructor does it once, so we need to do it once manually
            .map(|leaf_bytes| keccak256(&hex::encode(leaf_bytes)))
            .collect::<Result<Vec<_>, _>>()?;

    MerkleTree::new(&pre_hashed_leaves)
        .map_err(|error| AttestationTreeError::CreateTreeFromLeaves { error })
}

#[cfg(test)]
mod tests {
    use codec::Encode;
    use eth_merkle_tree::utils::bytes::hash_pair;
    use pallet_commitments::_GeneratedPrefixForStorageCommitmentStorageMap;
    use polkadot_sdk::frame_support::traits::StorageInstance;
    use polkadot_sdk::sp_core::blake2_128;
    use proof_of_sql_commitment_map::{CommitmentScheme, TableCommitmentBytes};
    use sxt_core::tables::TableIdentifier;
    use sxt_runtime::Runtime;

    use super::*;

    fn valid_attestation_tree_and_items() -> (
        MerkleTree,
        TableIdentifier,
        CommitmentScheme,
        TableCommitmentBytes,
    ) {
        let table_identifier = TableIdentifier {
            namespace: b"SCHEMA".to_vec().try_into().unwrap(),
            name: b"TABLE".to_vec().try_into().unwrap(),
        };
        let table_identifier_bytes = table_identifier.encode();

        let commitment_scheme = CommitmentScheme::DynamicDory;
        let commitment_scheme_bytes = commitment_scheme.encode();

        let commitment_key_bytes =
            _GeneratedPrefixForStorageCommitmentStorageMap::<Runtime>::prefix_hash()
                .into_iter()
                .chain(blake2_128(&table_identifier_bytes))
                .chain(table_identifier_bytes)
                .chain(blake2_128(&commitment_scheme_bytes))
                .chain(commitment_scheme_bytes)
                .collect::<Vec<_>>();

        let raw_table_commitment_bytes = (0u8..=255).collect::<Vec<_>>();
        let table_commitment_bytes = TableCommitmentBytes {
            data: raw_table_commitment_bytes.clone().try_into().unwrap(),
        };
        let table_commitment_value_bytes = table_commitment_bytes.encode();

        let commitments_iter = [(commitment_key_bytes, table_commitment_value_bytes)];

        let attestation_tree =
            attestation_tree_from_prefixes::<_, Runtime>(commitments_iter).unwrap();

        (
            attestation_tree,
            table_identifier,
            commitment_scheme,
            table_commitment_bytes,
        )
    }

    #[test]
    fn we_can_prove_leaves_in_attestation_tree() {
        let (attestation_tree, table_identifier, commitment_scheme, table_commitment_bytes) =
            valid_attestation_tree_and_items();

        let commitment_leaf = keccak256(
            &keccak256(&hex::encode(encode_key_value_leaf::<
                CommitmentMapPrefixFoliate<Runtime>,
            >(
                (table_identifier.clone(), commitment_scheme),
                table_commitment_bytes.clone(),
            )))
            .unwrap(),
        )
        .unwrap();

        dbg!(&commitment_leaf);

        let proof_of_commitment = prove_leaf_pair::<CommitmentMapPrefixFoliate<Runtime>>(
            &attestation_tree,
            (table_identifier, commitment_scheme),
            table_commitment_bytes,
        )
        .unwrap();

        dbg!(&proof_of_commitment);

        let proven_root_hash = std::iter::once(commitment_leaf)
            .chain(proof_of_commitment.into_iter().map(|h| h[2..].to_string()))
            .reduce(|left, right| hash_pair(&left, &right).unwrap())
            .unwrap();

        assert_eq!(proven_root_hash, attestation_tree.root.unwrap().data);
    }

    #[test]
    fn we_cannot_create_attestation_tree_from_invalid_data() {
        let result = attestation_tree_from_prefixes::<_, Runtime>([(vec![0], vec![1])]);

        assert!(matches!(
            result,
            Err(AttestationTreeError::DecodeStorage { .. })
        ));
    }

    #[test]
    fn we_cannot_prove_leaf_that_does_not_exist() {
        let (attestation_tree, table_identifier, _, table_commitment_bytes) =
            valid_attestation_tree_and_items();

        let nonexistent_scheme = CommitmentScheme::HyperKzg;

        let result = prove_leaf_pair::<CommitmentMapPrefixFoliate<Runtime>>(
            &attestation_tree,
            (table_identifier, nonexistent_scheme),
            table_commitment_bytes,
        );
        assert!(matches!(result, Err(AttestationTreeProofError::NoSuchLeaf)));
    }
}
