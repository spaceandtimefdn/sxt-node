use codec::{Decode, Encode, FullCodec};
use frame_support::traits::StorageInstance;
use snafu::Snafu;

use crate::HashAndKeyTuple;

/// Defines how to create leaves for a given storage prefix.
pub trait PrefixFoliate {
    /// The instance type for the pallet storage item (the hidden _GeneratedPrefixForStorage type).
    type StorageInstance: StorageInstance;

    /// The sequence of hashes and keys used in the storage item.
    type HashAndKeyTuple: HashAndKeyTuple;

    /// The value of the storage item.
    type Value: FullCodec;

    /// Returns the encoding of the strongly-typed storage key to be used in the leaf.
    ///
    /// Just uses its scale encoding by default.
    fn leaf_encode_key(key: <Self::HashAndKeyTuple as HashAndKeyTuple>::KeyTuple) -> Vec<u8> {
        key.encode()
    }

    /// Returns the encoding of the strongly-typed storage value to be used in the leaf.
    ///
    /// Just uses its scale encoding by default.
    fn leaf_encode_value(value: Self::Value) -> Vec<u8> {
        value.encode()
    }
}

/// Errors that can occur when decoding raw storage bytes.
#[derive(Snafu, Debug)]
pub enum DecodeStorageError {
    /// Storage key prefix doesn't match [`PrefixFoliate`] definition.
    #[snafu(display("storage key prefix doesn't match PrefixFoliate definition"))]
    UnexpectedStoragePrefix,
    /// Storage key bytes longer than expected.
    #[snafu(display("storage key bytes longer than expected"))]
    UnexpectedKeyBytes,
    /// Storage value bytes longer than expected.
    #[snafu(display("storage value bytes longer than expected"))]
    UnexpectedValueBytes,
    /// Unable to decode storage bytes.
    #[snafu(display("unable to decode storage bytes: {source}"), context(false))]
    Decode {
        /// The source scale codec error.
        source: codec::Error,
    },
}

/// Decodes a [`PrefixFoliate`]'s key and value from raw storage bytes.
pub fn decode_storage_key_and_value<PF>(
    key_bytes: &[u8],
    mut value_bytes: &[u8],
) -> Result<
    (
        <PF::HashAndKeyTuple as HashAndKeyTuple>::KeyTuple,
        PF::Value,
    ),
    DecodeStorageError,
>
where
    PF: PrefixFoliate,
{
    let expected_prefix = PF::StorageInstance::prefix_hash();
    let key_bytes = key_bytes
        .strip_prefix(&expected_prefix)
        .ok_or(DecodeStorageError::UnexpectedStoragePrefix)?;

    let (key_tuple, key_bytes) =
        PF::HashAndKeyTuple::decode_key_tuple_from_storage_key_suffix(key_bytes)?;
    if !key_bytes.is_empty() {
        return Err(DecodeStorageError::UnexpectedKeyBytes);
    }

    let value = PF::Value::decode(&mut value_bytes)?;
    if !value_bytes.is_empty() {
        return Err(DecodeStorageError::UnexpectedValueBytes);
    }

    Ok((key_tuple, value))
}

/// Encodes a leaf for a [`PrefixFoliate`] given a key and value.
pub fn encode_key_value_leaf<PF: PrefixFoliate>(
    key: <PF::HashAndKeyTuple as HashAndKeyTuple>::KeyTuple,
    value: PF::Value,
) -> Vec<u8> {
    PF::leaf_encode_key(key)
        .into_iter()
        .chain(PF::leaf_encode_value(value))
        .collect()
}

/// Returns the leaves for a [`PrefixFoliate`] given an iterator of raw storage key-value pairs.
pub fn encode_prefix_leaves<PF, I>(key_value_pairs: I) -> Result<Vec<Vec<u8>>, DecodeStorageError>
where
    PF: PrefixFoliate,
    I: IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
{
    key_value_pairs
        .into_iter()
        .map(|(key_bytes, value_bytes)| {
            decode_storage_key_and_value::<PF>(&key_bytes, &value_bytes)
                .map(|(key_tuple, value)| encode_key_value_leaf::<PF>(key_tuple, value))
        })
        .collect()
}

/// Returns the full storage key for the given strongly-typed key tuple.
pub fn storage_key_for_prefix_key_tuple<PF>(
    key_tuple: <PF::HashAndKeyTuple as HashAndKeyTuple>::KeyTuple,
) -> Vec<u8>
where
    PF: PrefixFoliate,
{
    PF::StorageInstance::prefix_hash()
        .into_iter()
        .chain(PF::HashAndKeyTuple::storage_key_suffix_from_key_tuple(
            key_tuple,
        ))
        .collect()
}

#[cfg(test)]
mod tests {
    use frame_support::Identity;

    use super::*;

    const TEST_PALLET_PREFIX: &str = "TestPallet";

    struct TestStorageInstance;

    impl StorageInstance for TestStorageInstance {
        fn pallet_prefix() -> &'static str {
            TEST_PALLET_PREFIX
        }

