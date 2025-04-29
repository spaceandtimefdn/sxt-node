use alloc::vec::Vec;

use indexmap::map::{IntoIter, Iter};
use primitive_types::U256;
use proof_of_sql::base::commitment::CommittableColumn;
use proof_of_sql::base::scalar::Scalar;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sqlparser::ast::Ident;

use crate::column::OnChainColumn;
use crate::map::IndexMap;
use crate::OutOfScalarBounds;

/// Table data type for all data types supported by sxt-node.
///
/// Guarantees that all column identifiers are uppercase.
///
/// With the `arrow` feature, implements conversion to/from arrow `RecordBatch`s.
///
/// Without the `std` feature, this type can be used in `no_std` envs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OnChainTable(IndexMap<Ident, OnChainColumn>);

// This custom impl leverages [`OnChainTable::try_from_iter`] to preserve type guarantees.
impl<'de> Deserialize<'de> for OnChainTable {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map = IndexMap::<Ident, OnChainColumn>::deserialize(deserializer)?;

        OnChainTable::try_from_iter(map).map_err(serde::de::Error::custom)
    }
}

/// Errors that can occur when constructing a [`OnChainTable`].
#[derive(Debug, Snafu)]
pub enum OnChainTableError {
    /// [`OnChainTable`] must at least have one column.
    #[snafu(display("OnChainTable must at least have one column"))]
    NoColumns,
    /// [`OnChainTable`] cannot have columns of differing lengths.
    #[snafu(display("OnChainTable cannot have columns of different lengths"))]
    ColumnLengthMismatch,
}

impl OnChainTable {
    /// Create a new [`OnChainTable`] from an iterator.
    ///
    /// Coerces all column identifiers to uppercase.
    pub fn try_from_iter(
        iter: impl IntoIterator<Item = (Ident, OnChainColumn)>,
    ) -> Result<OnChainTable, OnChainTableError> {
        let mut peekable_iter = iter.into_iter().peekable();

        let length = peekable_iter
            .peek()
            .map(|(_, column)| column.len())
            .ok_or(OnChainTableError::NoColumns)?;

        peekable_iter
            .map(|(identifier, column)| {
                let identifier = Ident {
                    value: identifier.value.to_uppercase(),
                    ..identifier
                };
                (identifier, column)
            })
            .map(|(identifier, column)| {
                if column.len() != length {
                    Err(OnChainTableError::ColumnLengthMismatch)
                } else {
                    Ok((identifier, column))
                }
            })
            .collect::<Result<_, _>>()
            .map(OnChainTable)
    }

    /// Returns the number of columns in this table.
    pub fn num_columns(&self) -> usize {
        self.0.len()
    }

    /// Returns the number of rows in this table.
    pub fn num_rows(&self) -> usize {
        // internal map is guaranteed to..
        // 1. have at least one column
        // 2. have the same # of rows in every column
        self.0[0].len()
    }

    /// Returns the internal column map for this table.
    pub fn as_map(&self) -> &IndexMap<Ident, OnChainColumn> {
        &self.0
    }

    /// Returns a borrowing iterator over all identifier-column pairs.
    pub fn iter(&self) -> Iter<Ident, OnChainColumn> {
        self.into_iter()
    }

    /// Returns an iterator over this table with committable columns in the scalar field `S`.
    ///
    /// After the error is handled, this can be supplied to the `proof-of-sql` commitment API.
    pub fn iter_committable<S: Scalar>(
        &self,
    ) -> impl Iterator<Item = Result<(&Ident, CommittableColumn), OutOfScalarBounds>> {
        self.iter().map(|(id, column)| {
            column
                .try_to_committable_column::<S>()
                .map(|column| (id, column))
        })
    }

