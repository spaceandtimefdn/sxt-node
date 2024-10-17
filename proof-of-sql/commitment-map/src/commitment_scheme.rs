use curve25519_dalek::RistrettoPoint;
#[cfg(feature = "substrate")]
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen};
use proof_of_sql::proof_primitive::dory::DoryCommitment;
#[cfg(feature = "substrate")]
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

use crate::generic_over_commitment::{
    ConcreteType,
    GenericOverCommitment,
    OptionType,
    PairType,
    ResultOkType,
};
use crate::GenericOverCommitmentFn;

/// Identifier for proof-of-sql commitment schemes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub enum CommitmentScheme {
    /// Scheme with commitments in the ristretto group, proven by inner-product-argument.
    Ipa,
    /// Scheme with dory commitments.
    Dory,
}

/// Flags for selecting a combination of proof-of-sql commitment schemes.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct CommitmentSchemeFlags {
    /// Select [`CommitmentScheme::Ipa`].
    pub ipa: bool,
    /// Select [`CommitmentScheme::Dory`].
    pub dory: bool,
}

impl CommitmentSchemeFlags {
    /// Construct a [`CommitmentSchemeFlags`] with all schemes selected.
    pub const fn all() -> Self {
        CommitmentSchemeFlags {
            ipa: true,
            dory: true,
        }
    }
}

impl FromIterator<CommitmentScheme> for CommitmentSchemeFlags {
    fn from_iter<T: IntoIterator<Item = CommitmentScheme>>(iter: T) -> Self {
        iter.into_iter().fold(
            CommitmentSchemeFlags::default(),
            |acc, scheme| match scheme {
                CommitmentScheme::Ipa => CommitmentSchemeFlags { ipa: true, ..acc },
                CommitmentScheme::Dory => CommitmentSchemeFlags { dory: true, ..acc },
            },
        )
    }
}

impl IntoIterator for CommitmentSchemeFlags {
    type Item = CommitmentScheme;
    type IntoIter =
        core::iter::Chain<core::option::IntoIter<Self::Item>, core::option::IntoIter<Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        let CommitmentSchemeFlags { ipa, dory } = self;

        itertools::chain!(
            ipa.then_some(CommitmentScheme::Ipa),
            dory.then_some(CommitmentScheme::Dory)
        )
    }
}

/// Commitment-associated data of any commitment scheme.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub enum AnyCommitmentScheme<T: GenericOverCommitment> {
    /// Data with [`CommitmentScheme::Ipa`].
    Ipa(T::WithCommitment<RistrettoPoint>),
    /// Data with [`CommitmentScheme::Dory`].
    Dory(T::WithCommitment<DoryCommitment>),
}

impl<T: GenericOverCommitment> AnyCommitmentScheme<T> {
    /// Returns the scheme of this commitment.
    pub fn to_scheme(&self) -> CommitmentScheme {
        self.into()
    }

    /// Maps a `AnyCommitmentScheme<T>` to an `AnyCommitmentScheme<M::Out>` by applying the mapper.
    pub fn map<M>(self, mapper: M) -> AnyCommitmentScheme<M::Out>
    where
        M: GenericOverCommitmentFn<In = T>,
    {
        match self {
            AnyCommitmentScheme::Ipa(data) => AnyCommitmentScheme::Ipa(mapper.call(data)),
            AnyCommitmentScheme::Dory(data) => AnyCommitmentScheme::Dory(mapper.call(data)),
        }
    }
}

impl<T: GenericOverCommitment> AnyCommitmentScheme<OptionType<T>> {
    /// Transpose an `AnyCommitmentScheme<Option<T>>` to an `Option<AnyCommitmentScheme<T>>`.
    pub fn transpose_option(self) -> Option<AnyCommitmentScheme<T>> {
        match self {
            AnyCommitmentScheme::Ipa(Some(data)) => Some(AnyCommitmentScheme::Ipa(data)),
            AnyCommitmentScheme::Dory(Some(data)) => Some(AnyCommitmentScheme::Dory(data)),
            AnyCommitmentScheme::Ipa(None) | AnyCommitmentScheme::Dory(None) => None,
        }
    }
}

impl<T: GenericOverCommitment, E> AnyCommitmentScheme<ResultOkType<T, E>> {
    /// Transpose an `AnyCommitmentScheme<Result<T, E>>` to an `Result<AnyCommitmentScheme<T>, E>`.
    pub fn transpose_result(self) -> Result<AnyCommitmentScheme<T>, E> {
        match self {
            AnyCommitmentScheme::Ipa(Ok(data)) => Ok(AnyCommitmentScheme::Ipa(data)),
            AnyCommitmentScheme::Dory(Ok(data)) => Ok(AnyCommitmentScheme::Dory(data)),
            AnyCommitmentScheme::Ipa(Err(e)) | AnyCommitmentScheme::Dory(Err(e)) => Err(e),
        }
    }
}

