use alloc::vec::Vec;

use on_chain_table::{IndexSet, OnChainColumn, OnChainTable};
use snafu::Snafu;
use sqlparser::ast::Ident;

/// Errors that can occur in [`try_map_on_chain_table`].
#[derive(Debug, Snafu)]
pub enum TryMapOnChainTableError<E: core::error::Error> {
    /// Mapping function returned error.
    #[snafu(display("mapping function returned error: {error}"))]
    MapError {
        /// The source error.
        error: E,
    },
    /// After mapping, columns no longer have equal length
    #[snafu(display("after mapping, columns no longer have equal length"))]
    ColumnsNoLongerEqualLength,
}

/// Returns the table with the given mapping applied to its columns.
pub fn try_map_columns<E: core::error::Error>(
    table: OnChainTable,
    f: impl FnMut((Ident, OnChainColumn)) -> Result<(Ident, OnChainColumn), E>,
) -> Result<OnChainTable, TryMapOnChainTableError<E>> {
    let columns = table
        .into_iter()
        .map(f)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| TryMapOnChainTableError::MapError { error })?;

    OnChainTable::try_from_iter(columns)
        .map_err(|_| TryMapOnChainTableError::ColumnsNoLongerEqualLength)
}

/// Returns a set of identifiers whose type is `VarChar` in the table.
pub fn varchar_columns(table: &OnChainTable) -> IndexSet<Ident> {
    table
        .iter()
        .filter_map(|(identifier, column)| {
            matches!(column, OnChainColumn::VarChar(_)).then_some(identifier.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;
    use core::convert::Infallible;

    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(display("col already mapped"))]
    struct AlreadyMapped;

    fn append_mapped_to_ident(
        (mut ident, column): (Ident, OnChainColumn),
    ) -> Result<(Ident, OnChainColumn), AlreadyMapped> {
        if ident.value.ends_with("_MAPPED") {
            Err(AlreadyMapped)
        } else {
            ident.value.push_str("_MAPPED");
            Ok((ident, column))
        }
    }

    #[test]
    fn we_can_map_columns() {
        let table = OnChainTable::try_from_iter([
            (Ident::new("INT_COL"), OnChainColumn::Int(vec![1, 2, 3])),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ])
        .unwrap();

        let mapped_table = try_map_columns(table, append_mapped_to_ident).unwrap();

        assert_eq!(
            mapped_table
                .as_map()
                .clone()
                .into_keys()
                .collect::<Vec<_>>(),
            ["INT_COL_MAPPED", "VARCHAR_COL_MAPPED"]
                .map(Ident::new)
                .to_vec()
        );

        assert!(matches!(
            try_map_columns(mapped_table, append_mapped_to_ident),
            Err(TryMapOnChainTableError::MapError { .. })
        ));
    }

    #[test]
    fn we_cannot_map_columns_deleting_some_rows() {
        let table = OnChainTable::try_from_iter([
            (Ident::new("INT_COL"), OnChainColumn::Int(vec![1, 2, 3])),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ])
        .unwrap();

        let result = try_map_columns(table, |(ident, column)| {
            let column = match column {
                OnChainColumn::Int(_) => OnChainColumn::Int(vec![]),
                _ => column,
            };

            Ok::<_, Infallible>((ident, column))
        });

        assert!(matches!(
            result,
            Err(TryMapOnChainTableError::ColumnsNoLongerEqualLength)
        ));
    }

    #[test]
    fn we_can_get_all_varchar_columns_from_table() {
        let table = OnChainTable::try_from_iter([
            (Ident::new("INT_COL"), OnChainColumn::Int(vec![1, 2, 3])),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
            (
                Ident::new("OTHER_VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ])
        .unwrap();

        let varchar_columns = varchar_columns(&table);
        let expected_varchar_columns =
            IndexSet::from_iter(["VARCHAR_COL", "OTHER_VARCHAR_COL"].map(Ident::new));

        assert_eq!(varchar_columns, expected_varchar_columns);
    }
}
