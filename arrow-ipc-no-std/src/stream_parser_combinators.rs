//! Parser combinators for parsing some arrow IPC streams.
//!
//! See [the arrow documentation](https://arrow.apache.org/docs/format/Columnar.html#serialization-and-interprocess-communication-ipc)
//! for more information on the larger stream format the surrounds the flatbuffers types.
use core::num::TryFromIntError;

use snafu::{AsErrorSource, Snafu};

use crate::generated::org::apache::arrow::flatbuf::{
    root_as_message,
    Message,
    RecordBatch,
    Schema,
};

/// Bytes used to indicate the continuation of an arrow IPC stream.
const CONTINUATION_MARKER: [u8; 4] = [255, 255, 255, 255];

/// Bytes used to indicate the end of an arrow IPC stream.
const END_OF_STREAM_MARKER: [u8; 8] = [255, 255, 255, 255, 0, 0, 0, 0];

/// Representation of the continuation markers that appear in arrow IPC stream.
struct Continuation;

/// Expected a continuation marker.
#[derive(Snafu, Debug)]
#[snafu(display("expected a continuation marker"))]
pub struct ExpectedContinuation;

/// Parser combinator for the continuation of an arrow IPC stream.
fn continuation_parser(input: &[u8]) -> Result<(&[u8], Continuation), ExpectedContinuation> {
    let (continuation_bytes, input) = input.split_at_checked(4).ok_or(ExpectedContinuation)?;

    if continuation_bytes == CONTINUATION_MARKER {
        Ok((input, Continuation))
    } else {
        Err(ExpectedContinuation)
    }
}

/// Expected a little-endian u32.
#[derive(Snafu, Debug)]
#[snafu(display("expected a little-endian u32"))]
pub struct ExpectedLeU32;

/// Parser combinator for u32s.
fn le_u32_parser(input: &[u8]) -> Result<(&[u8], u32), ExpectedLeU32> {
    let (u32_bytes, input) = input.split_at_checked(4).ok_or(ExpectedLeU32)?;

    let u32_array: [u8; 4] = u32_bytes.try_into().map_err(|_| ExpectedLeU32)?;

    Ok((input, u32::from_le_bytes(u32_array)))
}

/// Errors that can occur when trying to parse a message.
#[derive(Snafu, Debug)]
pub enum ArrowMessageParseError {
    /// Failed to parse continuation marker.
    #[snafu(transparent)]
    Continuation {
        /// The source continuation error.
        source: ExpectedContinuation,
    },
    /// Failed to parse u32 (metadata length).
    #[snafu(transparent)]
    U32 {
        /// The source u32 parser error.
        source: ExpectedLeU32,
    },
    /// Failed to parse message as flatbuffers.
    #[snafu(display("failed to parse message flatbuffers"))]
    Flatbuffers,
    /// Sum of length of metadata and body out of usize bounds.
    #[snafu(display("sum of length of metadata and body out of usize bounds"))]
    LengthOutOfUsizeBounds,
    /// Input too short to contain message.
    #[snafu(display("input too short to contain message"))]
    Incomplete,
}

impl From<TryFromIntError> for ArrowMessageParseError {
    fn from(_: TryFromIntError) -> Self {
        ArrowMessageParseError::LengthOutOfUsizeBounds
    }
}

/// Parser combinator for arrow IPC messages.
fn message_parser<'a>(input: &'a [u8]) -> Result<(&'a [u8], Message<'a>), ArrowMessageParseError> {
    let (input, _) = continuation_parser(input)?;
    let (input, metadata_len) = le_u32_parser(input)?;
    let message = root_as_message(input).map_err(|_| ArrowMessageParseError::Flatbuffers)?;

    let metadata_length_usize = usize::try_from(metadata_len)?;
    let body_length_usize = usize::try_from(message.bodyLength())?;

    let message_length = metadata_length_usize
        .checked_add(body_length_usize)
        .ok_or(ArrowMessageParseError::LengthOutOfUsizeBounds)?;

    let (_, input) = input
        .split_at_checked(message_length)
        .ok_or(ArrowMessageParseError::Incomplete)?;

    Ok((input, message))
}