impl<T: GenericOverCommitment, U: GenericOverCommitment> AnyCommitmentScheme<PairType<T, U>> {
    /// Unzips a `AnyCommitmentScheme` containing a pair into a pair of `AnyCommitmentScheme`s.
    pub fn unzip(self) -> (AnyCommitmentScheme<T>, AnyCommitmentScheme<U>) {
        match self {
            AnyCommitmentScheme::Ipa((left, right)) => (
                AnyCommitmentScheme::Ipa(left),
                AnyCommitmentScheme::Ipa(right),
            ),
            AnyCommitmentScheme::Dory((left, right)) => (
                AnyCommitmentScheme::Dory(left),
                AnyCommitmentScheme::Dory(right),
            ),
        }
    }
}

impl<T> AnyCommitmentScheme<ConcreteType<T>> {
    /// Unwraps an `AnyCommitmentScheme` with a concrete type into its internal value
    pub fn unwrap(self) -> T {
        match self {
            AnyCommitmentScheme::Ipa(data) => data,
            AnyCommitmentScheme::Dory(data) => data,
        }
    }
}

impl<T: GenericOverCommitment> From<&AnyCommitmentScheme<T>> for CommitmentScheme {
    fn from(commitment: &AnyCommitmentScheme<T>) -> Self {
        match commitment {
            AnyCommitmentScheme::Ipa(_) => CommitmentScheme::Ipa,
            AnyCommitmentScheme::Dory(_) => CommitmentScheme::Dory,
        }
    }
}

/// Collection of commitment-associated data, with one element per commitment scheme.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct PerCommitmentScheme<T: GenericOverCommitment> {
    /// Element with [`CommitmentScheme::Ipa`].
    pub ipa: T::WithCommitment<RistrettoPoint>,
    /// Element with [`CommitmentScheme::Dory`].
    pub dory: T::WithCommitment<DoryCommitment>,
}

impl<T: GenericOverCommitment> PerCommitmentScheme<T> {
    /// Maps a `PerCommitmentScheme<T>` to a `PerCommitmentScheme<M::Out>` by applying the mapper.
    pub fn map<M>(self, mapper: M) -> PerCommitmentScheme<M::Out>
    where
        M: GenericOverCommitmentFn<In = T>,
    {
        PerCommitmentScheme {
            ipa: mapper.call(self.ipa),
            dory: mapper.call(self.dory),
        }
    }

    /// Returns this collection including only the elements selected by `flags`.
    pub fn select(self, flags: &CommitmentSchemeFlags) -> PerCommitmentScheme<OptionType<T>> {
        PerCommitmentScheme {
            ipa: flags.ipa.then_some(self.ipa),
            dory: flags.dory.then_some(self.dory),
        }
    }

    /// Zips `self` with another `PerCommitmentScheme`.
    pub fn zip<U: GenericOverCommitment>(
        self,
        other: PerCommitmentScheme<U>,
    ) -> PerCommitmentScheme<PairType<T, U>> {
        PerCommitmentScheme {
            ipa: (self.ipa, other.ipa),
            dory: (self.dory, other.dory),
        }
    }
}

impl<T: GenericOverCommitment, U: GenericOverCommitment> PerCommitmentScheme<PairType<T, U>> {
    /// Unzips a `PerCommitmentScheme` containing a pair into a pair of `PerCommitmentScheme`s.
    pub fn unzip(self) -> (PerCommitmentScheme<T>, PerCommitmentScheme<U>) {
        (
            PerCommitmentScheme {
                ipa: self.ipa.0,
                dory: self.dory.0,
            },
            PerCommitmentScheme {
                ipa: self.ipa.1,
                dory: self.dory.1,
            },
        )
    }
}

impl<T: GenericOverCommitment> PerCommitmentScheme<OptionType<T>> {
    /// Returns the schemes present in this collection as a [`CommitmentSchemeFlags`].
    pub fn to_flags(&self) -> CommitmentSchemeFlags {
        self.into()
    }

    /// Returns an iterator over `AnyCommitmentScheme<T>`, flattening out the internal `Option`.
    pub fn into_flat_iter(self) -> impl Iterator<Item = AnyCommitmentScheme<T>> {
        self.into_iter()
            .flat_map(AnyCommitmentScheme::transpose_option)
    }
}

