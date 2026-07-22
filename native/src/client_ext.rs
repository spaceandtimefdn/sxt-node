//! Extension giving offchain workers read access to the node's client.

/// Thin interface onto the node's client, implemented by the node service and registered
/// as a [`ClientExt`] so offchain workers can read from it.
pub trait ClientProvider: Send + Sync {
    /// The hash of the current finalized block.
    fn finalized_hash(&self) -> [u8; 32];
    /// The number of the current finalized block.
    fn finalized_number(&self) -> u32;
}

polkadot_sdk::sp_externalities::decl_extension! {
    /// Externalities extension exposing the node's client to offchain workers.
    pub struct ClientExt(std::sync::Arc<dyn ClientProvider>);
}
