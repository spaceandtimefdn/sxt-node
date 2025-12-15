use alloc::vec::Vec;

use itertools::Itertools;
use proof_of_sql::base::commitment::TableCommitment;
use proof_of_sql_commitment_map::generic_over_commitment::{
    ConcreteType, OptionType, PairType, TableCommitmentType,
};
use proof_of_sql_commitment_map::{
    AnyCommitmentScheme, CommitmentId, GenericOverCommitmentFn, PerCommitmentScheme,
};
use snafu::Snafu;

/// The insert cannot be performed as the resulting end row exceeds the limit.
#[derive(Debug, Snafu)]
#[snafu(display("the insert cannot be performed as the resulting end row exceeds the limit."))]
pub struct InsertExceedsLimit {
    /// The original end row value before insert.
    pub original_end: u32,
    /// The length of the insert.
    pub insert_len: u32,
}

/// Returns the end row value that would result from inserting `insert_len` rows to the table
/// commitment.
fn table_commitment_end_row_insert_simulation_generic<C: CommitmentId>(
    insert_len: u32,
    table_commitment: TableCommitment<C>,
    end_row_limit: u32,
) -> Result<u32, InsertExceedsLimit> {
    // end is a `usize`, which is 32-bit in the runtime wasm32 target.
    let original_end = table_commitment.range().end as u32;

    original_end
        .checked_add(insert_len)
        .and_then(|new_end| (new_end <= end_row_limit).then_some(new_end))
        .ok_or(InsertExceedsLimit {
            original_end,
            insert_len,
        })
}

/// Simple wrapper around `table_commitment_end_row_insert_simulation_generic` that takes an
/// optional `TableCommitment`, and returns an optional result accordingly.
fn maybe_table_commitment_end_row_insert_simulation_generic<C: CommitmentId>(
    insert_len: u32,
    maybe_table_commitment: Option<TableCommitment<C>>,
    end_row_limit: u32,
) -> Option<Result<u32, InsertExceedsLimit>> {
    maybe_table_commitment.map(|table_commitment| {
        table_commitment_end_row_insert_simulation_generic(
            insert_len,
            table_commitment,
            end_row_limit,
        )
    })
}

/// `GenericOverCommitmentFn` that simulates an insert and returns the resulting end row.
pub struct TableCommitmentEndRowInsertSimulation {
    insert_len: u32,
}

impl GenericOverCommitmentFn for TableCommitmentEndRowInsertSimulation {
    type In = PairType<OptionType<TableCommitmentType>, ConcreteType<u32>>;
    type Out = ConcreteType<Option<Result<u32, InsertExceedsLimit>>>;

    fn call<C: CommitmentId>(
        &self,
        input: (Option<TableCommitment<C>>, u32),
    ) -> Option<Result<u32, InsertExceedsLimit>> {
        maybe_table_commitment_end_row_insert_simulation_generic(self.insert_len, input.0, input.1)
    }
}

/// Errors that can occur when simulating the end row value after an insert for all commitments.
#[derive(Debug, Snafu)]
pub enum EndRowInsertSimulationAllSchemesError {
    /// No commitments to simulate insert.
    #[snafu(display("no commitments to simulate insert"))]
    NoCommitments,
    /// Table commitments have mismatched rows.
    #[snafu(display("table commitments have mismatched end rows"))]
    EndRowMismatch,
    /// The insert cannot be performed as it exceeds the limit.
    #[snafu(display("{source}"), context(false))]
    ExceedsLimit {
        /// The source limit error.
        source: InsertExceedsLimit,
    },
}

/// Simulates an insert and returns the resulting end row for all commitments.
pub fn table_commitment_end_row_insert_simulation_all_schemes(
    insert_len: u32,
    table_commitments_per_scheme: PerCommitmentScheme<OptionType<TableCommitmentType>>,
    end_row_limits_per_scheme: PerCommitmentScheme<ConcreteType<u32>>,
) -> Result<u32, EndRowInsertSimulationAllSchemesError> {
    let end_row_insert_simulation = TableCommitmentEndRowInsertSimulation { insert_len };

    let end_row_simulations_per_scheme = table_commitments_per_scheme
        .zip(end_row_limits_per_scheme)
        .map(end_row_insert_simulation);

    end_row_simulations_per_scheme
        .into_iter()
        .flat_map(AnyCommitmentScheme::unwrap)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .all_equal_value()
        .map_err(|maybe_diff| {
            maybe_diff
                .map(|_| EndRowInsertSimulationAllSchemesError::EndRowMismatch)
                .unwrap_or(EndRowInsertSimulationAllSchemesError::NoCommitments)
        })
}