    /// Returns this [`OnChainTable`], with columns in the order provided, case-sensitive.
    ///
    /// There are a couple edge cases handled infallibly:
    /// - Any columns in the table that don't appear in the order will be placed at the end of the
    ///   table in their existing order.
    /// - Any identifier in the order that doesn't appear in the table is ignored.
    pub fn with_column_order<'a>(
        mut self,
        order: impl IntoIterator<Item = &'a Ident>,
    ) -> OnChainTable {
        let ordered_columns: IndexMap<_, _> = order
            .into_iter()
            .filter_map(|identifier| self.0.shift_remove_entry(identifier))
            // This intermediate collect explicitly performs all the shifting to the original map
            // before chaining the remainder of the columns, avoiding double mutable reference.
            .collect::<Vec<_>>()
            .into_iter()
            .chain(self.0)
            .collect();

        OnChainTable(ordered_columns)
    }

    /// Attempts to retrieve the values for a given decimal column name
    /// Returns None if the provided column does not exist
    pub fn get_decimal_by_column(&self, column_name: &str) -> Option<&Vec<U256>> {
        let column_id: Ident = Ident::new(column_name.to_uppercase());
        let column = self.as_map().get(&column_id)?;
        match column {
            OnChainColumn::Decimal75(_, _, values) => Some(values),
            _ => None,
        }
    }

    /// Attempts to retrieve the values for a given Bytes column name
    /// Returns None if the provided column does not exist
    pub fn get_bytes_by_column(&self, column_name: &str) -> Option<&Vec<Vec<u8>>> {
        let column_id: Ident = Ident::new(column_name.to_uppercase());
        let column = self.as_map().get(&column_id)?;
        match column {
            OnChainColumn::VarBinary(values) => Some(values),
            _ => None,
        }
    }

    /// Attempts to retrieve the values for a given VarChar column name
    /// Returns None if the provided column does not exist
    pub fn get_varchars_by_column(&self, column_name: &str) -> Option<&Vec<alloc::string::String>> {
        let column_id: Ident = Ident::new(column_name.to_uppercase());
        let column = self.as_map().get(&column_id)?;
        match column {
            OnChainColumn::VarChar(values) => Some(values),
            _ => None,
        }
    }

    /// Get the maximum block number contained in this on chain table
    pub fn max_block_number(&self) -> Option<i64> {
        // All SxT DDLs use BLOCK_NUMBER
        // TODO update this for user defined tables
        let column_id = Ident::new("BLOCK_NUMBER");
        let column = self.as_map().get(&column_id)?;

        match column {
            // All SxT DDLs use big int for block numbers
            OnChainColumn::BigInt(values) => values.iter().max().cloned(),
            _ => None,
        }
    }
}

