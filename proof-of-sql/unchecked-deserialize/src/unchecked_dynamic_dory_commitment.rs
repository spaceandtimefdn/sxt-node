//! This module provides `UncheckedDynamicDoryCommitment`, a wrapper around `DynamicDoryCommitment` that allows for unchecked deserialization.

use core::ops::Mul;

use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use derive_more::{AddAssign, Neg, Sub, SubAssign};
use proof_of_sql::base::commitment::{Commitment, CommittableColumn};
use proof_of_sql::proof_primitive::dory::{DoryScalar, DynamicDoryCommitment};

#[derive(
    Debug,
    Sub,
    Eq,
    PartialEq,
    Neg,
    Copy,
    Clone,
    AddAssign,
    SubAssign,
    CanonicalSerialize,
    CanonicalDeserialize,
    Default,
)]

/// A wrapper around `DynamicDoryCommitment` with unchecked deserialization.
/// Note: while it implements the `Commitment` trait, it does not actually implement its methods.
/// This should `only` be used for unchecked deserialization, and then converted to `DynamicDoryCommitment`.
pub struct UncheckedDynamicDoryCommitment(DynamicDoryCommitment);

impl From<UncheckedDynamicDoryCommitment> for DynamicDoryCommitment {
    fn from(value: UncheckedDynamicDoryCommitment) -> Self {
        value.0
    }
}
impl From<&UncheckedDynamicDoryCommitment> for DynamicDoryCommitment {
    fn from(value: &UncheckedDynamicDoryCommitment) -> Self {
        value.0
    }
}

impl serde::Serialize for UncheckedDynamicDoryCommitment {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut bytes = Vec::with_capacity(CanonicalSerialize::compressed_size(self));
        CanonicalSerialize::serialize_compressed(self, &mut bytes)
            .map_err(serde::ser::Error::custom)?;
        bytes.serialize(serializer)
    }
}
impl<'de> serde::Deserialize<'de> for UncheckedDynamicDoryCommitment {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        CanonicalDeserialize::deserialize_compressed_unchecked(
            Vec::deserialize(deserializer)?.as_slice(),
        )
        .map_err(serde::de::Error::custom)
    }
}
impl Mul<UncheckedDynamicDoryCommitment> for DoryScalar {
    type Output = UncheckedDynamicDoryCommitment;
    fn mul(self, _rhs: UncheckedDynamicDoryCommitment) -> Self::Output {
        unimplemented!()
    }
}
impl Mul<&UncheckedDynamicDoryCommitment> for DoryScalar {
    type Output = UncheckedDynamicDoryCommitment;
    fn mul(self, _rhs: &UncheckedDynamicDoryCommitment) -> Self::Output {
        unimplemented!()
    }
}
impl Commitment for UncheckedDynamicDoryCommitment {
    type Scalar = DoryScalar;
    type PublicSetup<'a> = ();
    fn compute_commitments(
        _committable_columns: &[CommittableColumn],
        _offset: usize,
        _setup: &Self::PublicSetup<'_>,
    ) -> Vec<Self> {
        unimplemented!()
    }

    fn to_transcript_bytes(&self) -> Vec<u8> {
        unimplemented!()
    }
}