impl<T: GenericOverCommitment> From<&PerCommitmentScheme<OptionType<T>>> for CommitmentSchemeFlags {
    fn from(PerCommitmentScheme { ipa, dory }: &PerCommitmentScheme<OptionType<T>>) -> Self {
        CommitmentSchemeFlags {
            ipa: ipa.is_some(),
            dory: dory.is_some(),
        }
    }
}

impl<T: GenericOverCommitment> Default for PerCommitmentScheme<OptionType<T>> {
    fn default() -> Self {
        PerCommitmentScheme {
            ipa: None,
            dory: None,
        }
    }
}

impl<T: GenericOverCommitment> IntoIterator for PerCommitmentScheme<T> {
    type Item = AnyCommitmentScheme<T>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let PerCommitmentScheme { ipa, dory } = self;

        alloc::vec![
            AnyCommitmentScheme::Ipa(ipa),
            AnyCommitmentScheme::Dory(dory),
        ]
        .into_iter()
    }
}

impl<G: GenericOverCommitment> FromIterator<AnyCommitmentScheme<G>>
    for PerCommitmentScheme<OptionType<G>>
{
    fn from_iter<T: IntoIterator<Item = AnyCommitmentScheme<G>>>(iter: T) -> Self {
        iter.into_iter()
            .fold(PerCommitmentScheme::default(), |acc, scheme| match scheme {
                AnyCommitmentScheme::Ipa(data) => PerCommitmentScheme {
                    ipa: Some(data),
                    ..acc
                },
                AnyCommitmentScheme::Dory(data) => PerCommitmentScheme {
                    dory: Some(data),
                    ..acc
                },
            })
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use proof_of_sql::base::scalar::{Curve25519Scalar, Scalar};
    use proof_of_sql::proof_primitive::dory::DoryScalar;

    use super::*;
    use crate::generic_over_commitment::{AssociatedScalarType, CommitmentType};
    use crate::generic_over_commitment_fn::tests::SomeFn;

    #[test]
    fn we_can_iterate_over_commitment_schemes_in_commitment_scheme_flags() {
        let no_flags = CommitmentSchemeFlags {
            ipa: false,
            dory: false,
        };
        assert_eq!(Vec::from_iter(no_flags), vec![]);

        let ipa_flags = CommitmentSchemeFlags {
            ipa: true,
            dory: false,
        };
        assert_eq!(Vec::from_iter(ipa_flags), vec![CommitmentScheme::Ipa]);

        let dory_flags = CommitmentSchemeFlags {
            ipa: false,
            dory: true,
        };
        assert_eq!(Vec::from_iter(dory_flags), vec![CommitmentScheme::Dory]);

        let all_flags = CommitmentSchemeFlags::all();
        assert_eq!(
            Vec::from_iter(all_flags),
            vec![CommitmentScheme::Ipa, CommitmentScheme::Dory]
        );
    }

    #[test]
    fn we_can_collect_commitment_schemes_into_commitment_scheme_flags() {
        let no_flags = CommitmentSchemeFlags::from_iter(None);
        assert_eq!(no_flags, CommitmentSchemeFlags::default());

        let ipa_flags = CommitmentSchemeFlags::from_iter([CommitmentScheme::Ipa]);
        assert_eq!(
            ipa_flags,
            CommitmentSchemeFlags {
                ipa: true,
                dory: false
            }
        );

        let dory_flags = CommitmentSchemeFlags::from_iter([CommitmentScheme::Dory]);
        assert_eq!(
            dory_flags,
            CommitmentSchemeFlags {
                ipa: false,
                dory: true
            }
        );

        let all_flags =
            CommitmentSchemeFlags::from_iter([CommitmentScheme::Ipa, CommitmentScheme::Dory]);
        assert_eq!(all_flags, CommitmentSchemeFlags::all());
    }

    #[test]
    fn we_can_iterate_over_commitments_in_per_commitment_scheme() {
        let all_commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: Default::default(),
            dory: Default::default(),
        };
        assert_eq!(
            Vec::from_iter(all_commitments),
            vec![
                AnyCommitmentScheme::<CommitmentType>::Ipa(Default::default()),
                AnyCommitmentScheme::<CommitmentType>::Dory(Default::default())
            ]
        );
    }

    #[test]
    fn we_can_convert_any_commitment_scheme_to_scheme() {
        let ipa_commitment = AnyCommitmentScheme::<CommitmentType>::Ipa(Default::default());
        assert_eq!(ipa_commitment.to_scheme(), CommitmentScheme::Ipa);

        let dory_commitment = AnyCommitmentScheme::<CommitmentType>::Dory(Default::default());
        assert_eq!(dory_commitment.to_scheme(), CommitmentScheme::Dory);
    }

    #[test]
    fn we_can_convert_per_commitment_scheme_to_flags() {
        let no_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: None,
            dory: None,
        };
        assert_eq!(no_commitments.to_flags(), CommitmentSchemeFlags::default());

        let ipa_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: Some(Default::default()),
            dory: None,
        };
        assert_eq!(
            ipa_commitments.to_flags(),
            CommitmentSchemeFlags {
                ipa: true,
                dory: false
            }
        );

        let dory_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: None,
            dory: Some(Default::default()),
        };
        assert_eq!(
            dory_commitments.to_flags(),
            CommitmentSchemeFlags {
                ipa: false,
                dory: true
            }
        );

        let all_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: Some(Default::default()),
            dory: Some(Default::default()),
        };
        assert_eq!(all_commitments.to_flags(), CommitmentSchemeFlags::all());
    }

    #[test]
    fn we_can_transpose_any_commitment_scheme_with_option_type() {
        let ipa_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::Ipa(Some(Default::default()));
        assert_eq!(
            ipa_commitment.transpose_option(),
            Some(AnyCommitmentScheme::Ipa(Default::default()))
        );

        let dory_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::Dory(Some(Default::default()));
        assert_eq!(
            dory_commitment.transpose_option(),
            Some(AnyCommitmentScheme::Dory(Default::default()))
        );

        let ipa_commitment = AnyCommitmentScheme::<OptionType<CommitmentType>>::Ipa(None);
        assert_eq!(ipa_commitment.transpose_option(), None);

        let dory_commitment = AnyCommitmentScheme::<OptionType<CommitmentType>>::Dory(None);
        assert_eq!(dory_commitment.transpose_option(), None);
    }

    #[test]
    fn we_can_transpose_any_commitment_scheme_with_result_type() {
        let ipa_commitment =
            AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::Ipa(Ok(Default::default()));
        assert_eq!(
            ipa_commitment.transpose_result(),
            Ok(AnyCommitmentScheme::Ipa(Default::default()))
        );

        let dory_commitment = AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::Dory(Ok(
            Default::default(),
        ));
        assert_eq!(
            dory_commitment.transpose_result(),
            Ok(AnyCommitmentScheme::Dory(Default::default()))
        );

        let ipa_commitment =
            AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::Ipa(Err(1));
        assert_eq!(ipa_commitment.transpose_result(), Err(1));

        let dory_commitment =
            AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::Dory(Err(2));
        assert_eq!(dory_commitment.transpose_result(), Err(2));
    }

    #[test]
    fn we_can_collect_per_commitment_scheme_with_option_type_from_iter_and_into_flat_iter() {
        let no_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: None,
            dory: None,
        };
        let no_iterator = vec![];
        assert_eq!(
            no_commitments.clone().into_flat_iter().collect::<Vec<_>>(),
            no_iterator.clone()
        );
        assert_eq!(PerCommitmentScheme::from_iter(no_iterator), no_commitments);

        let ipa_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: Some(Default::default()),
            dory: None,
        };
        let ipa_iterator = vec![AnyCommitmentScheme::<CommitmentType>::Ipa(
            Default::default(),
        )];
        assert_eq!(
            ipa_commitments.clone().into_flat_iter().collect::<Vec<_>>(),
            ipa_iterator.clone(),
        );
        assert_eq!(
            PerCommitmentScheme::from_iter(ipa_iterator),
            ipa_commitments
        );

        let dory_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: None,
            dory: Some(Default::default()),
        };
        let dory_iterator = vec![AnyCommitmentScheme::<CommitmentType>::Dory(
            Default::default(),
        )];
        assert_eq!(
            dory_commitments
                .clone()
                .into_flat_iter()
                .collect::<Vec<_>>(),
            dory_iterator.clone(),
        );
        assert_eq!(
            PerCommitmentScheme::from_iter(dory_iterator),
            dory_commitments
        );

        let all_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: Some(Default::default()),
            dory: Some(Default::default()),
        };
        let all_iterator = vec![
            AnyCommitmentScheme::<CommitmentType>::Ipa(Default::default()),
            AnyCommitmentScheme::<CommitmentType>::Dory(Default::default()),
        ];
        assert_eq!(
            all_commitments.clone().into_flat_iter().collect::<Vec<_>>(),
            all_iterator.clone()
        );
        assert_eq!(
            PerCommitmentScheme::from_iter(all_iterator),
            all_commitments
        );
    }

    #[test]
    fn we_can_map_any_commitment_scheme_to_another() {
        let some_fn = SomeFn::<CommitmentType>::new();

        let ipa_commitment = AnyCommitmentScheme::<CommitmentType>::Ipa(Default::default());
        let some_ipa_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::Ipa(Some(Default::default()));
        assert_eq!(ipa_commitment.map(&some_fn), some_ipa_commitment);

        let dory_commitment = AnyCommitmentScheme::<CommitmentType>::Ipa(Default::default());
        let some_dory_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::Ipa(Some(Default::default()));
        assert_eq!(dory_commitment.map(some_fn), some_dory_commitment);
    }

    #[test]
    fn we_can_map_per_commitment_scheme_to_another() {
        let some_fn = SomeFn::<CommitmentType>::new();

        let per_commitment_scheme = PerCommitmentScheme::<CommitmentType> {
            ipa: Default::default(),
            dory: Default::default(),
        };
        let some_per_commitment_scheme = PerCommitmentScheme::<OptionType<CommitmentType>> {
            ipa: Some(Default::default()),
            dory: Some(Default::default()),
        };

        assert_eq!(
            per_commitment_scheme.map(some_fn),
            some_per_commitment_scheme
        );
    }

    #[test]
    fn we_can_select_per_commitment_scheme_by_flags() {
        let per_commitment_scheme = PerCommitmentScheme::<CommitmentType> {
            ipa: Default::default(),
            dory: Default::default(),
        };

        let no_flags = CommitmentSchemeFlags::default();
        assert_eq!(
            per_commitment_scheme.clone().select(&no_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>>::default()
        );

        let ipa_flags = CommitmentSchemeFlags {
            ipa: true,
            ..Default::default()
        };
        assert_eq!(
            per_commitment_scheme.clone().select(&ipa_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>> {
                ipa: Some(Default::default()),
                dory: None,
            }
        );

        let dory_flags = CommitmentSchemeFlags {
            dory: true,
            ..Default::default()
        };
        assert_eq!(
            per_commitment_scheme.clone().select(&dory_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>> {
                ipa: None,
                dory: Some(Default::default()),
            }
        );

        let all_flags = CommitmentSchemeFlags::all();
        assert_eq!(
            per_commitment_scheme.clone().select(&all_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>> {
                ipa: Some(Default::default()),
                dory: Some(Default::default()),
            }
        );
    }

    #[test]
    fn we_can_zip_and_unzip_per_commitment_scheme() {
        let commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: Default::default(),
            dory: Default::default(),
        };

        let scalars = PerCommitmentScheme::<AssociatedScalarType> {
            ipa: Curve25519Scalar::ZERO,
            dory: DoryScalar::ONE,
        };

        let commitments_with_scalars =
            PerCommitmentScheme::<PairType<CommitmentType, AssociatedScalarType>> {
                ipa: (Default::default(), Curve25519Scalar::ZERO),
                dory: (Default::default(), DoryScalar::ONE),
            };

        assert_eq!(
            commitments.clone().zip(scalars.clone()),
            commitments_with_scalars
        );
        assert_eq!(commitments_with_scalars.unzip(), (commitments, scalars));
    }

    #[test]
    fn we_can_unzip_any_commitment_scheme() {
        let ipa_commitment_with_scalar =
            AnyCommitmentScheme::<PairType<CommitmentType, AssociatedScalarType>>::Ipa((
                Default::default(),
                Curve25519Scalar::ONE,
            ));
        assert_eq!(
            ipa_commitment_with_scalar.unzip(),
            (
                AnyCommitmentScheme::<CommitmentType>::Ipa(Default::default()),
                AnyCommitmentScheme::<AssociatedScalarType>::Ipa(Curve25519Scalar::ONE)
            )
        );

        let dory_commitment_with_scalar = AnyCommitmentScheme::<
            PairType<CommitmentType, AssociatedScalarType>,
        >::Dory((Default::default(), DoryScalar::TWO));
        assert_eq!(
            dory_commitment_with_scalar.unzip(),
            (
                AnyCommitmentScheme::<CommitmentType>::Dory(Default::default()),
                AnyCommitmentScheme::<AssociatedScalarType>::Dory(DoryScalar::TWO)
            )
        );
    }

    #[test]
    fn we_can_unwrap_any_commitment_scheme_with_concrete_type() {
        let ipa_usize = AnyCommitmentScheme::<ConcreteType<usize>>::Ipa(123);
        assert_eq!(ipa_usize.unwrap(), 123);

        let dory_usize = AnyCommitmentScheme::<ConcreteType<usize>>::Dory(456);
        assert_eq!(dory_usize.unwrap(), 456);
    }
}
