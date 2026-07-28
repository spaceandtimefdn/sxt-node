//! Extension giving offchain workers read access to the node's client.

use alloc::string::String;

use polkadot_sdk::sp_core::storage::{StorageData, StorageKey};
use polkadot_sdk::sp_core::H256;
use polkadot_sdk::sp_externalities::ExternalitiesExt;
pub use polkadot_sdk::sp_runtime_interface::runtime_interface;

/// Thin interface onto the node's client, implemented by the node service and registered
/// as a [`ClientExt`] so offchain workers can read from it.
#[cfg(feature = "std")]
pub trait ClientProvider: Send + Sync {
    /// Last finalized state.
    fn finalized_state(&self) -> Option<(H256, u32)>;
    /// Given a block's `Hash` and a key, return the value under the key in that block.
    fn storage(
        &self,
        hash: H256,
        key: &StorageKey,
    ) -> polkadot_sdk::sp_blockchain::Result<Option<StorageData>>;
}

#[cfg(feature = "std")]
polkadot_sdk::sp_externalities::decl_extension! {
    /// Externalities extension exposing the node's client to offchain workers.
    pub struct ClientExt(std::sync::Arc<dyn ClientProvider>);
}

/// Native code interface onto the node's client, exposed to offchain workers
/// via the [`ClientExt`] externalities extension.
#[runtime_interface]
pub trait Client {
    /// Last finalized state.
    ///
    /// Wraps [`ClientProvider::finalized_state`], which is modeled off of `FullClient::info().finalized_state`.
    /// Returns `None` if the [`ClientExt`] extension is not registered. Otherwise, returns `Some(FullClient::info().finalized_state)`.
    fn finalized_state(&mut self) -> Option<Option<(H256, u32)>> {
        ExternalitiesExt::extension(self).map(|ClientExt(provider)| provider.finalized_state())
    }

    /// Given a block's `Hash` and a key, return the value under the key in that block.
    ///
    /// Wraps [`ClientProvider::storage`], which is modeled off of `FullClient::storage`.
    /// Returns `None` if the [`ClientExt`] extension is not registered. Otherwise, returns `Some(FullClient::storage(hash, key))`.
    fn storage(
        &mut self,
        hash: H256,
        key: alloc::vec::Vec<u8>,
    ) -> Option<Result<Option<StorageData>, String>> {
        ExternalitiesExt::extension(self).map(|ClientExt(provider)| {
            provider
                .storage(hash, &StorageKey(key))
                .map_err(|error| error.to_string())
        })
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use std::sync::Arc;

    use polkadot_sdk::sp_io::TestExternalities;

    use super::*;

    struct MockClientProvider {
        finalized_state: Option<(H256, u32)>,
        storage_response: Result<Option<StorageData>, String>,
    }

    impl ClientProvider for MockClientProvider {
        fn finalized_state(&self) -> Option<(H256, u32)> {
            self.finalized_state
        }

        fn storage(
            &self,
            _hash: H256,
            _key: &StorageKey,
        ) -> polkadot_sdk::sp_blockchain::Result<Option<StorageData>> {
            self.storage_response
                .clone()
                .map_err(polkadot_sdk::sp_blockchain::Error::UnknownBlock)
        }
    }

    #[test]
    fn finalized_state_is_none_when_extension_not_registered() {
        let mut ext = TestExternalities::default();
        ext.execute_with(|| {
            assert_eq!(client::finalized_state(), None);
        });
    }

    #[test]
    fn storage_is_none_when_extension_not_registered() {
        let mut ext = TestExternalities::default();
        ext.execute_with(|| {
            assert_eq!(client::storage(H256::zero(), vec![123]), None);
        })
    }

    #[test]
    fn finalized_state_forwards_provider_value() {
        let mut ext = TestExternalities::default();
        ext.register_extension(ClientExt(Arc::new(MockClientProvider {
            finalized_state: Some((H256::repeat_byte(0xAB), 456)),
            storage_response: Ok(None),
        })));
        ext.execute_with(|| {
            assert_eq!(
                client::finalized_state(),
                Some(Some((H256::repeat_byte(0xAB), 456)))
            );
        });
    }

    #[test]
    fn storage_forwards_provider_value() {
        let mut ext = TestExternalities::default();
        ext.register_extension(ClientExt(Arc::new(MockClientProvider {
            finalized_state: None,
            storage_response: Ok(Some(StorageData(vec![107, 108, 109]))),
        })));
        ext.execute_with(|| {
            assert_eq!(
                client::storage(H256::zero(), vec![123]),
                Some(Ok(Some(StorageData(vec![107, 108, 109]))))
            );
        });
    }

    #[test]
    fn storage_forwards_provider_error_as_string() {
        let mut ext = TestExternalities::default();
        ext.register_extension(ClientExt(Arc::new(MockClientProvider {
            finalized_state: None,
            storage_response: Err("test error".to_string()),
        })));
        ext.execute_with(|| {
            assert_eq!(
                client::storage(H256::zero(), vec![123]),
                Some(Err("UnknownBlock: test error".to_string()))
            );
        });
    }
}
