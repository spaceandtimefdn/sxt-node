//! Implements [`ClientProvider`](native::client_ext::ClientProvider) for
//! thin wrapper around [`FullClient`].

use std::sync::Arc;

use polkadot_sdk::sc_client_api::HeaderBackend;

use crate::service::FullClient;

/// A handle onto [`FullClient`] used to implement
/// [`ClientProvider`](native::client_ext::ClientProvider) for it.
pub(crate) struct FullClientHandle(pub(crate) Arc<FullClient>);

impl native::client_ext::ClientProvider for FullClientHandle {
    fn finalized_hash(&self) -> [u8; 32] {
        self.0.info().finalized_hash.into()
    }

    fn finalized_number(&self) -> u32 {
        self.0.info().finalized_number
    }
}
