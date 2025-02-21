#![no_std]
#![doc = include_str!("../README.md")]

use ark_serialize::{CanonicalDeserialize, Compress, Validate};
use proof_of_sql::proof_primitive::dory::{ProverSetup, PublicParameters};
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::PerCommitmentScheme;

/// Ark-serialized bytes of proof-of-sql public parameters, generated with...
/// - nu of 1
/// - ChaCha20Rng with seed "SpaceAndTime"
#[cfg(all(feature = "nu_1", not(feature = "nu_14")))]
const PUBLIC_PARAMETERS_BYTES: &[u8; 1064] = include_bytes!("../public_parameters_nu_1");

/// Ark-serialized bytes of proof-of-sql public parameters, generated with...
/// - nu of 14
/// - ChaCha20Rng with seed "SpaceAndTime"
#[cfg(feature = "nu_14")]
const PUBLIC_PARAMETERS_BYTES: &[u8; 4719080] = include_bytes!("../public_parameters_nu_14");

lazy_static::lazy_static! {
    /// Proof-of-sql PublicParameters, built from [`PUBLIC_PARAMETERS_BYTES`].
    static ref PUBLIC_PARAMETERS: PublicParameters = PublicParameters::deserialize_with_mode(
        &PUBLIC_PARAMETERS_BYTES[..],
        Compress::No,
        Validate::No,
    )
    .unwrap();

    /// Proof-of-sql prover setup.
    static ref PROVER_SETUP: ProverSetup<'static> = ProverSetup::from(&*PUBLIC_PARAMETERS);

    /// Proof-of-sql public setups for all commitment schemes.
    pub static ref PUBLIC_SETUPS: PerCommitmentScheme<AssociatedPublicSetupType<'static>> =
        PerCommitmentScheme {
            ipa: (),
            dynamic_dory: &*PROVER_SETUP,
        };
}
