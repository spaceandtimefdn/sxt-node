//! Older versions of the chain's native interface compute commitments to VarChar columns in ways
//! that aren't easy to verify in the EVM. On the other hand, a similar column, VarBinary, computes
//! commitments in a more EVM-compatible way.
//!
//! These utilities are meant to be used to temporarily convert varchar columns to varbinary before
//! computing commitments in the native interface. That way, using a more EVM-commpatible VarChar
//! commitment strategy can be deployed as a runtime upgrade, and native interface improvements can
//! come in more gracefully.

use alloc::string::FromUtf8Error;

use on_chain_table::{IndexSet, OnChainTable};
use proof_of_sql::base::commitment::NegativeRange;
use proof_of_sql_commitment_map::{PerCommitmentScheme, TableCommitmentPerCommitmentScheme};
use snafu::Snafu;
use sqlparser::ast::Ident;

use crate::combinator::{identifier_is_in_fn, map_if, try_map_if};
use crate::map_on_chain_table::{self, TryMapOnChainTableError};
use crate::map_table_commitment::MapColumnCommitmentMetadataFn;
use crate::{map_column_commitment_metadata, try_map_on_chain_column};

/// Returns the given commitments and table with varchar columns transformed to varbinary, as well
/// as a set of Idents that were changed.
pub fn convert_varchar_to_varbinary(
    commitments: TableCommitmentPerCommitmentScheme,
    table: OnChainTable,
) -> Result<
    (
        IndexSet<Ident>,
        TableCommitmentPerCommitmentScheme,
        OnChainTable,
    ),
    NegativeRange,
> {
    let varchar_columns = map_on_chain_table::varchar_columns(&table);

    let commitments = commitments
        .into_flat_iter()
        .map(|any| {
            any.map(MapColumnCommitmentMetadataFn(map_if(
                map_column_commitment_metadata::varchar_to_varbinary,
                identifier_is_in_fn(&varchar_columns),
            )))
            .transpose_result()
        })
        .collect::<Result<PerCommitmentScheme<_>, _>>()?;

    let table = map_on_chain_table::try_map_columns(
        table,
        try_map_if(
            try_map_on_chain_column::varchar_to_varbinary,
            identifier_is_in_fn(&varchar_columns),
        ),
    )
    .expect("the mapping functions used here will not change the number of rows");

    Ok((varchar_columns, commitments, table))
}

/// Errors that can occur in [`convert_selected_varbinary_columns_to_varchar`].
#[derive(Snafu, Debug)]
pub enum ConvertSelectedVarbinaryColumnsToVarcharError {
    /// Unable to reconstruct varbinary converted table commitment.
    ///
    /// Technically this is possible with current proof-of-sql due to TableCommitment
    /// deserialization not checking this type guarantee.
    #[snafu(
        display("unable to reconstruct varbinary converted table commitment: {source}"),
        context(false)
    )]
    UnexpectedNegativeRange {
        /// The source error.
        source: NegativeRange,
    },
    /// Unable to utf8 decode varbinary columns..
    #[snafu(display("unable to utf8 decode varbinary columns: {error}"))]
    FromUtf8 {
        /// The source error.
        error: FromUtf8Error,
    },
}

/// Returns the given commitments and table with varbinary columns transformed to varbinary, if
/// they are members of the selected columns.
pub fn convert_selected_varbinary_columns_to_varchar(
    selected_columns: &IndexSet<Ident>,
    commitments: TableCommitmentPerCommitmentScheme,
    table: OnChainTable,
) -> Result<
    (TableCommitmentPerCommitmentScheme, OnChainTable),
    ConvertSelectedVarbinaryColumnsToVarcharError,
> {
    let commitments = commitments
        .into_flat_iter()
        .map(|any| {
            any.map(MapColumnCommitmentMetadataFn(map_if(
                map_column_commitment_metadata::varbinary_to_varchar,
                identifier_is_in_fn(selected_columns),
            )))
            .transpose_result()
        })
        .collect::<Result<PerCommitmentScheme<_>, _>>()?;

    let table = map_on_chain_table::try_map_columns(
        table,
        try_map_if(
            try_map_on_chain_column::varbinary_to_varchar,
            identifier_is_in_fn(selected_columns),
        ),
    )
    .map_err(|e| match e {
        TryMapOnChainTableError::ColumnsNoLongerEqualLength => {
            panic!("the mapping functions used here will not change the number of rows")
        }
        TryMapOnChainTableError::MapError { error } => {
            ConvertSelectedVarbinaryColumnsToVarcharError::FromUtf8 { error }
        }
    })?;

    Ok((commitments, table))
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use commitment_sql::OnChainTableToTableCommitmentFn;
    use on_chain_table::OnChainColumn;
    use proof_of_sql::base::database::ColumnType;
    use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;

    use super::*;
    use crate::map_table_commitment::tests::GetCommitmentMetadataFn;

    #[test]
    fn we_can_convert_varchar_to_varbinary_and_back() {
        let setups = get_or_init_from_files_with_four_points_unchecked();

        let table = OnChainTable::try_from_iter([
            (Ident::new("INT_COL"), OnChainColumn::Int(vec![1, 2, 3])),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
            (
                Ident::new("VARBINARY_COL"),
                OnChainColumn::VarBinary(vec![
                    b"lorem".to_vec(),
                    b"ipsum".to_vec(),
                    vec![0, 1, 128],
                ]),
            ),
        ])
        .unwrap();

        let commitments = setups
            .into_iter()
            .map(|any| {
                any.map(OnChainTableToTableCommitmentFn::new(&table, 0))
                    .transpose_result()
                    .unwrap()
            })
            .collect::<PerCommitmentScheme<_>>();

        let (varchar_columns, converted_commitments, converted_table) =
            convert_varchar_to_varbinary(commitments.clone(), table.clone()).unwrap();

        converted_commitments
            .clone()
            .into_flat_iter()
            .for_each(|any| {
                let (_, column_commitment_metadata_map) = any.map(GetCommitmentMetadataFn).unwrap();

                assert_eq!(
                    column_commitment_metadata_map
                        .get(&Ident::new("INT_COL"))
                        .unwrap()
                        .column_type(),
                    &ColumnType::Int
                );
                assert_eq!(
                    column_commitment_metadata_map
                        .get(&Ident::new("VARCHAR_COL"))
                        .unwrap()
                        .column_type(),
                    &ColumnType::VarBinary
                );
                assert_eq!(
                    column_commitment_metadata_map
                        .get(&Ident::new("VARBINARY_COL"))
                        .unwrap()
                        .column_type(),
                    &ColumnType::VarBinary
                );
            });

        assert!(matches!(
            converted_table
                .as_map()
                .get(&Ident::new("INT_COL"))
                .unwrap(),
            OnChainColumn::Int(..)
        ));
        assert!(matches!(
            converted_table
                .as_map()
                .get(&Ident::new("VARCHAR_COL"))
                .unwrap(),
            OnChainColumn::VarBinary(..)
        ));
        assert!(matches!(
            converted_table
                .as_map()
                .get(&Ident::new("VARBINARY_COL"))
                .unwrap(),
            OnChainColumn::VarBinary(..)
        ));

        let (roundtrip_commitments, roundtrip_table) =
            convert_selected_varbinary_columns_to_varchar(
                &varchar_columns,
                converted_commitments,
                converted_table,
            )
            .unwrap();

        assert_eq!(roundtrip_commitments, commitments);
        assert_eq!(roundtrip_table, table);
    }
}
