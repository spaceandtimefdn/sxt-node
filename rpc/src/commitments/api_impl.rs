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
use pallet_commitments::runtime_api::CommitmentsApi;
use pallet_system_contracts::_GeneratedPrefixForStorageStakingContract;
use proof_of_sql::sql::evm_proof_plan::EVMProofPlan;
use proof_of_sql::sql::proof::ProofPlan;
use proof_of_sql::sql::proof_plans::DynProofPlan;
use proof_of_sql_commitment_map::{CommitmentScheme, TableCommitmentBytes};
use proof_of_sql_planner::statement_with_uppercase_identifiers;
use sc_client_api::{Backend as BackendT, StorageKey, StorageProvider};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_runtime::traits::Block as BlockT;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use sxt_core::tables::TableIdentifier;
use sxt_core::utils::proof_of_sql_bincode_config;
use sxt_runtime::pallet_commitments;

use super::proof_plan_for_query_and_commitments::ProofPlanForQueryAndCommitments;
use super::statement_and_associated_table_refs::StatementAndAssociatedTableRefs;
use crate::commitments::api::{
    ProofPlanResponse,
    VerifiableCommitment,
    VerifiableCommitmentsResponse,
};
use crate::commitments::error::CommitmentsApiError;
use crate::commitments::limits::{NUM_TABLES_LIMIT, PROOF_PLAN_SIZE_LIMIT, QUERY_SIZE_LIMIT};
use crate::commitments::CommitmentsApiServer;

/// Deserialize a `DynProofPlan` from a binary representation.
///
/// Try to deserialize as an `EVMProofPlan` first, then fallback to `DynProofPlan`.
fn try_from_bincode_as_dyn_proof_plan(
    bytes: &[u8],
) -> Result<DynProofPlan, bincode::error::DecodeError> {
    let cfg = proof_of_sql_bincode_config::<PROOF_PLAN_SIZE_LIMIT>();

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
    Client: Send
        + Sync
        + HeaderBackend<Block>
        + StorageProvider<Block, Backend>
        + ProvideRuntimeApi<Block>
        + 'static,
    Client::Api: pallet_commitments::runtime_api::CommitmentsApi<Block>,
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

        let proof_plan = try_from_bincode_as_dyn_proof_plan(&proof_plan)
            .map_err(|source| CommitmentsApiError::DeserializeProofPlan { source })?;

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

    fn v1_proof_plan(
        &self,
        query: String,
        at: Option<Block::Hash>,
    ) -> Result<ProofPlanResponse<Block::Hash>, CommitmentsApiError> {
        let query_size = query.len();
        if query_size > QUERY_SIZE_LIMIT {
            return Err(CommitmentsApiError::QuerySizeLimit { query_size });
        }

        let [statement] = Parser::parse_sql(&GenericDialect {}, &query)?
            .try_into()
            .map_err(|statements: Vec<_>| {
                let num_statements = statements.len();

                CommitmentsApiError::NotOneStatement { num_statements }
            })?;

        let statement = statement_with_uppercase_identifiers(statement);

        let statement_and_associated_table_refs =
            StatementAndAssociatedTableRefs::try_from(statement)?;

        let num_tables = statement_and_associated_table_refs.table_refs().len();
        if num_tables > NUM_TABLES_LIMIT {
            return Err(CommitmentsApiError::NumTablesLimit { num_tables });
        }

        let table_identifiers = statement_and_associated_table_refs
            .table_refs()
            .iter()
            .cloned()
            .map(TableIdentifier::try_from)
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .expect("We've already verified that there are fewer than 64 tables");

        let at = at.unwrap_or_else(|| self.client.info().best_hash);

        let proof_plan = self
            .client
            .runtime_api()
            .table_commitments_any_scheme(at, table_identifiers)?
            .ok_or(CommitmentsApiError::IncompleteCommitmentCoverage)?
            .map(ProofPlanForQueryAndCommitments(
                statement_and_associated_table_refs,
            ))
            .unwrap()?;

        let proof_plan_bytes = bincode::serde::encode_to_vec(
            &proof_plan,
            proof_of_sql_bincode_config::<PROOF_PLAN_SIZE_LIMIT>(),
        )?;

        let proof_plan = Bytes(proof_plan_bytes);

        Ok(ProofPlanResponse { proof_plan, at })
    }

    fn v1_evm_proof_plan(
        &self,
        query: String,
        at: Option<Block::Hash>,
    ) -> Result<ProofPlanResponse<Block::Hash>, CommitmentsApiError> {
        let proof_plan_response = self.v1_proof_plan(query, at)?;

        let cfg = proof_of_sql_bincode_config::<PROOF_PLAN_SIZE_LIMIT>();

        let (dyn_proof_plan, _) =
            bincode::serde::decode_from_slice(&proof_plan_response.proof_plan, cfg)
                .map_err(|source| CommitmentsApiError::DeserializeProofPlan { source })?;

        let evm_proof_plan = EVMProofPlan::new(dyn_proof_plan);

        let proof_plan_bytes = bincode::serde::encode_to_vec(&evm_proof_plan, cfg)?;

        let proof_plan = Bytes(proof_plan_bytes);

        Ok(ProofPlanResponse {
            proof_plan,
            ..proof_plan_response
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn we_cannot_deserialize_proof_plan_with_capacity_overflow() {
        let bad_input = u128::MAX.to_be_bytes();
        assert!(try_from_bincode_as_dyn_proof_plan(bad_input.as_slice()).is_err());
    }

    #[test]
    fn we_cannot_deserialize_proof_plan_with_memory_overallocation() {
        let bad_input = hex::decode(
            "0000000700000002000000000000001000000000000000000000000a54494d455f5354414d500000",
        )
        .unwrap();
        assert!(try_from_bincode_as_dyn_proof_plan(bad_input.as_slice()).is_err());
    }
}
