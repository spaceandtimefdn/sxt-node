use codec::{Decode, Encode, FullCodec};
use frame_support::{ReversibleStorageHasher, StorageHasher};
use impl_trait_for_tuples::impl_for_tuples;

/// Trait for a pair of individual hash and key types that are associated in storage map definitions.
pub trait HashAndKey {
    /// The hash type for the storage key.
    type Hash: ReversibleStorageHasher;
    /// The key type for the storage key.
    type Key: FullCodec;
}

impl<H: ReversibleStorageHasher, K: FullCodec> HashAndKey for (H, K) {
    type Hash = H;
    type Key = K;
}

/// Trait for a full sequence of hash and key pairs that are associated in storage map definitions.
pub trait HashAndKeyTuple {
    /// The full key type for the storage map definition.
    type KeyTuple: FullCodec;

    /// Decode the full key given the hash-key sequence portion of a storage key.
    ///
    /// Returns the strongly-typed full key as a tuple and the remaining bytes.
    fn decode_key_tuple_from_storage_key_suffix(
        bytes: &[u8],
    ) -> Result<(Self::KeyTuple, &[u8]), codec::Error>;

    /// Returns the hash-key sequence portion of a storage key given a strongly-typed key.
    fn storage_key_suffix_from_key_tuple(key_tuple: Self::KeyTuple) -> Vec<u8>;
}

// Implements HashAndKeyTuple for tuples of 0 to 4 elements.
// So, these abstractions can handle storage maps of various lengths, and even plain storage values.
#[impl_for_tuples(0, 4)]
#[tuple_types_custom_trait_bound(HashAndKey)]
impl HashAndKeyTuple for Tuple {
    for_tuples!( type KeyTuple = ( #( Tuple::Key ),* ); );

    fn decode_key_tuple_from_storage_key_suffix(
        mut bytes: &[u8],
    ) -> Result<(Self::KeyTuple, &[u8]), codec::Error> {
        let key_tuple = for_tuples!(
        (
            #( {
                bytes = Tuple::Hash::reverse(&bytes);
                Tuple::Key::decode(&mut bytes)?
            } ),*
        )
        );

        Ok((key_tuple, bytes))
    }

    #[allow(clippy::let_and_return)]
    fn storage_key_suffix_from_key_tuple(key_tuple: Self::KeyTuple) -> Vec<u8> {
        let mut buffer = Vec::new();

        for_tuples!(
            #(
                let key_bytes = key_tuple.Tuple.encode();
                let hash_bytes = Tuple::Hash::hash(&key_bytes).as_ref().to_vec();
                buffer.extend(hash_bytes);
            )*
        );

        buffer
    }
}

#[cfg(test)]
mod tests {
    use frame_support::{Blake2_128Concat, Identity, Twox64Concat};
    use sp_core::{blake2_128, twox_64};

    use super::*;

