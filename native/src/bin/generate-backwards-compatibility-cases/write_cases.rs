use std::path::Path;

use codec::Encode;
use polkadot_sdk::sp_core::keccak_256;
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRng, TestRunner};

/// Generates and writes cases to files.
///
/// Takes...
/// - a proptest strategy to generate input
/// - an `assert` function for callers to make assertions against input/output
/// - the directory to write cases to
pub fn write_cases<S, I, O, F, A, P>(strategy: S, f: F, assert: A, case_directory: P)
where
    S: Strategy<Value = I>,
    F: Fn(I) -> O,
    A: Fn(&I, &O) -> bool,
    I: Encode + Clone + std::fmt::Debug,
    O: Encode,
    P: AsRef<Path> + Clone,
{
    let config = proptest::test_runner::Config {
        cases: 64,
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..Default::default()
    };

    // using a deterministic RNG essentially gives the generation some idempotency, we won't
    // clutter the cases directory with new cases unless some strategy has changed.
    let rng = TestRng::deterministic_rng(config.rng_algorithm);

    let mut runner = TestRunner::new_with_rng(config, rng);

    runner
        .run(&strategy, move |input| {
            let input_bytes = input.encode();

            let input_hash = keccak_256(&input_bytes);

            let case_file = case_directory.as_ref().join(hex::encode(input_hash));

            // skip cases we've already generated
            if std::fs::exists(&case_file).unwrap() {
                return Ok(());
            }

            let output = f(input.clone());

            assert!(assert(&input, &output));

            let case = (input, output);

            let case_bytes = case.encode();

            std::fs::write(&case_file, &case_bytes).unwrap();

            Ok(())
        })
        .unwrap();
}
