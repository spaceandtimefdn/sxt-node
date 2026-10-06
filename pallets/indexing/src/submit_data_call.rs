use codec::Encode;
use native_api::NativeApi;
use polkadot_sdk::frame_support::dispatch::DispatchInfo;
use polkadot_sdk::pallet_transaction_payment::{self, OnChargeTransaction};
use polkadot_sdk::sp_runtime::traits::Dispatchable;
use sxt_core::tables::TableIdentifier;

use crate::{BatchId, Call, Config, RowData};

/// The arguments of a [`Call::submit_data`] or [`Call::submit_blockchain_data`].
#[allow(non_camel_case_types)]
pub(crate) enum SubmitDataCall {
    /// The arguments of [`Call::submit_data`].
    submit_data {
        /// The table to submit to.
        table: TableIdentifier,
        /// The batch id as submitted.
        batch_id: BatchId,
        /// The submitted record batch.
        data: RowData,
    },
    /// The arguments of [`Call::submit_blockchain_data`].
    submit_blockchain_data {
        /// The table to submit to.
        table: TableIdentifier,
        /// The batch id as submitted.
        batch_id: BatchId,
        /// The submitted record batch.
        data: RowData,
        /// The block number of the submitted data.
        block_number: u64,
    },
}

impl<T, I> From<SubmitDataCall> for Call<T, I>
where
    T: Config<I>,
    I: NativeApi,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
    <T as pallet_transaction_payment::Config>::OnChargeTransaction:
        OnChargeTransaction<T, Balance = T::Balance>,
{
    fn from(call: SubmitDataCall) -> Self {
        match call {
            SubmitDataCall::submit_data {
                table,
                batch_id,
                data,
            } => Call::submit_data {
                table,
                batch_id,
                data,
            },
            SubmitDataCall::submit_blockchain_data {
                table,
                batch_id,
                data,
                block_number,
            } => Call::submit_blockchain_data {
                table,
                batch_id,
                data,
                block_number,
            },
        }
    }
}

/// The parts of a [`SubmitDataCall`], plus the encoded length of the equivalent `RuntimeCall`.
#[derive(Debug, PartialEq)]
pub(crate) struct SubmitDataParts {
    /// The table to submit to.
    pub table: TableIdentifier,
    /// The batch id as submitted.
    pub batch_id: BatchId,
    /// The submitted record batch.
    pub data: RowData,
    /// The block number of the submitted data, if any.
    pub block_number: Option<u64>,
    /// The encoded length of the equivalent `RuntimeCall`.
    pub len: u32,
}

impl SubmitDataCall {
    /// Returns the [`SubmitDataParts`] of this call.
    pub(crate) fn into_parts<T, I>(self) -> SubmitDataParts
    where
        T: Config<I>,
        I: NativeApi,
        T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
        <T as pallet_transaction_payment::Config>::OnChargeTransaction:
            OnChargeTransaction<T, Balance = T::Balance>,
    {
        let call = Call::<T, I>::from(self);
        // RuntimeCall encoding also includes the pallet index byte.
        let len = (1 + call.encoded_size()) as u32;
        match call {
            Call::submit_data {
                table,
                batch_id,
                data,
            } => SubmitDataParts {
                table,
                batch_id,
                data,
                block_number: None,
                len,
            },
            Call::submit_blockchain_data {
                table,
                batch_id,
                data,
                block_number,
            } => SubmitDataParts {
                table,
                batch_id,
                data,
                block_number: Some(block_number),
                len,
            },
            _ => unreachable!("constructed from a SubmitDataCall"),
        }
    }
}

#[cfg(test)]
mod tests {
    use native_api::Api;
    use sxt_core::tables::{TableName, TableNamespace};

    use super::*;
    use crate::mock::{RuntimeCall, Test};

    #[test]
    fn submit_data_into_parts_matches_runtime_call() {
        let table = TableIdentifier {
            namespace: TableNamespace::try_from(b"NAMESPACE".to_vec()).unwrap(),
            name: TableName::try_from(b"TABLE".to_vec()).unwrap(),
        };
        let batch_id = BatchId::try_from(b"batch".to_vec()).unwrap();
        let data = RowData::try_from(vec![1, 2, 3]).unwrap();
        let expected_len = RuntimeCall::from(Call::<Test, Api>::submit_data {
            table: table.clone(),
            batch_id: batch_id.clone(),
            data: data.clone(),
        })
        .encoded_size() as u32;

        assert_eq!(
            SubmitDataCall::submit_data {
                table: table.clone(),
                batch_id: batch_id.clone(),
                data: data.clone(),
            }
            .into_parts::<Test, Api>(),
            SubmitDataParts {
                table,
                batch_id,
                data,
                block_number: None,
                len: expected_len,
            }
        );
    }

    #[test]
    fn submit_blockchain_data_into_parts_matches_runtime_call() {
        let table = TableIdentifier {
            namespace: TableNamespace::try_from(b"NAMESPACE".to_vec()).unwrap(),
            name: TableName::try_from(b"TABLE".to_vec()).unwrap(),
        };
        let batch_id = BatchId::try_from(b"batch".to_vec()).unwrap();
        let data = RowData::try_from(vec![1, 2, 3]).unwrap();
        let expected_len = RuntimeCall::from(Call::<Test, Api>::submit_blockchain_data {
            table: table.clone(),
            batch_id: batch_id.clone(),
            data: data.clone(),
            block_number: 12345,
        })
        .encoded_size() as u32;

        assert_eq!(
            SubmitDataCall::submit_blockchain_data {
                table: table.clone(),
                batch_id: batch_id.clone(),
                data: data.clone(),
                block_number: 12345,
            }
            .into_parts::<Test, Api>(),
            SubmitDataParts {
                table,
                batch_id,
                data,
                block_number: Some(12345),
                len: expected_len,
            }
        );
    }
}
