//! Implements [`ClientProvider`](native::client::ClientProvider) for
//! thin wrapper around [`FullClient`].

use std::sync::Arc;

use polkadot_sdk::sc_client_api::{HeaderBackend, StorageProvider};
use polkadot_sdk::sp_blockchain;
use polkadot_sdk::sp_core::storage::{StorageData, StorageKey};
use polkadot_sdk::sp_core::H256;

use crate::service::FullClient;

/// A handle onto [`FullClient`] used to implement
/// [`ClientProvider`](native::client::ClientProvider) for it.
pub(crate) struct FullClientHandle(pub(crate) Arc<FullClient>);

impl native::client::ClientProvider for FullClientHandle {
    fn finalized_hash(&self) -> [u8; 32] {
        self.0.info().finalized_hash.into()
    }

    fn finalized_number(&self) -> u32 {
        self.0.info().finalized_number
    }

    fn storage(&self, hash: H256, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>> {
        self.0.storage(hash, key)
    }
}