    #[test]
    fn we_can_encode_and_decode_storage_key_suffixes() {
        let key_values = [1u8, 2, 3];
        let hash_1: Vec<u8> = vec![];
        let hash_2 = blake2_128(&key_values[1..2]).to_vec();
        let hash_3 = twox_64(&key_values[2..3]).to_vec();
        let full_storage_suffix = [hash_1, hash_2, hash_3]
            .iter()
            .cloned()
            .zip(key_values)
            .flat_map(|(hash, value)| hash.into_iter().chain(std::iter::once(value)))
            .collect::<Vec<_>>();

        // no keys
        let key_tuple_0 = ();
        let storage_suffix_size = 0;

        let encoded_key_suffix_0 = <()>::storage_key_suffix_from_key_tuple(key_tuple_0);
        assert_eq!(
            encoded_key_suffix_0,
            full_storage_suffix[0..storage_suffix_size].to_vec()
        );

        let (decoded_key_tuple_0, remaining_bytes_0) =
            <()>::decode_key_tuple_from_storage_key_suffix(&full_storage_suffix).unwrap();
        assert_eq!(
            remaining_bytes_0,
            &full_storage_suffix[storage_suffix_size..]
        );

        // 1 key
        let key_tuple_1 = (key_values[0],);
        let storage_suffix_size = storage_suffix_size + 1;

        let encoded_key_suffix_1 =
            <((Identity, u8),)>::storage_key_suffix_from_key_tuple(key_tuple_1);
        assert_eq!(
            encoded_key_suffix_1,
            full_storage_suffix[0..storage_suffix_size].to_vec()
        );

        let (decoded_key_tuple_1, remaining_bytes_1) =
            <((Identity, u8),)>::decode_key_tuple_from_storage_key_suffix(&full_storage_suffix)
                .unwrap();
        assert_eq!(key_tuple_1, decoded_key_tuple_1);
        assert_eq!(
            remaining_bytes_1,
            &full_storage_suffix[storage_suffix_size..]
        );

        // 2 keys
        let key_tuple_2 = (key_values[0], key_values[1]);
        let storage_suffix_size = storage_suffix_size + 16 + 1;

        let encoded_key_suffix_2 =
            <((Identity, u8), (Blake2_128Concat, u8))>::storage_key_suffix_from_key_tuple(
                key_tuple_2,
            );
        assert_eq!(
            encoded_key_suffix_2,
            full_storage_suffix[0..storage_suffix_size].to_vec()
        );

        let (decoded_key_tuple_2, remaining_bytes_2) =
            <((Identity, u8), (Blake2_128Concat, u8))>::decode_key_tuple_from_storage_key_suffix(
                &full_storage_suffix,
            )
            .unwrap();
        assert_eq!(key_tuple_2, decoded_key_tuple_2);
        assert_eq!(
            remaining_bytes_2,
            &full_storage_suffix[storage_suffix_size..]
        );

        // 3 keys
        let key_tuple_3 = (key_values[0], key_values[1], key_values[2]);
        let storage_suffix_size = storage_suffix_size + 8 + 1;

        let encoded_key_suffix_3 =
            <((Identity, u8), (Blake2_128Concat, u8), (Twox64Concat, u8))>::storage_key_suffix_from_key_tuple(
                key_tuple_3,
            );
        assert_eq!(
            encoded_key_suffix_3,
            full_storage_suffix[0..storage_suffix_size].to_vec()
        );

        let (decoded_key_tuple_3, remaining_bytes_3) = <(
            (Identity, u8),
            (Blake2_128Concat, u8),
            (Twox64Concat, u8),
        )>::decode_key_tuple_from_storage_key_suffix(
            &full_storage_suffix
        )
        .unwrap();
        assert_eq!(key_tuple_3, decoded_key_tuple_3);
        assert_eq!(
            remaining_bytes_3,
            &full_storage_suffix[storage_suffix_size..]
        );
    }

    #[test]
    fn we_cannot_decode_invalid_suffix() {
        let valid_bool_bytes = [0u8, 1u8];
        assert!(
            <((Identity, bool), (Identity, bool))>::decode_key_tuple_from_storage_key_suffix(
                &valid_bool_bytes
            )
            .is_ok()
        );

        let first_bool_invalid = [2u8, 1u8];
        assert!(
            <((Identity, bool), (Identity, bool))>::decode_key_tuple_from_storage_key_suffix(
                &first_bool_invalid
            )
            .is_err()
        );

        let second_bool_invalid = [0u8, 2u8];
        assert!(
            <((Identity, bool), (Identity, bool))>::decode_key_tuple_from_storage_key_suffix(
                &second_bool_invalid
            )
            .is_err()
        );

        let both_bools_invalid = [2u8, 3u8];
        assert!(
            <((Identity, bool), (Identity, bool))>::decode_key_tuple_from_storage_key_suffix(
                &both_bools_invalid
            )
            .is_err()
        );
    }
}
