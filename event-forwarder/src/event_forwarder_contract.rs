#![allow(clippy::too_many_arguments)]
#![allow(missing_docs)]

use alloy::sol;

sol!(
    /// event forwarder contract
    #[sol(rpc)]
    EventForwarder,
    "artifacts/EventForwarder.json"
);
