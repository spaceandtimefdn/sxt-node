//! Implements [`ClientProvider`] for a thin wrapper around [`FullClient`].

use std::sync::Arc;

use native::client::ClientProvider;
use polkadot_sdk::sc_client_api::{HeaderBackend, StorageProvider};
use polkadot_sdk::sp_blockchain;
use polkadot_sdk::sp_core::storage::{StorageData, StorageKey};
use polkadot_sdk::sp_core::H256;

use crate::service::FullClient;

/// A handle onto [`FullClient`] used to implement [`ClientProvider`] for it.
pub(crate) struct FullClientHandle(pub(crate) Arc<FullClient>);

impl ClientProvider for FullClientHandle {
    fn finalized_state(&self) -> Option<(H256, u32)> {
        self.0.info().finalized_state
    }

    fn storage(&self, hash: H256, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>> {
        self.0.storage(hash, key)
    }
}
