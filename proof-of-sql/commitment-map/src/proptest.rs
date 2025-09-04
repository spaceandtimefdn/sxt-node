//! Strategies for generating commitment scheme types for use in tests.

use proptest::prelude::*;

use crate::CommitmentSchemeFlags;

prop_compose! {
    /// Strategy for producing [`CommitmentSchemeFlags`] with at least one commitment scheme
    /// enabled.
    pub fn commitment_scheme_flags()(weight in 1u8..4) -> CommitmentSchemeFlags {
        CommitmentSchemeFlags {
            hyper_kzg: weight % 2 == 1,
            dynamic_dory: (weight >> 1) % 2 == 1,
        }
    }
}
