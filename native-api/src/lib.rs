//! The declaration for the native trait that can be adhered to by pallets that want to execute std code.
#![cfg_attr(not(feature = "std"), no_std)]

use proof_of_sql_commitment_map::TableCommitmentBytesPerCommitmentSchemePassBy;
use sxt_core::native::{NativeCommitmentError, NativeError, OnChainTableBytes, RowData};
use sxt_core::tables::TableIdentifier;

/// The native api that our pallets can adhere to.
/// The inputs and output to these types need to implement the `PassByCode` trait.
pub trait NativeApi: 'static {
    /// Convert row_data to a serialized OnChainTable
    fn record_batch_to_onchain(row_data: RowData) -> Result<OnChainTableBytes, NativeError>;

    /// Process insert to support commitment metadata.
    ///
    /// Returns..
    /// - the processed insert data with comitment metadata
    /// - the updated commitments for the table
    fn process_insert(
        table_identifier: TableIdentifier,
        insert_data_bytes: OnChainTableBytes,
        previous_commitments_bytes: TableCommitmentBytesPerCommitmentSchemePassBy,
    ) -> Result<
        (
            OnChainTableBytes,
            TableCommitmentBytesPerCommitmentSchemePassBy,
        ),
        NativeCommitmentError,
    >;
}

/// Needed for type checks in pallets, if adding new functions to the NativeApi they will need to be implemented here.
impl NativeApi for () {
    fn record_batch_to_onchain(_row_data: RowData) -> Result<OnChainTableBytes, NativeError> {
        unimplemented!()
    }

    fn process_insert(
        _table_identifier: TableIdentifier,
        _insert_data_bytes: OnChainTableBytes,
        _previous_commitments_bytes: TableCommitmentBytesPerCommitmentSchemePassBy,
    ) -> Result<
        (
            OnChainTableBytes,
            TableCommitmentBytesPerCommitmentSchemePassBy,
        ),
        NativeCommitmentError,
    > {
        unimplemented!()
    }
}

/// Actual NativeApi implementation that uses runtime_interface functions.
pub struct Api;

impl NativeApi for Api {
    fn record_batch_to_onchain(row_data: RowData) -> Result<OnChainTableBytes, NativeError> {
        native::interface::record_batch_to_onchain(row_data)
    }

    fn process_insert(
        table_identifier: TableIdentifier,
        insert_data_bytes: OnChainTableBytes,
        previous_commitments_bytes: TableCommitmentBytesPerCommitmentSchemePassBy,
    ) -> Result<
        (
            OnChainTableBytes,
            TableCommitmentBytesPerCommitmentSchemePassBy,
        ),
        NativeCommitmentError,
    > {
        native::interface::process_insert(
            table_identifier,
            insert_data_bytes,
            previous_commitments_bytes,
        )
    }
}
