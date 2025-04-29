use std::marker::PhantomData;
use std::sync::Arc;

use attestation_tree::{
    attestation_tree_from_prefixes,
    prove_leaf_pair,
    storage_key_for_prefix_key_tuple,
    CommitmentMapPrefixFoliate,
    LocksStakingPrefixFoliate,
    PrefixFoliate,
};
use codec::Decode;
use frame_support::traits::StorageInstance;
use pallet_system_contracts::_GeneratedPrefixForStorageStakingContract;
use proof_of_sql::sql::evm_proof_plan::EVMProofPlan;
use proof_of_sql::sql::proof::ProofPlan;
use proof_of_sql::sql::proof_plans::DynProofPlan;
use proof_of_sql_commitment_map::{CommitmentScheme, TableCommitmentBytes};
use sc_client_api::{Backend as BackendT, StorageKey, StorageProvider};
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_runtime::traits::Block as BlockT;
use sxt_core::tables::TableIdentifier;
use sxt_runtime::pallet_commitments;

use crate::commitments::api::{VerifiableCommitment, VerifiableCommitmentsResponse};
use crate::commitments::error::CommitmentsApiError;
use crate::commitments::limits::{NUM_TABLES_LIMIT, PROOF_PLAN_SIZE_LIMIT};
use crate::commitments::CommitmentsApiServer;

/// Deserialize a `DynProofPlan` from a binary representation.
///
/// Try to deserialize as an `EVMProofPlan` first, then fallback to `DynProofPlan`.
fn try_from_bincode_as_dyn_proof_plan(
    bytes: &[u8],
) -> Result<DynProofPlan, bincode::error::DecodeError> {
    let cfg = bincode::config::legacy()
        .with_fixed_int_encoding()
        .with_big_endian();

    // Attempt parse as EVMProofPlan first
    bincode::serde::decode_from_slice::<EVMProofPlan, _>(bytes, cfg)
        .map(|(proof_plan, _)| proof_plan.into_inner())
        .or_else(|_| {
            let (proof_plan, _) = bincode::serde::decode_from_slice::<DynProofPlan, _>(bytes, cfg)?;
            Ok(proof_plan)
        })
}

pub struct CommitmentsApiImpl<Client, Backend, Block, Config> {
    client: Arc<Client>,
    _phantom: PhantomData<(Backend, Block, Config)>,
}

impl<Client, Backend, Block, Config> CommitmentsApiImpl<Client, Backend, Block, Config> {
    /// Construct a new [`CommitmentsApiImpl`].
    pub fn new(client: Arc<Client>) -> Self {
        CommitmentsApiImpl {
            client,
            _phantom: PhantomData,
        }
    }
}

fn storage_key_for<PF: PrefixFoliate>() -> StorageKey {
    StorageKey(<PF::StorageInstance as StorageInstance>::prefix_hash().to_vec())
}

impl<Client, Backend, Block, Config> CommitmentsApiServer<Block::Hash>
    for CommitmentsApiImpl<Client, Backend, Block, Config>
where
    Client: Send + Sync + HeaderBackend<Block> + StorageProvider<Block, Backend> + 'static,
    Backend: BackendT<Block> + 'static,
    Block: BlockT + 'static,
    Config: Send
        + Sync
        + pallet_commitments::Config
        + pallet_balances::Config<(), Balance = u128>
        + pallet_system_contracts::Config
        + 'static,
{
    fn v1_verifiable_commitments_for_proof_plan(
        &self,
        proof_plan: Bytes,
        commitment_scheme: CommitmentScheme,
        at: Option<Block::Hash>,
    ) -> Result<VerifiableCommitmentsResponse<Block::Hash>, CommitmentsApiError> {
        let proof_plan_size = proof_plan.len();
        if proof_plan_size > PROOF_PLAN_SIZE_LIMIT {
            return Err(CommitmentsApiError::ProofPlanSizeLimit { proof_plan_size });
        }

        let proof_plan = try_from_bincode_as_dyn_proof_plan(&proof_plan)?;

        let table_identifiers = proof_plan
            .get_table_references()
            .into_iter()
            .map(TableIdentifier::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let num_tables = table_identifiers.len();
        if num_tables > NUM_TABLES_LIMIT {
            return Err(CommitmentsApiError::NumTablesLimit { num_tables });
        }

        let at = at.unwrap_or_else(|| self.client.info().best_hash);

        let table_commitments = table_identifiers
            .into_iter()
            .map(|table_identifier| {
                let storage_key = StorageKey(storage_key_for_prefix_key_tuple::<
                    CommitmentMapPrefixFoliate<Config>,
                >((
                    table_identifier.clone(),
                    commitment_scheme,
                )));

                self.client
                    .storage(at, &storage_key)?
                    .ok_or(CommitmentsApiError::NoSuchCommitment)
                    .and_then(|storage_bytes| {
                        let table_commitment_bytes =
                            TableCommitmentBytes::decode(&mut storage_bytes.0.as_ref())?;

                        Ok((table_identifier, table_commitment_bytes))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let commitments_prefix_iter = self
            .client
            .storage_pairs(
                at,
                Some(&storage_key_for::<CommitmentMapPrefixFoliate<Config>>()),
                None,
            )?
            .map(|(key, data)| (key.0, data.0));
        let locks_prefix_iter = self
            .client
            .storage_pairs(
                at,
                Some(&storage_key_for::<LocksStakingPrefixFoliate<Config>>()),
                None,
            )?
            .map(|(key, data)| (key.0, data.0));
        let storage_contract_info = self
            .client
            .storage(
                at,
                &StorageKey(
                    _GeneratedPrefixForStorageStakingContract::<Config>::prefix_hash().to_vec(),
                ),
            )?
            .ok_or(CommitmentsApiError::NoStakingContract)?;

        let attestation_tree = attestation_tree_from_prefixes::<_, _, Config>(
            commitments_prefix_iter,
            locks_prefix_iter,
            storage_contract_info.0,
        )?;

        let verifiable_commitments = table_commitments
            .into_iter()
            .map(|(table_identifier, table_commitment_bytes)| {
                prove_leaf_pair::<CommitmentMapPrefixFoliate<Config>>(
                    &attestation_tree,
                    (table_identifier.clone(), commitment_scheme),
                    table_commitment_bytes.clone(),
                )
                .map(|merkle_proof| {
                    let table_identifier = String::try_from(&table_identifier)
                        .expect("TableIdentifier built from TableRef should have valid utf8");
                    let commitment = Bytes(table_commitment_bytes.data.into_inner());

                    (
                        table_identifier,
                        VerifiableCommitment {
                            merkle_proof,
                            commitment,
                        },
                    )
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(VerifiableCommitmentsResponse {
            verifiable_commitments,
            at,
        })
    }
}