/// Representation of the end of stream marker that appears at the end of arrow IPC streams.
struct EndOfStream;

/// Expected an end-of-stream marker.
#[derive(Snafu, Debug)]
#[snafu(display("expected an end-of-stream marker"))]
struct ExpectedEos;

/// Parser combinator for arrow IPC EOS.
fn end_of_stream_parser(input: &[u8]) -> Result<(&[u8], EndOfStream), ExpectedEos> {
    let (end_of_stream_bytes, input) = input.split_at_checked(8).ok_or(ExpectedEos)?;

    if end_of_stream_bytes == END_OF_STREAM_MARKER {
        Ok((input, EndOfStream))
    } else {
        Err(ExpectedEos)
    }
}

/// Parser combinator that makes another parser combinator optional.
fn maybe_parser<'a, T, E>(
    f: impl Fn(&'a [u8]) -> Result<(&'a [u8], T), E>,
) -> impl Fn(&'a [u8]) -> (&'a [u8], Option<T>) {
    move |input| {
        f(input)
            .map(|(rem, t)| (rem, Some(t)))
            .unwrap_or_else(|_| (input, None))
    }
}

/// Errors that can occur when parsing an arrow schema.
#[derive(Snafu, Debug)]
pub enum ArrowSchemaParseError {
    /// Failed to parse arrow message.
    #[snafu(display("failed to parse arrow message: {source}"), context(false))]
    ArrowMessageParse {
        /// The source message parser error.
        source: ArrowMessageParseError,
    },
    /// Expected message to be a schema.
    #[snafu(display("expected message to be a schema."))]
    ExpectedSchema,
}

fn schema_parser<'a>(input: &'a [u8]) -> Result<(&'a [u8], Schema<'a>), ArrowSchemaParseError> {
    let (input, message) = message_parser(input)?;

    let schema = message
        .header_as_schema()
        .ok_or(ArrowSchemaParseError::ExpectedSchema)?;

    Ok((input, schema))
}

/// Errors that can occur when parsing an arrow record batch.
#[derive(Snafu, Debug)]
pub enum ArrowRecordBatchParseError {
    /// Failed to parse arrow message.
    #[snafu(display("failed to parse arrow message: {source}"), context(false))]
    ArrowMessageParse {
        /// The source message parser error.
        source: ArrowMessageParseError,
    },
    /// Expected message to be a record batch.
    #[snafu(display("expected message to be a record batch."))]
    ExpectedRecordBatch,
}

fn record_batch_parser<'a>(
    input: &'a [u8],
) -> Result<(&'a [u8], RecordBatch<'a>), ArrowRecordBatchParseError> {
    let (input, message) = message_parser(input)?;

    let record_batch = message
        .header_as_record_batch()
        .ok_or(ArrowRecordBatchParseError::ExpectedRecordBatch)?;

    Ok((input, record_batch))
}

/// Errors that can occur when parsing a single record batch stream.
#[derive(Debug, Snafu)]
pub enum SingleBatchStreamParseError {
    /// Failed to parse schema.
    #[snafu(display("failed to parse schema: {source}"), context(false))]
    Schema {
        /// The source schema parser error.
        source: ArrowSchemaParseError,
    },
    /// Failed to parse record batch.
    #[snafu(display("failed to parse record batch: {source}"), context(false))]
    RecordBatch {
        /// The source batch parser error.
        source: ArrowRecordBatchParseError,
    },
}

/// The simplest record-batch-containing arrow IPC stream.
///
/// Can be parsed with [`single_batch_stream_parser`].
pub struct SingleBatchStream<'a> {
    /// Schema message of the stream.
    pub schema: Schema<'a>,
    /// Record batch message of the stream.
    pub record_batch: RecordBatch<'a>,
}

