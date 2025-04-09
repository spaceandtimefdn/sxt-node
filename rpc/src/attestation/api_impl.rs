use std::marker::PhantomData;
use std::sync::Arc;

use codec::{Decode, Encode};
use frame_support::traits::StorageInstance;
use frame_support::{Blake2_128Concat, StorageHasher};
use sc_client_api::{Backend as BackendT, StorageData, StorageKey, StorageProvider};
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use sxt_core::attestation::Attestation;
use sxt_runtime::MINUTES;

use crate::attestation::api::AttestationApiServer;
use crate::attestation::{AttestationApiError, AttestationsResponse};

/// [`AttestationApiServer`] implementor providing its RPCs.
pub struct AttestationApiImpl<Client, Backend, Block, Config> {
    client: Arc<Client>,
    _phantom: PhantomData<(Backend, Block, Config)>,
}

impl<Client, Backend, Block, Config> AttestationApiImpl<Client, Backend, Block, Config> {
    /// Construct a new [`AttestationApiImpl`].
    pub fn new(client: Arc<Client>) -> Self {
        AttestationApiImpl {
            client,
            _phantom: PhantomData,
        }
    }
}

/// Generates a storage key for the attestations for the given block number.
fn storage_key_for_attestations_for_block_number<Config>(block_number: u32) -> StorageKey
where
    Config: pallet_attestation::Config,
{
    let storage_prefix = <pallet_attestation::_GeneratedPrefixForStorageAttestations<Config> as StorageInstance>::prefix_hash();
    let key_suffix = Blake2_128Concat::hash(&block_number.encode());

    StorageKey(storage_prefix.into_iter().chain(key_suffix).collect())
}

impl<Client, Backend, Block, Config> AttestationApiServer<Block::Hash>
    for AttestationApiImpl<Client, Backend, Block, Config>
where
    Client: Send + Sync + HeaderBackend<Block> + StorageProvider<Block, Backend> + 'static,
    Backend: BackendT<Block> + 'static,
    Block: BlockT + 'static,
    Block::Header: HeaderT<Number = u32>,
    Config: Send + Sync + pallet_attestation::Config + 'static,
{
    fn v1_attestations_for_block(
        &self,
        attestations_for: Block::Hash,
        at: Option<Block::Hash>,
    ) -> Result<AttestationsResponse<Block::Hash>, AttestationApiError> {
        let at = at.unwrap_or_else(|| self.client.info().best_hash);

        let attestations_for_block_number = self
            .client
            .number(attestations_for)?
            .ok_or(AttestationApiError::BlockNumberQuery)?;

        let attestations_bytes = self
            .client
            .storage(
                at,
                &storage_key_for_attestations_for_block_number::<Config>(
                    attestations_for_block_number,
                ),
            )?
            .unwrap_or(StorageData(Vec::<u8>::new().encode()));

        let attestations =
            Vec::<Attestation<Block::Hash>>::decode(&mut attestations_bytes.0.as_slice())?;

        Ok(AttestationsResponse {
            attestations,
            attestations_for,
            attestations_for_block_number,
            at,
        })
    }

    fn v1_best_recent_attestations(
        &self,
        at: Option<Block::Hash>,
    ) -> Result<AttestationsResponse<Block::Hash>, AttestationApiError> {
        let at = at.unwrap_or_else(|| self.client.info().best_hash);

        let end_block_number = self
            .client
            .number(at)?
            .ok_or(AttestationApiError::BlockNumberQuery)?;

        let start_block_number = end_block_number.saturating_sub(MINUTES);

        let best_attestations = (start_block_number..=end_block_number)
            .map(|block_number| {
                let attestations_for = self
                    .client
                    .hash(block_number)?
                    .ok_or(AttestationApiError::BlockHashQuery)?;

                self.v1_attestations_for_block(attestations_for, Some(at))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max_by_key(|response| {
                (
                    response.attestations.len(),
                    response.attestations_for_block_number,
                )
            })
            .expect("this iterator should be non-empty, error cases have already quit early");

        Ok(best_attestations)
    }
}
