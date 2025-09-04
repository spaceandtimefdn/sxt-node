//! Strategies for producing sxt-core types for use in tests.
use alloc::vec;

use on_chain_table::proptest::ident;
use proptest::prelude::*;
use sqlparser::ast::ObjectName;

use crate::tables::TableIdentifier;

prop_compose! {
    /// Strategy for producing [`TableIdentifier`]s.
    pub fn table_identifier()(namespace in ident(), name in ident()) -> TableIdentifier {
        TableIdentifier::try_from(&ObjectName(vec![namespace, name]))
            .expect("ident strategies produce valid identifiers")
    }
}