        const STORAGE_PREFIX: &str = "TestStorage";
    }

    struct TestPrefixFoliate;

    impl PrefixFoliate for TestPrefixFoliate {
        type StorageInstance = TestStorageInstance;
        type HashAndKeyTuple = ((Identity, bool),);
        type Value = bool;
    }

    #[test]
    fn we_can_encode_key_and_value_leaf() {
        let key = (false,);
        let value = true;

        let encoded = encode_key_value_leaf::<TestPrefixFoliate>(key, value);

        assert_eq!(encoded, vec![0u8, 1u8]);
    }

    fn valid_storage_key_and_value() -> (Vec<u8>, Vec<u8>) {
        let storage_prefix = TestStorageInstance::prefix_hash().to_vec();
        let storage_suffix = vec![1u8];

        let storage_key = storage_prefix
            .into_iter()
            .chain(storage_suffix)
            .collect::<Vec<_>>();

        let storage_value = vec![0u8];

        (storage_key, storage_value)
    }

    #[test]
    fn we_can_decode_storage_key_and_value() {
        let (storage_key, storage_value) = valid_storage_key_and_value();

        let (decoded_key, decoded_value) =
            decode_storage_key_and_value::<TestPrefixFoliate>(&storage_key, &storage_value)
                .unwrap();

        assert_eq!(decoded_key, (true,));
        assert!(!decoded_value);
    }

    #[test]
    fn we_cannot_decode_storage_key_with_bad_prefix() {
        let (storage_key, storage_value) = valid_storage_key_and_value();

        let storage_key_with_bad_prefix = storage_key
            .iter()
            .cloned()
            .take(1)
            .map(|byte| 255 - byte)
            .chain(storage_key.iter().cloned().skip(1))
            .collect::<Vec<_>>();

        let result = decode_storage_key_and_value::<TestPrefixFoliate>(
            &storage_key_with_bad_prefix,
            &storage_value,
        );

        assert!(matches!(
            result,
            Err(DecodeStorageError::UnexpectedStoragePrefix)
        ));
    }

    #[test]
    fn we_cannot_decode_storage_key_with_too_many_bytes() {
        let (storage_key, storage_value) = valid_storage_key_and_value();

        let excessive_storage_key = storage_key
            .into_iter()
            .chain(std::iter::once(0u8))
            .collect::<Vec<_>>();

        let result = decode_storage_key_and_value::<TestPrefixFoliate>(
            &excessive_storage_key,
            &storage_value,
        );

        assert!(matches!(
            result,
            Err(DecodeStorageError::UnexpectedKeyBytes)
        ));
    }

    #[test]
    fn we_cannot_decode_value_with_too_many_bytes() {
        let (storage_key, storage_value) = valid_storage_key_and_value();

        let excessive_storage_value = storage_value
            .into_iter()
            .chain(std::iter::once(0u8))
            .collect::<Vec<_>>();

        let result = decode_storage_key_and_value::<TestPrefixFoliate>(
            &storage_key,
            &excessive_storage_value,
        );

        assert!(matches!(
            result,
            Err(DecodeStorageError::UnexpectedValueBytes)
        ));
    }

    #[test]
    fn we_cannot_decode_invalid_key() {
        let (storage_key, storage_value) = valid_storage_key_and_value();

        let invalid_key = storage_key
            .into_iter()
            .take(32)
            .chain(std::iter::once(2u8))
            .collect::<Vec<_>>();

        let result =
            decode_storage_key_and_value::<TestPrefixFoliate>(&invalid_key, &storage_value);

        assert!(matches!(result, Err(DecodeStorageError::Decode { .. })));
    }

    #[test]
    fn we_cannot_decode_invalid_value() {
        let (storage_key, _) = valid_storage_key_and_value();

        let invalid_value = vec![3];

        let result =
            decode_storage_key_and_value::<TestPrefixFoliate>(&storage_key, &invalid_value);

        assert!(matches!(result, Err(DecodeStorageError::Decode { .. })));
    }

    #[test]
    fn we_can_encode_prefix_leaves() {
        let key_value_pairs = [
            (vec![0], vec![0]),
            (vec![0], vec![1]),
            (vec![1], vec![0]),
            (vec![1], vec![1]),
        ]
        .map(|(k, v)| {
            let storage_key = TestStorageInstance::prefix_hash()
                .into_iter()
                .chain(k)
                .collect();

            (storage_key, v)
        });

        let actual = encode_prefix_leaves::<TestPrefixFoliate, _>(key_value_pairs).unwrap();
        let expected = vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]];

        assert_eq!(actual, expected);
    }

    #[test]
    fn we_cannot_encode_prefix_leaves_that_fail_to_decode() {
        let key_value_pairs = [
            (vec![0], vec![0]),
            (vec![0], vec![1]),
            (vec![1], vec![2]), // bad value
            (vec![1], vec![1]),
        ]
        .map(|(k, v)| {
            let storage_key = TestStorageInstance::prefix_hash()
                .into_iter()
                .chain(k)
                .collect();

            (storage_key, v)
        });

        let result = encode_prefix_leaves::<TestPrefixFoliate, _>(key_value_pairs);

        assert!(matches!(result, Err(DecodeStorageError::Decode { .. })));
    }

    #[test]
    fn we_can_calculate_storage_key_from_key_tuple() {
        let key = (true,);

        let actual = storage_key_for_prefix_key_tuple::<TestPrefixFoliate>(key);

        let (expected, _) = valid_storage_key_and_value();

        assert_eq!(actual, expected);
    }
}