impl IntoIterator for OnChainTable {
    type Item = (Ident, OnChainColumn);
    type IntoIter = IntoIter<Ident, OnChainColumn>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a OnChainTable {
    type Item = (&'a Ident, &'a OnChainColumn);
    type IntoIter = Iter<'a, Ident, OnChainColumn>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    use proof_of_sql::base::database::{OwnedColumn, OwnedTable};
    use proof_of_sql::base::math::decimal::Precision;
    use proof_of_sql::proof_primitive::dory::DoryScalar;
    use proof_of_sql::proof_primitive::hyperkzg::BNScalar;

    use super::*;

    #[test]
    fn we_can_convert_table_to_and_from_iter() {
        let data = [
            (
                Ident::new("BIGINT_COL"),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ];

        let table = OnChainTable::try_from_iter(data.clone()).unwrap();
        let expected_map = IndexMap::<Ident, OnChainColumn>::from_iter(data.clone());
        assert_eq!(table.as_map(), &expected_map);

        assert_eq!(
            table.iter().collect::<Vec<_>>(),
            expected_map.iter().collect::<Vec<_>>()
        );

        assert_eq!(table.into_iter().collect::<Vec<_>>(), data.to_vec());
    }

    #[test]
    fn we_can_get_table_size() {
        let data = [(Ident::new("BIGINT_COL"), OnChainColumn::BigInt(vec![]))];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        assert_eq!(table.num_columns(), 1);
        assert_eq!(table.num_rows(), 0);

        let data = [
            (
                Ident::new("BIGINT_COL"),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        assert_eq!(table.num_columns(), 2);
        assert_eq!(table.num_rows(), 3);
    }

    #[test]
    fn we_cannot_construct_table_with_no_columns() {
        assert!(matches!(
            OnChainTable::try_from_iter([]),
            Err(OnChainTableError::NoColumns)
        ))
    }

    #[test]
    fn we_cannot_construct_table_with_columns_of_differing_lengths() {
        let data = [
            (
                Ident::new("bigint_col"),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                Ident::new("varchar_col"),
                OnChainColumn::VarChar(["lorem", "ipsum"].map(String::from).to_vec()),
            ),
        ];
        assert!(matches!(
            OnChainTable::try_from_iter(data),
            Err(OnChainTableError::ColumnLengthMismatch)
        ));

        let data = [
            (Ident::new("bigint_col"), OnChainColumn::BigInt(vec![1, 2])),
            (
                Ident::new("varchar_col"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ];
        assert!(matches!(
            OnChainTable::try_from_iter(data),
            Err(OnChainTableError::ColumnLengthMismatch)
        ));

        let data = [
            (
                Ident::new("bigint_col"),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                Ident::new("varchar_col"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
            (
                Ident::new("boolean_col"),
                OnChainColumn::Boolean(vec![true, false]),
            ),
        ];
        assert!(matches!(
            OnChainTable::try_from_iter(data),
            Err(OnChainTableError::ColumnLengthMismatch)
        ));
    }

    fn we_can_iter_table_with_committable_columns<S: Scalar>() {
        let bigint_id = Ident::new("BIGINT_COL");
        let bigint_data = vec![-10, 0, 3];

        let varchar_id = Ident::new("VARCHAR_COL");
        let varchar_data = ["lorem", "ipsum", "dolor"].map(String::from).to_vec();

        let on_chain_data = [
            (
                bigint_id.clone(),
                OnChainColumn::BigInt(bigint_data.clone()),
            ),
            (
                varchar_id.clone(),
                OnChainColumn::VarChar(varchar_data.clone()),
            ),
        ];
        let on_chain_table = OnChainTable::try_from_iter(on_chain_data.clone()).unwrap();

        let owned_table_data = [
            (bigint_id, OwnedColumn::<S>::BigInt(bigint_data)),
            (varchar_id, OwnedColumn::<S>::VarChar(varchar_data)),
        ];
        let owned_table = OwnedTable::<S>::try_from_iter(owned_table_data).unwrap();

        let committable_columns = on_chain_table
            .iter_committable::<S>()
            .map(Result::unwrap)
            .collect::<Vec<_>>();

        let expected = owned_table
            .inner_table()
            .iter()
            .map(|(id, column)| (id, CommittableColumn::from(column)))
            .collect::<Vec<_>>();

        assert_eq!(committable_columns, expected);
    }

    #[test]
    fn we_can_order_columns() {
        let bigint_id = Ident::new("BIGINT_COL");
        let bigint_entry = (bigint_id.clone(), OnChainColumn::BigInt(vec![-10, 0, 3]));

        let varchar_id = Ident::new("VARCHAR_COL");
        let varchar_entry = (
            varchar_id.clone(),
            OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
        );

        let int_id = Ident::new("INT_COL");
        let int_entry = (int_id.clone(), OnChainColumn::Int(vec![0, 1, 1000]));

        let table = OnChainTable::try_from_iter([
            bigint_entry.clone(),
            varchar_entry.clone(),
            int_entry.clone(),
        ])
        .unwrap();

        let reversed_table = table.with_column_order([&int_id, &varchar_id, &bigint_id]);
        let expected_reversed_table = OnChainTable::try_from_iter([
            int_entry.clone(),
            varchar_entry.clone(),
            bigint_entry.clone(),
        ])
        .unwrap();
        assert_eq!(reversed_table, expected_reversed_table);

        let bumped_column_table = reversed_table.with_column_order([&varchar_id]);
        let expected_bumped_column_table = OnChainTable::try_from_iter([
            varchar_entry.clone(),
            int_entry.clone(),
            bigint_entry.clone(),
        ])
        .unwrap();
        assert_eq!(bumped_column_table, expected_bumped_column_table);

        let ignored_column_table =
            bumped_column_table.with_column_order(&[Ident::new("does_not_exist")]);
        let expected_ignored_column_table = OnChainTable::try_from_iter([
            varchar_entry.clone(),
            int_entry.clone(),
            bigint_entry.clone(),
        ])
        .unwrap();
        assert_eq!(ignored_column_table, expected_ignored_column_table);

        let all_cases_table = ignored_column_table.with_column_order([
            &bigint_id,
            &Ident::new("does_not_exist"),
            &int_id,
        ]);
        let expected_all_cases_table = OnChainTable::try_from_iter([
            bigint_entry.clone(),
            int_entry.clone(),
            varchar_entry.clone(),
        ])
        .unwrap();
        assert_eq!(all_cases_table, expected_all_cases_table);
    }

    #[test]
    fn we_can_iter_table_with_dory_committable_columns() {
        we_can_iter_table_with_committable_columns::<DoryScalar>()
    }

    #[test]
    fn we_can_iter_table_with_hyper_kzg_committable_columns() {
        we_can_iter_table_with_committable_columns::<BNScalar>()
    }
    #[test]
    fn get_decimal_with_valid_params_works() {
        let data = [(
            Ident::new("price"),
            OnChainColumn::Decimal75(
                Precision::new(18).unwrap(),
                2,
                vec![U256::from(100), U256::from(200)],
            ),
        )];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        let result = table.get_decimal_by_column("price");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &vec![U256::from(100), U256::from(200)]);
    }

    #[test]
    fn get_decimal_with_missing_column_is_none() {
        let data = [(
            Ident::new("name"),
            OnChainColumn::VarChar(vec!["Alice".to_string(), "Bob".to_string()]),
        )];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();
        let result = table.get_decimal_by_column("missing_column");
        assert!(result.is_none());
    }

    #[test]
    fn get_decimal_with_wrong_type_is_none() {
        let data = [(
            Ident::new("name"),
            OnChainColumn::VarChar(vec!["Alice".to_string(), "Bob".to_string()]),
        )];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        let result = table.get_decimal_by_column("name");
        assert!(result.is_none());
    }

    #[test]
    fn get_varchar_with_valid_params_works() {
        let data = [(
            Ident::new("name"),
            OnChainColumn::VarChar(vec!["Alice".to_string(), "Bob".to_string()]),
        )];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        let result = table.get_varchars_by_column("name");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            &vec!["Alice".to_string(), "Bob".to_string()]
        );
    }

    #[test]
    fn get_varchar_with_missing_column_is_none() {
        let data = [(
            Ident::new("name"),
            OnChainColumn::VarChar(vec!["Alice".to_string(), "Bob".to_string()]),
        )];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();
        let result = table.get_varchars_by_column("missing_column");
        assert!(result.is_none());
    }

    #[test]
    fn get_varchar_with_wrong_type_is_none() {
        let data = [(
            Ident::new("price"),
            OnChainColumn::Decimal75(
                Precision::new(18).unwrap(),
                2,
                vec![U256::from(100), U256::from(200)],
            ),
        )];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        let result = table.get_varchars_by_column("price");
        assert!(result.is_none());
    }

    #[test]
    fn we_can_construct_table_with_lowercase_column_identifiers_and_get_uppercase() {
        let data = [
            (
                Ident::new("bigint_col"),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                Ident::new("VaRcHaR_cOl"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ];

        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        let expected_data = [
            (
                Ident::new("BIGINT_COL"),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ];
        let expected_map = IndexMap::<Ident, OnChainColumn>::from_iter(expected_data);
        assert_eq!(table.as_map(), &expected_map);
    }
}
