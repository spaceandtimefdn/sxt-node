//! Module containing utility functions `TableCommitment`s.

use proof_of_sql::base::commitment::{Commitment, NegativeRange, TableCommitment};

/// Map a `TableCommitment<A>` to a `TableCommitment<B>` by applying `f` to each column commitment.
pub fn map_table_commitment<'a, A: Commitment, B: Commitment, F: Fn(&'a A) -> B>(
    table_commitment: &'a TableCommitment<A>,
    f: F,
) -> Result<TableCommitment<B>, NegativeRange> {
    TableCommitment::try_new(
        table_commitment
            .column_commitments()
            .iter()
            .map(|(i, m, c)| (i.clone(), *m, f(c)))
            .collect(),
        table_commitment.range().clone(),
    )
}
