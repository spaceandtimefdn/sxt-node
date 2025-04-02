use std::marker::PhantomData;

use codec::Encode;
use frame_support::Blake2_128Concat;
use proof_of_sql_commitment_map::{CommitmentScheme, TableCommitmentBytes};
use sxt_core::tables::TableIdentifier;

use crate::{HashAndKeyTuple, PrefixFoliate};

/// [`PrefixFoliate`] for the `CommitmentStorageMap` storage in `pallet_commitments`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitmentMapPrefixFoliate<T: pallet_commitments::Config>(PhantomData<T>);

const ASCII_PERIOD: u8 = 46;

impl<T> PrefixFoliate for CommitmentMapPrefixFoliate<T>
where
    T: pallet_commitments::Config,
{
    type StorageInstance = pallet_commitments::_GeneratedPrefixForStorageCommitmentStorageMap<T>;
    type HashAndKeyTuple = (
        (Blake2_128Concat, TableIdentifier),
        (Blake2_128Concat, CommitmentScheme),
    );
    type Value = TableCommitmentBytes;

    // encode the table identifier like "{namespace}.{name}"
    fn leaf_encode_key(
        (TableIdentifier { namespace, name }, commitment_scheme): <Self::HashAndKeyTuple as HashAndKeyTuple>::KeyTuple,
    ) -> Vec<u8> {
        let table_identifier_utf8: Vec<u8> = namespace
            .into_iter()
            .chain(std::iter::once(ASCII_PERIOD))
            .chain(name)
            .collect();

        // the table identifier length should never exceed one 127
        let table_identifier_length_prefix = table_identifier_utf8.len() as u8;

        std::iter::once(table_identifier_length_prefix)
            .chain(table_identifier_utf8)
            .chain(commitment_scheme.encode())
            .collect()
    }

    // encode the raw bytes without length prefix
    fn leaf_encode_value(value: Self::Value) -> Vec<u8> {
        value.data.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use sxt_runtime::Runtime;

    use super::*;

    #[test]
    fn we_can_encode_leaf_table_identifier() {
        let table_schema = "SCHEMA";
        let table_name = "TABLE";

        let table_id_string = format!("{table_schema}.{table_name}");

        let table_id = TableIdentifier {
            namespace: table_schema.as_bytes().to_vec().try_into().unwrap(),
            name: table_name.as_bytes().to_vec().try_into().unwrap(),
        };

        let commitment_scheme = CommitmentScheme::DynamicDory;

        let actual =
            CommitmentMapPrefixFoliate::<Runtime>::leaf_encode_key((table_id, commitment_scheme));

        let expected = std::iter::once(table_id_string.len() as u8) // length prefix
            .chain(table_id_string.as_bytes().iter().copied()) // stringified table identifier
            .chain(std::iter::once(1)) // commitment scheme
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn we_can_leaf_encode_commitment() {
        let raw_bytes = (0u8..=255).collect::<Vec<_>>();
        let table_commitment_bytes = TableCommitmentBytes {
            data: raw_bytes.clone().try_into().unwrap(),
        };

        let actual =
            CommitmentMapPrefixFoliate::<Runtime>::leaf_encode_value(table_commitment_bytes);

        assert_eq!(actual, raw_bytes);
    }
}
