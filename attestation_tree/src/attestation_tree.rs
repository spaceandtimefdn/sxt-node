use std::error::Error;

use eth_merkle_tree::tree::MerkleTree;
use eth_merkle_tree::utils::keccak::keccak256;
use snafu::Snafu;

use crate::prefix_foliate::{encode_key_value_leaf, encode_prefix_leaves};
use crate::{
    CommitmentMapPrefixFoliate,
    DecodeStorageError,
    HashAndKeyTuple,
    LocksStakingPrefixFoliate,
    PrefixFoliate,
};

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
///
/// In addition to storage prefixes, this depends on the staking contract info, stored in the
/// system contracts pallet. This information is encoded into the locks prefix leaves.
pub fn attestation_tree_from_prefixes<C, A, T>(
    commitment_prefix_iter: C,
    locks_prefix_iter: A,
    staking_contract_info: Vec<u8>,
) -> Result<MerkleTree, AttestationTreeError>
where
    C: IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
    A: IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
    T: pallet_commitments::Config + pallet_balances::Config<(), Balance = u128>,
{
    let pre_hashed_leaves =
        encode_prefix_leaves::<CommitmentMapPrefixFoliate<T>, _>(commitment_prefix_iter)?
            .into_iter()
            .chain(encode_prefix_leaves::<LocksStakingPrefixFoliate<T>, _>(
                locks_prefix_iter.into_iter().map(|(key, data)| {
                    let data_with_contract_info = data
                        .into_iter()
                        .chain(staking_contract_info.clone())
                        .collect();

                    (key, data_with_contract_info)
                }),
            )?)
            // we want to double-keccack hash the leaves
            // the `MerkleTree::new` constructor does it once, so we need to do it once manually
            .map(|leaf_bytes| keccak256(&hex::encode(leaf_bytes)))
            .collect::<Result<Vec<_>, _>>()?;

    MerkleTree::new(&pre_hashed_leaves)
        .map_err(|error| AttestationTreeError::CreateTreeFromLeaves { error })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use codec::Encode;
    use eth_merkle_tree::utils::bytes::hash_pair;
    use frame_support::traits::StorageInstance;
    use frame_support::WeakBoundedVec;
    use pallet_balances::{BalanceLock, Reasons, _GeneratedPrefixForStorageLocks};
    use pallet_commitments::_GeneratedPrefixForStorageCommitmentStorageMap;
    use proof_of_sql_commitment_map::{CommitmentScheme, TableCommitmentBytes};
    use sp_core::crypto::AccountId32;
    use sp_core::{blake2_128, ConstU32, H160, U256};
    use sxt_core::system_contracts::ContractInfo;
    use sxt_core::tables::TableIdentifier;
    use sxt_runtime::Runtime;

    use super::*;
    use crate::STAKING_BALANCE_LOCK_ID;

    fn valid_attestation_tree_and_items() -> (
        MerkleTree,
        AccountId32,
        WeakBoundedVec<BalanceLock<u128>, ConstU32<50>>,
        ContractInfo,
        TableIdentifier,
        CommitmentScheme,
        TableCommitmentBytes,
    ) {
        let account_id_bytes: [u8; 32] = (0u8..32).collect::<Vec<_>>().try_into().unwrap();

        let account_key_bytes = _GeneratedPrefixForStorageLocks::<Runtime, ()>::prefix_hash()
            .into_iter()
            .chain(blake2_128(&account_id_bytes))
            .chain(account_id_bytes)
            .collect::<Vec<_>>();

        let account_id = AccountId32::new(account_id_bytes);

        let staking_lock = BalanceLock::<u128> {
            amount: 257,
            id: *STAKING_BALANCE_LOCK_ID,
            reasons: Reasons::All,
        };
        let misc_lock = BalanceLock::<u128> {
            amount: 515,
            id: *b"otherloc",
            reasons: Reasons::All,
        };

        let locks: WeakBoundedVec<_, ConstU32<50>> =
            vec![misc_lock, staking_lock].try_into().unwrap();

        let locks_bytes = locks.encode();

        let locks_iter = [(account_key_bytes, locks_bytes)];

        let chain_id = U256::from(1028u32);
        let address = H160::from_str("0x000102030405060708090a0b0c0d0e0f10111213").unwrap();

        let contract_info = ContractInfo { chain_id, address };

        let contract_info_bytes = contract_info.encode();

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

        let attestation_tree = attestation_tree_from_prefixes::<_, _, Runtime>(
            commitments_iter,
            locks_iter,
            contract_info_bytes,
        )
        .unwrap();

        (
            attestation_tree,
            account_id,
            locks,
            contract_info,
            table_identifier,
            commitment_scheme,
            table_commitment_bytes,
        )
    }

    #[test]
    fn we_can_prove_leaves_in_attestation_tree() {
        let (
            attestation_tree,
            account_id,
            locks,
            contract_info,
            table_identifier,
            commitment_scheme,
            table_commitment_bytes,
        ) = valid_attestation_tree_and_items();

        let locks_leaf = keccak256(
            &keccak256(&hex::encode(encode_key_value_leaf::<
                LocksStakingPrefixFoliate<Runtime>,
            >(
                (account_id.clone(),),
                (locks.clone(), contract_info),
            )))
            .unwrap(),
        )
        .unwrap();

        dbg!(&locks_leaf);

        let proof_of_locks = prove_leaf_pair::<LocksStakingPrefixFoliate<Runtime>>(
            &attestation_tree,
            (account_id,),
            (locks, contract_info),
        )
        .unwrap();

        dbg!(&proof_of_locks);

        let proven_root_hash = std::iter::once(locks_leaf)
            .chain(proof_of_locks.into_iter().map(|h| h[2..].to_string()))
            .reduce(|left, right| hash_pair(&left, &right).unwrap())
            .unwrap();

        assert_eq!(
            proven_root_hash,
            attestation_tree.root.as_ref().unwrap().data
        );

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
        let result =
            attestation_tree_from_prefixes::<_, _, Runtime>([], [(vec![0], vec![1])], vec![]);

        assert!(matches!(
            result,
            Err(AttestationTreeError::DecodeStorage { .. })
        ));
    }

    #[test]
    fn we_cannot_prove_leaf_that_does_not_exist() {
        let (attestation_tree, _, _, _, table_identifier, _, table_commitment_bytes) =
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
