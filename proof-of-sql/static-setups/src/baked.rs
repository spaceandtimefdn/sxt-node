//! Contains static public setups whose data is baked into the compiled binary.
//!
//! Useful in non-std environments.
use ark_serialize::{CanonicalDeserialize, Compress, Validate};
use proof_of_sql::proof_primitive::dory::{ProverSetup, PublicParameters};
use proof_of_sql::proof_primitive::hyperkzg::{
    deserialize_flat_compressed_hyperkzg_public_setup_from_slice,
    HyperKZGPublicSetupOwned,
};
use proof_of_sql_commitment_map::generic_over_commitment::AssociatedPublicSetupType;
use proof_of_sql_commitment_map::PerCommitmentScheme;

/// Ark-serialized bytes of proof-of-sql public parameters, generated with...
/// - nu of 1
/// - ChaCha20Rng with seed "SpaceAndTime"
const PUBLIC_PARAMETERS_BYTES: &[u8; 1064] = include_bytes!("../public_parameters_nu_1");

/// Ark-serialized bytes of proof-of-sql public parameters with degree 2
const PPOT_BYTES: &[u8; 128] = include_bytes!("../ppot_0080_02.bin");

lazy_static::lazy_static! {
    /// Proof-of-sql PublicParameters, built from [`PUBLIC_PARAMETERS_BYTES`].
    static ref PUBLIC_PARAMETERS: PublicParameters = PublicParameters::deserialize_with_mode(
        &PUBLIC_PARAMETERS_BYTES[..],
        Compress::No,
        Validate::No,
    )
    .unwrap();

    static ref HYPERKZG_PUBLIC_SETUP: HyperKZGPublicSetupOwned = deserialize_flat_compressed_hyperkzg_public_setup_from_slice(
        &PPOT_BYTES[..],
        Validate::No,
    )
    .unwrap();

    /// Proof-of-sql dory public setup.
    static ref DORY_PUBLIC_SETUP: ProverSetup<'static> = ProverSetup::from(&*PUBLIC_PARAMETERS);

    /// Proof-of-sql public setups for all commitment schemes.
    pub static ref PUBLIC_SETUPS: PerCommitmentScheme<AssociatedPublicSetupType<'static>> =
        PerCommitmentScheme {
            hyper_kzg: &*HYPERKZG_PUBLIC_SETUP,
            dynamic_dory: &*DORY_PUBLIC_SETUP,
        };
}