/// Parser combinator for a arrow stream with a single record batch.
pub fn single_batch_stream_parser<'a>(
    input: &'a [u8],
) -> Result<(&'a [u8], SingleBatchStream<'a>), SingleBatchStreamParseError> {
    let (input, schema) = schema_parser(input)?;
    let (input, record_batch) = record_batch_parser(input)?;
    let (input, _) = maybe_parser(end_of_stream_parser)(input);

    Ok((
        input,
        SingleBatchStream {
            schema,
            record_batch,
        },
    ))
}

/// Errors that can occur when expecting a parser to finish.
#[derive(Debug, Snafu)]
pub enum FinishParseError<E>
where
    E: AsErrorSource + core::fmt::Display,
{
    /// Failed to parse.
    #[snafu(display("failed to parse: {source}"), context(false))]
    Parse {
        /// The source parser error
        source: E,
    },
    /// Parser did not finish reading input.
    #[snafu(display("parser did not finish reading input"))]
    Unfinished,
}

/// Takes a parser combinator and requires that it parses the entire input.
pub fn finish<'a, T, E>(
    f: impl Fn(&'a [u8]) -> Result<(&'a [u8], T), E>,
) -> impl Fn(&'a [u8]) -> Result<T, FinishParseError<E>>
where
    E: AsErrorSource + core::fmt::Display,
{
    move |input| {
        let (input, t) = f(input)?;

        if input.is_empty() {
            Ok(t)
        } else {
            Err(FinishParseError::Unfinished)
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use arrow::datatypes::DataType;
    use on_chain_table::proptest::{
        on_chain_table,
        on_chain_table_compatible_record_batch,
        proof_of_sql_schema,
    };
    use proptest::prelude::*;

    use super::*;
    use crate::generated::org::apache::arrow::flatbuf::Type;
    use crate::single_batch_stream_bytes;

    fn up_to_32_record_batch() -> impl Strategy<Value = arrow::array::RecordBatch> {
        on_chain_table_compatible_record_batch(on_chain_table(
            proof_of_sql_schema(1..32usize),
            0..32usize,
        ))
    }

    fn dummy_message_with_at_least_metadata_length<L>(length: L) -> impl Strategy<Value = Vec<u8>>
    where
        L: Strategy<Value = u32>,
    {
        length
            .prop_flat_map(|length| {
                (
                    Just(length),
                    proptest::collection::vec(any::<u8>(), length as usize..=length as usize * 2),
                )
            })
            .prop_map(|(length, metadata_bytes)| {
                std::iter::chain(CONTINUATION_MARKER, length.to_le_bytes())
                    .chain(metadata_bytes)
                    .collect()
            })
    }

    proptest! {
        #[test]
        fn single_batch_stream_parsed_batch_has_correct_data(record_batch in up_to_32_record_batch()) {
            let record_batch_bytes = single_batch_stream_bytes(&record_batch).unwrap();

            let parsed_stream = finish(single_batch_stream_parser)(&record_batch_bytes).unwrap();

            assert_eq!(parsed_stream.schema.fields().unwrap().iter().count(), record_batch.num_columns());

            parsed_stream.schema.fields().unwrap().iter().zip(record_batch.schema().fields()).for_each(|(parsed_field, expected_field)| {
                assert_eq!(parsed_field.name().unwrap(), expected_field.name());

                match (parsed_field.type_type(), expected_field.data_type()) {
                    (Type::Bool, DataType::Boolean) => (),
                    (Type::Int, DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64) => (),
                    (Type::Utf8, DataType::Utf8) => (),
                    (Type::Binary, DataType::Binary) => (),
                    (Type::Timestamp, DataType::Timestamp(_, _)) => (),
                    (Type::Decimal, DataType::Decimal256(_, _)) => (),
                    (parsed_type, expected_type) => panic!("{:?} != {expected_type} or case is not covered", parsed_type.variant_name()),
                }
            });

            assert_eq!(parsed_stream.record_batch.length() as usize, record_batch.num_rows());

        }

        #[test]
        fn continuation_parser_does_not_panic(bytes in proptest::collection::vec(any::<u8>(), 0..16)) {
            let _no_panic = continuation_parser(&bytes);
        }

        #[test]
        fn le_u32_parser_does_not_panic(bytes in proptest::collection::vec(any::<u8>(), 0..16)) {
            let _no_panic = le_u32_parser(&bytes);
        }

        #[test]
        fn message_parser_does_not_panic_past_length(
            bytes in dummy_message_with_at_least_metadata_length(0..512u32)
        ) {
            let no_panic = message_parser(&bytes);

            assert!(
                !matches!(
                    no_panic,
                    Err(
                        ArrowMessageParseError::Continuation { .. }
                        | ArrowMessageParseError::U32 { .. }
                    )
                )
            );
        }

        #[test]
        fn schema_parser_does_not_panic(
            bytes in dummy_message_with_at_least_metadata_length(0..512u32)
        ) {
            let _no_panic = schema_parser(&bytes);
        }

        #[test]
        fn record_batch_parser_does_not_panic(
            bytes in dummy_message_with_at_least_metadata_length(0..512u32)
        ) {
            let _no_panic = record_batch_parser(&bytes);
        }

        #[test]
        fn end_of_stream_parser_does_not_panic(bytes in proptest::collection::vec(any::<u8>(), 0..16)) {
            let _no_panic = end_of_stream_parser(&bytes);
        }

        #[test]
        fn single_batch_stream_parser_does_not_panic(
            schema_message in dummy_message_with_at_least_metadata_length(0..512u32),
            batch_message in dummy_message_with_at_least_metadata_length(0..512u32),
            include_end_of_stream in any::<bool>(),
        ) {

            let end_of_stream = include_end_of_stream
                .then_some(END_OF_STREAM_MARKER)
                .into_iter()
                .flatten();

            let bytes = std::iter::chain(schema_message, batch_message)
                .chain(end_of_stream)
                .collect::<Vec<_>>();

            let _no_panic = single_batch_stream_parser(&bytes);
        }
    }

    #[test]
    fn we_can_parse_actual_nft_erc1155_contracts_insert() {
        let real_batch_bytes = include_bytes!("../test-data/ETHEREUM.NFT_ERC1155_CONTRACTS.arrow");

        let expected_schema = [
            ("TIME_STAMP", Type::Timestamp),
            ("BLOCK_NUMBER", Type::Int),
            ("TRANSACTION_HASH", Type::Binary),
            ("TRANSACTION_INDEX", Type::Int),
            ("CONTRACT_ADDRESS", Type::Binary),
        ];

        let parsed_stream = finish(single_batch_stream_parser)(real_batch_bytes).unwrap();
        assert_eq!(
            parsed_stream.schema.fields().unwrap().iter().count(),
            expected_schema.len()
        );

        parsed_stream
            .schema
            .fields()
            .unwrap()
            .iter()
            .zip(expected_schema)
            .for_each(|(parsed_field, (expected_name, expected_type))| {
                assert_eq!(parsed_field.name().unwrap().to_uppercase(), expected_name);

                assert_eq!(parsed_field.type_type(), expected_type);
            });

        assert_eq!(parsed_stream.record_batch.length() as usize, 10);
    }

    #[test]
    fn we_can_parse_actual_native_wallets_contracts_insert() {
        let real_batch_bytes = include_bytes!("../test-data/ETHEREUM.NATIVE_WALLETS.arrow");

        let expected_schema = [
            ("TIME_STAMP", Type::Timestamp),
            ("BLOCK_NUMBER", Type::Int),
            ("WALLET_ADDRESS", Type::Binary),
            ("BALANCE", Type::Decimal),
        ];

        let parsed_stream = finish(single_batch_stream_parser)(real_batch_bytes).unwrap();
        assert_eq!(
            parsed_stream.schema.fields().unwrap().iter().count(),
            expected_schema.len()
        );

        parsed_stream
            .schema
            .fields()
            .unwrap()
            .iter()
            .zip(expected_schema)
            .for_each(|(parsed_field, (expected_name, expected_type))| {
                assert_eq!(parsed_field.name().unwrap().to_uppercase(), expected_name);

                assert_eq!(parsed_field.type_type(), expected_type);
            });

        assert_eq!(parsed_stream.record_batch.length() as usize, 447);
    }
}
