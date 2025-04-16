/// Input limit for proof-of-sql proof plans in bytes.
pub const PROOF_PLAN_SIZE_LIMIT: usize = 65_536;

/// Input limit for number of tables in queries.
pub const NUM_TABLES_LIMIT: usize = 64;

/// Input limit for sql query text.
pub const QUERY_SIZE_LIMIT: usize = 65_536;

#[cfg(test)]
mod tests {
    use sxt_runtime::pallet_commitments::runtime_api::MAX_TABLES_IN_TABLE_COMMITMENTS_QUERY;

    use super::*;

    #[test]
    fn num_tables_limit_does_not_exceed_table_commitments_api_limit() {
        assert!(NUM_TABLES_LIMIT as u32 <= MAX_TABLES_IN_TABLE_COMMITMENTS_QUERY);
    }
}
