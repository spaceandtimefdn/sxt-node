//! Strategies for producing sxt-core types for use in tests.
use alloc::vec;

use arrow::array::RecordBatch;
use on_chain_table::proptest::{
    ident,
    on_chain_table,
    on_chain_table_compatible_record_batch,
    proof_of_sql_schema,
};
use on_chain_table::OnChainTable;
use proptest::prelude::*;
use proptest_derive::Arbitrary;
use sqlparser::ast::ObjectName;

use crate::tables::{TableIdentifier, MAX_COLS_PER_TABLE};

prop_compose! {
    /// Strategy for producing [`TableIdentifier`]s.
    pub fn table_identifier()(namespace in ident(), name in ident()) -> TableIdentifier {
        TableIdentifier::try_from(&ObjectName(vec![namespace, name]))
            .expect("ident strategies produce valid identifiers")
    }
}

/// State transitions that can slightly transform byte data..
///
/// Good for randomly generating data that is close-to-valid by generating valid data and then
/// applying a random corruption.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Arbitrary)]
pub enum DataCorruption {
    /// A byte's value is changed.
    Set {
        /// The index of the byte.
        index: usize,
        /// The new value of the byte.
        value: u8,
    },
    /// A byte is inserted.
    Insert {
        /// The index of the new byte.
        index: usize,
        /// The value of the new byte.
        value: u8,
    },
    /// A byte is removed.
    Remove {
        /// The index of the removed byte.
        index: usize,
    },
}

impl DataCorruption {
    /// Applies this corruption to the given data.
    pub fn corrupt(&self, mut data: Vec<u8>) -> Vec<u8> {
        let len = data.len();
        match self {
            DataCorruption::Set { index, value } => {
                data[index % len] = *value;
            }
            DataCorruption::Insert { index, value } => data.insert(index % len, *value),
            DataCorruption::Remove { index } => {
                data.remove(index % len);
            }
        }
        data
    }
}

/// Common strategy for creating an on-chain-table with up to MAX_COLS_PER_TABLE columns and up to
/// 64 rows
pub fn canonical_on_chain_table() -> impl Strategy<Value = OnChainTable> {
    on_chain_table(
        proof_of_sql_schema(1..MAX_COLS_PER_TABLE as usize),
        0..64usize,
    )
}

/// Common strategy for creating a record batch with up to MAX_COLS_PER_TABLE columns and up to
/// 64 rows
pub fn canonical_record_batch() -> impl Strategy<Value = RecordBatch> {
    on_chain_table_compatible_record_batch(canonical_on_chain_table())
}
