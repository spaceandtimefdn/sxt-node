//! The declaration for the native trait that can be adhered to by pallets that want to execute std code.
#![cfg_attr(not(feature = "std"), no_std)]

use sxt_core::native::{CreateStatementPassBy, NativeError, OnChainTableBytes, RowData};
use sxt_core::tables::CreateStatement;

/// The native api that our pallets can adhere to.
/// The inputs and output to these types need to implement the `PassByCode` trait.
pub trait NativeApi: 'static {
    /// Convert row_data to a serialized OnChainTable
    fn record_batch_to_onchain(
        row_data: RowData,
        create_statement: CreateStatementPassBy,
    ) -> Result<OnChainTableBytes, NativeError>;
}

/// Needed for type checks in pallets, if adding new functions to the NativeApi they will need to be implemented here.
impl NativeApi for () {
    fn record_batch_to_onchain(
        row_data: RowData,
        create_statement: CreateStatementPassBy,
    ) -> Result<OnChainTableBytes, NativeError> {
        unimplemented!()
    }
}
