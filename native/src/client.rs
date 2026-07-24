//! Extension giving offchain workers read access to the node's client.
use alloc::string::ToString;

#[cfg(feature = "std")]
use polkadot_sdk::sp_blockchain;
use polkadot_sdk::sp_core::storage::{StorageData, StorageKey};
use polkadot_sdk::sp_core::H256;
pub use polkadot_sdk::sp_runtime_interface::runtime_interface;

/// Thin interface onto the node's client, implemented by the node service and registered
/// as a [`ClientExt`] so offchain workers can read from it.
#[cfg(feature = "std")]
pub trait ClientProvider: Send + Sync {
    /// The hash of the current finalized block.
    fn finalized_hash(&self) -> [u8; 32];
    /// The number of the current finalized block.
    fn finalized_number(&self) -> u32;
    /// Mirrors [`sc_client_api::StorageProvider::storage`](polkadot_sdk::sc_client_api::StorageProvider::storage),
    /// letting offchain workers read the value of any storage `key` as of any block `hash`.
    fn storage(&self, hash: H256, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>>;
}

#[cfg(feature = "std")]
polkadot_sdk::sp_externalities::decl_extension! {
    /// Externalities extension exposing the node's client to offchain workers.
    pub struct ClientExt(std::sync::Arc<dyn ClientProvider>);
}

/// Space and Time's native code interface onto the node's client, exposed to offchain workers
/// via the [`ClientExt`] externalities extension.
#[runtime_interface]
pub trait Client {
    /// Returns the current finalized block's number and hash, as reported by the node's
    /// [`ClientExt`], if one is registered.
    fn finalized_block(&mut self) -> Option<(u32, [u8; 32])> {
        polkadot_sdk::sp_externalities::ExternalitiesExt::extension(self)
            .map(|ClientExt(provider)| (provider.finalized_number(), provider.finalized_hash()))
    }

    /// Returns the raw storage value for `key` as of the block with the given hash, as reported
    /// by the node's [`ClientExt`], if one is registered. The outer `Option` is `None` when no
    /// `ClientExt` is registered; the inner `Result` mirrors the client's lookup, leaving it to
    /// the caller to decide how to handle a lookup error versus a present-but-empty vs. absent
    /// value.
    fn storage_at(
        &mut self,
        hash: H256,
        key: alloc::vec::Vec<u8>,
    ) -> Option<Result<Option<StorageData>, alloc::string::String>> {
        polkadot_sdk::sp_externalities::ExternalitiesExt::extension(self).map(
            |ClientExt(provider)| {
                provider
                    .storage(hash, &StorageKey(key))
                    .map_err(|error| error.to_string())
            },
        )
    }
}
