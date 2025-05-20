//! Abstraction for types that are generic over commitments.
//!
//! Contains [`GenericOverCommitment`] and its implementors.

use core::marker::PhantomData;

#[cfg(feature = "substrate")]
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen};
use proof_of_sql::base::commitment::{ColumnCommitments, QueryCommitments, TableCommitment};
#[cfg(feature = "substrate")]
use scale_info::TypeInfo;

use crate::CommitmentId;

/// Abstraction for types that are generic over commitments.
///
/// This offers pseudo-higher-kinded-type functionality for one specific use case.
/// Good for code that..
/// - is intended to deal with all commitment types simultaneously
/// - doesn't actually care about the specifics of the type, just that it is commitment-generic.
pub trait GenericOverCommitment {
    /// Generic type associated with this concrete type.
    type WithCommitment<C: CommitmentId>;
}

/// Concrete type associated with `Commitment` implementors.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct CommitmentType;

impl GenericOverCommitment for CommitmentType {
    type WithCommitment<C: CommitmentId> = C;
}

/// Concrete type associated with the generic `ColumnCommitments<C: CommitmentId>`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct ColumnCommitmentsType;

impl GenericOverCommitment for ColumnCommitmentsType {
    type WithCommitment<C: CommitmentId> = ColumnCommitments<C>;
}

/// Concrete type associated with the generic `TableCommitment<C: CommitmentId>`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct TableCommitmentType;

impl GenericOverCommitment for TableCommitmentType {
    type WithCommitment<C: CommitmentId> = TableCommitment<C>;
}

/// Concrete type associated with the generic `QueryCommitments<C: CommitmentId>`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct QueryCommitmentsType;

impl GenericOverCommitment for QueryCommitmentsType {
    type WithCommitment<C: CommitmentId> = QueryCommitments<C>;
}

/// Concrete type associated with `Commitment` implementors' `C::PublicSetup` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct AssociatedPublicSetupType<'a>(PhantomData<&'a ()>);

impl<'a> GenericOverCommitment for AssociatedPublicSetupType<'a> {
    type WithCommitment<C: CommitmentId> = C::PublicSetup<'a>;
}

/// Concrete type associated with `Commitment` implementors' `C::Scalar` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct AssociatedScalarType;

impl GenericOverCommitment for AssociatedScalarType {
    type WithCommitment<C: CommitmentId> = C::Scalar;
}

/// Concrete type associated with `Option<G::WithCommitment<C: CommitmentId>>` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct OptionType<G: GenericOverCommitment>(PhantomData<G>);

impl<G: GenericOverCommitment> GenericOverCommitment for OptionType<G> {
    type WithCommitment<C: CommitmentId> = Option<G::WithCommitment<C>>;
}

/// Concrete type associated with `Result<G::WithCommitment<C: CommitmentId>, E>` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct ResultOkType<G: GenericOverCommitment, E>(PhantomData<G>, PhantomData<E>);

impl<G: GenericOverCommitment, E> GenericOverCommitment for ResultOkType<G, E> {
    type WithCommitment<C: CommitmentId> = Result<G::WithCommitment<C>, E>;
}

/// Concrete type associated with a 2-tuple `(G0::WithCommitment<C>, G1::WithCommitment<C>)`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct PairType<G0: GenericOverCommitment, G1: GenericOverCommitment>(
    PhantomData<G0>,
    PhantomData<G1>,
);

impl<G0: GenericOverCommitment, G1: GenericOverCommitment> GenericOverCommitment
    for PairType<G0, G1>
{
    type WithCommitment<C: CommitmentId> = (G0::WithCommitment<C>, G1::WithCommitment<C>);
}

/// Concrete type associated with `T`, which is not necessarily generic over commitments.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct ConcreteType<T>(PhantomData<T>);

impl<T> GenericOverCommitment for ConcreteType<T> {
    type WithCommitment<C: CommitmentId> = T;
}
