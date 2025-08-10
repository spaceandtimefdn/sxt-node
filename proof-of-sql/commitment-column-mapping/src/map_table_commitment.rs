use proof_of_sql::base::commitment::{ColumnCommitmentMetadata, NegativeRange, TableCommitment};
use proof_of_sql_commitment_map::generic_over_commitment::{ResultOkType, TableCommitmentType};
use proof_of_sql_commitment_map::{CommitmentId, GenericOverCommitmentFn};
use sqlparser::ast::Ident;

/// Returns the table commitment with the mapping applied to its metadata.
pub fn map_column_commitment_metadata<C: CommitmentId>(
    table_commitment: TableCommitment<C>,
    f: impl Fn((Ident, ColumnCommitmentMetadata)) -> (Ident, ColumnCommitmentMetadata),
) -> Result<TableCommitment<C>, NegativeRange> {
    let column_commitments = table_commitment
        .column_commitments()
        .clone()
        .into_iter()
        .map(|(ident, metadata, commitment)| {
            let (ident, metadata) = f((ident, metadata));

            (ident, metadata, commitment)
        })
        .collect();

    TableCommitment::try_new(column_commitments, table_commitment.range().clone())
}

/// `GenericOverCommitmentFn` that returns the table commitment with the mapping applied to its
/// metadata.
#[derive(Copy, Clone)]
pub struct MapColumnCommitmentMetadataFn<F>(pub F)
where
    F: Fn((Ident, ColumnCommitmentMetadata)) -> (Ident, ColumnCommitmentMetadata);

impl<F> GenericOverCommitmentFn for MapColumnCommitmentMetadataFn<F>
where
    F: Fn((Ident, ColumnCommitmentMetadata)) -> (Ident, ColumnCommitmentMetadata),
{
    type In = TableCommitmentType;
    type Out = ResultOkType<TableCommitmentType, NegativeRange>;

    fn call<C: CommitmentId>(
        &self,
        input: TableCommitment<C>,
    ) -> Result<TableCommitment<C>, NegativeRange> {
        map_column_commitment_metadata(input, &self.0)
    }
}

#[cfg(test)]
pub mod tests {
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;
    use core::ops::Range;

    use commitment_sql::OnChainTableToTableCommitmentFn;
    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql::base::commitment::ColumnCommitmentMetadataMap;
    use proof_of_sql_commitment_map::generic_over_commitment::{
        CommitmentType,
        ConcreteType,
        VecType,
    };
    use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;

    use super::*;

    /// `GenericOverCommitmentFn` that returns a table commitment's metadata values.
    pub struct GetCommitmentMetadataFn;

    impl GenericOverCommitmentFn for GetCommitmentMetadataFn {
        type In = TableCommitmentType;
        type Out = ConcreteType<(Range<usize>, ColumnCommitmentMetadataMap)>;

        fn call<C: CommitmentId>(
            &self,
            input: TableCommitment<C>,
        ) -> (Range<usize>, ColumnCommitmentMetadataMap) {
            (
                input.range().clone(),
                input.column_commitments().column_metadata().clone(),
            )
        }
    }

    /// `GenericOverCommitmentFn` that returns a table commitment's commitment values.
    pub struct GetCommitmentVecFn;

    impl GenericOverCommitmentFn for GetCommitmentVecFn {
        type In = TableCommitmentType;
        type Out = VecType<CommitmentType>;

        fn call<C: CommitmentId>(&self, input: TableCommitment<C>) -> Vec<C> {
            input.column_commitments().commitments().clone()
        }
    }

    #[test]
    fn we_can_map_column_commitment_metadata() {
        let setups = get_or_init_from_files_with_four_points_unchecked();

        let table = OnChainTable::try_from_iter([
            (Ident::new("INT_COL"), OnChainColumn::Int(vec![1, 2, 3])),
            (
                Ident::new("VARCHAR_COL"),
                OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec()),
            ),
        ])
        .unwrap();

        setups.into_iter().for_each(|any| {
            let any_commitment = any
                .map(OnChainTableToTableCommitmentFn::new(&table, 0))
                .transpose_result()
                .unwrap();

            let original_column_commitments = any_commitment.clone().map(GetCommitmentVecFn);

            let (original_range, original_column_metadata_map) =
                any_commitment.clone().map(GetCommitmentMetadataFn).unwrap();

            let mapped_commitment = any_commitment
                .map(MapColumnCommitmentMetadataFn(|(mut ident, metadata)| {
                    ident.value.push_str("_MAPPED");
                    (ident, metadata)
                }))
                .transpose_result()
                .unwrap();

            let mapped_column_commitments = mapped_commitment.clone().map(GetCommitmentVecFn);

            let (mapped_range, mapped_column_metadata_map) = mapped_commitment
                .clone()
                .map(GetCommitmentMetadataFn)
                .unwrap();

            assert_eq!(mapped_column_commitments, original_column_commitments);
            assert_eq!(mapped_range, original_range);
            assert_eq!(
                mapped_column_metadata_map
                    .clone()
                    .into_values()
                    .collect::<Vec<_>>(),
                original_column_metadata_map
                    .into_values()
                    .collect::<Vec<_>>()
            );

            assert_eq!(
                mapped_column_metadata_map.into_keys().collect::<Vec<_>>(),
                ["INT_COL_MAPPED", "VARCHAR_COL_MAPPED"]
                    .map(Ident::new)
                    .to_vec()
            );
        });
    }
}
