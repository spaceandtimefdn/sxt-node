//! Abstraction for types that are generic over commitments.
//!
//! Contains [`GenericOverCommitment`] and its implementors.

use core::marker::PhantomData;

#[cfg(feature = "substrate")]
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen};
use proof_of_sql::base::commitment::{
    ColumnCommitments,
    Commitment,
    QueryCommitments,
    TableCommitment,
};
#[cfg(feature = "substrate")]
use scale_info::TypeInfo;

/// Abstraction for types that are generic over commitments.
///
/// This offers pseudo-higher-kinded-type functionality for one specific use case.
/// Good for code that..
/// - is intended to deal with all commitment types simultaneously
/// - doesn't actually care about the specifics of the type, just that it is commitment-generic.
pub trait GenericOverCommitment {
    /// Generic type associated with this concrete type.
    type WithCommitment<C: Commitment>;
}

/// Concrete type associated with `Commitment` implementors.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct CommitmentType;

impl GenericOverCommitment for CommitmentType {
    type WithCommitment<C: Commitment> = C;
}

/// Concrete type associated with the generic `ColumnCommitments<C: Commitment>`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct ColumnCommitmentsType;

impl GenericOverCommitment for ColumnCommitmentsType {
    type WithCommitment<C: Commitment> = ColumnCommitments<C>;
}

/// Concrete type associated with the generic `TableCommitment<C: Commitment>`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct TableCommitmentType;

impl GenericOverCommitment for TableCommitmentType {
    type WithCommitment<C: Commitment> = TableCommitment<C>;
}

/// Concrete type associated with the generic `QueryCommitments<C: Commitment>`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct QueryCommitmentsType;

impl GenericOverCommitment for QueryCommitmentsType {
    type WithCommitment<C: Commitment> = QueryCommitments<C>;
}

/// Concrete type associated with `Commitment` implementors' `C::PublicSetup` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct AssociatedPublicSetupType<'a>(PhantomData<&'a ()>);

impl<'a> GenericOverCommitment for AssociatedPublicSetupType<'a> {
    type WithCommitment<C: Commitment> = C::PublicSetup<'a>;
}

/// Concrete type associated with `Commitment` implementors' `C::Scalar` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct AssociatedScalarType;

impl GenericOverCommitment for AssociatedScalarType {
    type WithCommitment<C: Commitment> = C::Scalar;
}

/// Concrete type associated with `Option<G::WithCommitment<C: Commitment>>` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct OptionType<G: GenericOverCommitment>(PhantomData<G>);

impl<G: GenericOverCommitment> GenericOverCommitment for OptionType<G> {
    type WithCommitment<C: Commitment> = Option<G::WithCommitment<C>>;
}

/// Concrete type associated with `Result<G::WithCommitment<C: Commitment>, E>` types.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct ResultOkType<G: GenericOverCommitment, E>(PhantomData<G>, PhantomData<E>);

impl<G: GenericOverCommitment, E> GenericOverCommitment for ResultOkType<G, E> {
    type WithCommitment<C: Commitment> = Result<G::WithCommitment<C>, E>;
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
    type WithCommitment<C: Commitment> = (G0::WithCommitment<C>, G1::WithCommitment<C>);
}

/// Concrete type associated with `T`, which is not necessarily generic over commitments.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct ConcreteType<T>(PhantomData<T>);

impl<T> GenericOverCommitment for ConcreteType<T> {
    type WithCommitment<C: Commitment> = T;
}
