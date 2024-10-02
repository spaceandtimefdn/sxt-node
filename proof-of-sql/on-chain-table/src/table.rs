use crate::{column::OnChainColumn, map::IndexMap, OutOfScalarBounds};
use indexmap::map::{IntoIter, Iter};
use proof_of_sql::base::{commitment::CommittableColumn, scalar::Scalar};
use proof_of_sql_parser::Identifier;
use serde::{Deserialize, Serialize};
use snafu::Snafu;

/// Table data type for all data types supported by sxt-node.
///
/// With the `arrow` feature, implements conversion to/from arrow `RecordBatch`s.
///
/// Without the `std` feature, this type can be used in `no_std` envs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnChainTable(IndexMap<Identifier, OnChainColumn>);

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
    pub fn try_from_iter(
        iter: impl IntoIterator<Item = (Identifier, OnChainColumn)>,
    ) -> Result<OnChainTable, OnChainTableError> {
        let mut peekable_iter = iter.into_iter().peekable();

        let length = peekable_iter
            .peek()
            .map(|(_, column)| column.len())
            .ok_or(OnChainTableError::NoColumns)?;

        peekable_iter
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
    pub fn as_map(&self) -> &IndexMap<Identifier, OnChainColumn> {
        &self.0
    }

    /// Returns a borrowing iterator over all identifier-column pairs.
    pub fn iter(&self) -> Iter<Identifier, OnChainColumn> {
        self.into_iter()
    }

    /// Returns an iterator over this table with committable columns in the scalar field `S`.
    ///
    /// After the error is handled, this can be supplied to the `proof-of-sql` commitment API.
    pub fn iter_committable<S: Scalar>(
        &self,
    ) -> impl Iterator<Item = Result<(&Identifier, CommittableColumn), OutOfScalarBounds>> {
        self.iter().map(|(id, column)| {
            column
                .try_to_committable_column::<S>()
                .map(|column| (id, column))
        })
    }
}

impl IntoIterator for OnChainTable {
    type Item = (Identifier, OnChainColumn);
    type IntoIter = IntoIter<Identifier, OnChainColumn>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a OnChainTable {
    type Item = (&'a Identifier, &'a OnChainColumn);
    type IntoIter = Iter<'a, Identifier, OnChainColumn>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec, vec::Vec};
    use proof_of_sql::{
        base::{
            database::{OwnedColumn, OwnedTable},
            scalar::Curve25519Scalar,
        },
        proof_primitive::dory::DoryScalar,
    };

    #[test]
    fn we_can_convert_table_to_and_from_iter() {
        let data = [
            (
                "bigint_col".parse().unwrap(),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                "varchar_col".parse().unwrap(),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ];

        let table = OnChainTable::try_from_iter(data.clone()).unwrap();
        let expected_map = IndexMap::<Identifier, OnChainColumn>::from_iter(data.clone());
        assert_eq!(table.as_map(), &expected_map);

        assert_eq!(
            table.iter().collect::<Vec<_>>(),
            expected_map.iter().collect::<Vec<_>>()
        );

        assert_eq!(table.into_iter().collect::<Vec<_>>(), data.to_vec());
    }

    #[test]
    fn we_can_get_table_size() {
        let data = [("bigint_col".parse().unwrap(), OnChainColumn::BigInt(vec![]))];
        let table = OnChainTable::try_from_iter(data.clone()).unwrap();

        assert_eq!(table.num_columns(), 1);
        assert_eq!(table.num_rows(), 0);

        let data = [
            (
                "bigint_col".parse().unwrap(),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                "varchar_col".parse().unwrap(),
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
                "bigint_col".parse().unwrap(),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                "varchar_col".parse().unwrap(),
                OnChainColumn::VarChar(["lorem", "ipsum"].map(String::from).to_vec()),
            ),
        ];
        assert!(matches!(
            OnChainTable::try_from_iter(data),
            Err(OnChainTableError::ColumnLengthMismatch)
        ));

        let data = [
            (
                "bigint_col".parse().unwrap(),
                OnChainColumn::BigInt(vec![1, 2]),
            ),
            (
                "varchar_col".parse().unwrap(),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ];
        assert!(matches!(
            OnChainTable::try_from_iter(data),
            Err(OnChainTableError::ColumnLengthMismatch)
        ));

        let data = [
            (
                "bigint_col".parse().unwrap(),
                OnChainColumn::BigInt(vec![1, 2, 3]),
            ),
            (
                "varchar_col".parse().unwrap(),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
            (
                "boolean_col".parse().unwrap(),
                OnChainColumn::Boolean(vec![true, false]),
            ),
        ];
        assert!(matches!(
            OnChainTable::try_from_iter(data),
            Err(OnChainTableError::ColumnLengthMismatch)
        ));
    }

    fn we_can_iter_table_with_committable_columns<S: Scalar>() {
        let bigint_id = "bigint_col".parse().unwrap();
        let bigint_data = vec![-10, 0, 3];

        let varchar_id = "varchar_col".parse().unwrap();
        let varchar_data = ["lorem", "ipsum", "dolor"].map(String::from).to_vec();

        let on_chain_data = [
            (bigint_id, OnChainColumn::BigInt(bigint_data.clone())),
            (varchar_id, OnChainColumn::VarChar(varchar_data.clone())),
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
    fn we_can_iter_table_with_dory_committable_columns() {
        we_can_iter_table_with_committable_columns::<DoryScalar>()
    }

    #[test]
    fn we_can_iter_table_with_ipa_committable_columns() {
        we_can_iter_table_with_committable_columns::<Curve25519Scalar>()
    }
}
