//! Strategies for generating commitments for use in tests.

use on_chain_table::OnChainTable;
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::{
    CommitmentSchemeFlags,
    PerCommitmentScheme,
    TableCommitmentPerCommitmentScheme,
};
use proptest::prelude::*;

use crate::OnChainTableToTableCommitmentFn;

pub fn table_commitment_per_commitment_scheme<'a, T, CS>(
    setups: PerCommitmentScheme<AssociatedPublicSetupType<'a>>,
    table: T,
    commitment_schemes: CS,
) -> impl Strategy<Value = TableCommitmentPerCommitmentScheme> + use<'a, T, CS>
where
    T: Strategy<Value = OnChainTable>,
    CS: Strategy<Value = CommitmentSchemeFlags>,
{
    (table, commitment_schemes).prop_map(move |(table, commitment_schemes)| {
        let table_to_table_commitment = OnChainTableToTableCommitmentFn::new(&table, 0);
        setups
            .select(&commitment_schemes)
            .into_flat_iter()
            .map(|setup| {
                setup
                    .map(&table_to_table_commitment)
                    .transpose_result()
                    .expect("table is empty, therefore has no out-of-bounds values")
            })
            .collect()
    })
}
