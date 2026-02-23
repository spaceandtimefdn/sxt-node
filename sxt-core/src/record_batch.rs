use arrow_ipc_no_std::{
    finish,
    single_batch_stream_parser,
    FinishParseError,
    SingleBatchStreamParseError,
};
use snafu::Snafu;

/// Errors that can occur in [`record_batch_bytes_row_count`].
#[derive(Debug, Snafu)]
pub enum RecordBatchBytesRowCountError {
    /// Unable to parse record batch.
    #[snafu(
        display("unable to parse record batch stream: {source}"),
        context(false)
    )]
    Parse {
        /// The source parser error.
        source: FinishParseError<SingleBatchStreamParseError>,
    },
    /// Claimed row count in record batch bytes out of u32 bounds.
    #[snafu(display("claimed row count in record batch bytes out of u32 bounds"))]
    OutOfU32Bounds,
}

/// Extract the row count from the given record batch bytes (single-batch arrow IPC stream).
pub fn record_batch_bytes_row_count(
    record_batch_bytes: &[u8],
) -> Result<u32, RecordBatchBytesRowCountError> {
    let single_batch_stream = finish(single_batch_stream_parser)(record_batch_bytes)?;
    single_batch_stream
        .record_batch
        .length()
        .try_into()
        .map_err(|_| RecordBatchBytesRowCountError::OutOfU32Bounds)
}

#[cfg(all(test, feature = "proptest"))]
mod tests {
    use arrow_ipc_no_std::single_batch_stream_bytes;
    use proptest::prelude::*;

    use super::*;
    use crate::proptest::{canonical_record_batch, DataCorruption};

    proptest! {
        // generating the (up to) 64x64 record batches for these tests is a bit slow
        // Default value for this is 256, so this halves the test time and test coverage.
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn record_batch_bytes_row_count_is_correct(record_batch in canonical_record_batch()) {
            let expected_row_count = record_batch.num_rows();

            let record_batch_bytes = single_batch_stream_bytes(&record_batch).unwrap();

            let row_count = record_batch_bytes_row_count(&record_batch_bytes).unwrap();

            assert_eq!(row_count, expected_row_count as u32);
        }

        #[test]
        fn random_record_batch_bytes_row_count_does_not_panic(row_data in proptest::collection::vec(any::<u8>(), 0..1024)) {
            let _no_panic = record_batch_bytes_row_count(&row_data);
        }

        #[test]
        fn corrupted_record_batch_bytes_row_count_does_not_panic(
            record_batch in canonical_record_batch(),
            corruptions in proptest::collection::vec(any::<DataCorruption>(), 0..1024)
        ) {
            let record_batch_bytes = single_batch_stream_bytes(&record_batch).unwrap();
            let corrupted_bytes = corruptions.iter().fold(record_batch_bytes, |data, corruption| DataCorruption::corrupt(corruption, data));
            let _no_panic = record_batch_bytes_row_count(&corrupted_bytes);
        }
    }

    #[test]
    fn we_can_extract_row_count_from_actual_nft_erc1155_contracts_insert() {
        let real_batch_bytes =
            include_bytes!("../../arrow-ipc-no-std/test-data/ETHEREUM.NFT_ERC1155_CONTRACTS.arrow");

        let row_count = record_batch_bytes_row_count(real_batch_bytes).unwrap();

        assert_eq!(row_count, 10);
    }

    #[test]
    fn we_can_extract_row_count_from_actual_native_wallets_insert() {
        let real_batch_bytes =
            include_bytes!("../../arrow-ipc-no-std/test-data/ETHEREUM.NATIVE_WALLETS.arrow");

        let row_count = record_batch_bytes_row_count(real_batch_bytes).unwrap();

        assert_eq!(row_count, 447);
    }
}
