#[cfg(feature = "substrate")]
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen};
use proof_of_sql::base::commitment::Commitment;
use proof_of_sql::proof_primitive::dory::DynamicDoryCommitment;
use proof_of_sql::proof_primitive::hyperkzg::HyperKZGCommitment;
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
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub enum CommitmentScheme {
    /// Scheme with commitments in the ristretto group, proven by inner-product-argument.
    HyperKzg,
    /// Scheme with dory commitments.
    DynamicDory,
}

impl CommitmentScheme {
    /// Returns `AnyCommitmentScheme(value)` with the appropriate [`CommitmentScheme`] variant.
    pub fn into_any_concrete<T>(self, value: T) -> AnyCommitmentScheme<ConcreteType<T>> {
        match self {
            CommitmentScheme::HyperKzg => AnyCommitmentScheme::HyperKzg(value),
            CommitmentScheme::DynamicDory => AnyCommitmentScheme::DynamicDory(value),
        }
    }
}

/// Trait for commitment types that defines their associated [`CommitmentScheme`].
pub trait CommitmentId: Commitment + Serialize + for<'de> Deserialize<'de> {
    /// The [`CommitmentScheme`] associated with this commitment type.
    const COMMITMENT_SCHEME: CommitmentScheme;
}

impl CommitmentId for HyperKZGCommitment {
    const COMMITMENT_SCHEME: CommitmentScheme = CommitmentScheme::HyperKzg;
}

impl CommitmentId for DynamicDoryCommitment {
    const COMMITMENT_SCHEME: CommitmentScheme = CommitmentScheme::DynamicDory;
}

/// Flags for selecting a combination of proof-of-sql commitment schemes.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct CommitmentSchemeFlags {
    /// Select [`CommitmentScheme::Ipa`].
    pub hyper_kzg: bool,
    /// Select [`CommitmentScheme::DynamicDory`].
    pub dynamic_dory: bool,
}

impl CommitmentSchemeFlags {
    /// Construct a [`CommitmentSchemeFlags`] with all schemes selected.
    pub const fn all() -> Self {
        CommitmentSchemeFlags {
            hyper_kzg: true,
            dynamic_dory: true,
        }
    }
}

impl FromIterator<CommitmentScheme> for CommitmentSchemeFlags {
    fn from_iter<T: IntoIterator<Item = CommitmentScheme>>(iter: T) -> Self {
        iter.into_iter().fold(
            CommitmentSchemeFlags::default(),
            |acc, scheme| match scheme {
                CommitmentScheme::HyperKzg => CommitmentSchemeFlags {
                    hyper_kzg: true,
                    ..acc
                },
                CommitmentScheme::DynamicDory => CommitmentSchemeFlags {
                    dynamic_dory: true,
                    ..acc
                },
            },
        )
    }
}

impl IntoIterator for CommitmentSchemeFlags {
    type Item = CommitmentScheme;
    type IntoIter =
        core::iter::Chain<core::option::IntoIter<Self::Item>, core::option::IntoIter<Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        let CommitmentSchemeFlags {
            hyper_kzg,
            dynamic_dory,
        } = self;

        itertools::chain!(
            hyper_kzg.then_some(CommitmentScheme::HyperKzg),
            dynamic_dory.then_some(CommitmentScheme::DynamicDory)
        )
    }
}

/// Commitment-associated data of any commitment scheme.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub enum AnyCommitmentScheme<T: GenericOverCommitment> {
    /// Data with [`CommitmentScheme::Ipa`].
    HyperKzg(T::WithCommitment<HyperKZGCommitment>),
    /// Data with [`CommitmentScheme::DynamicDory`].
    DynamicDory(T::WithCommitment<DynamicDoryCommitment>),
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
            AnyCommitmentScheme::HyperKzg(data) => AnyCommitmentScheme::HyperKzg(mapper.call(data)),
            AnyCommitmentScheme::DynamicDory(data) => {
                AnyCommitmentScheme::DynamicDory(mapper.call(data))
            }
        }
    }
}

impl<T: GenericOverCommitment> AnyCommitmentScheme<OptionType<T>> {
    /// Transpose an `AnyCommitmentScheme<Option<T>>` to an `Option<AnyCommitmentScheme<T>>`.
    pub fn transpose_option(self) -> Option<AnyCommitmentScheme<T>> {
        match self {
            AnyCommitmentScheme::HyperKzg(Some(data)) => Some(AnyCommitmentScheme::HyperKzg(data)),
            AnyCommitmentScheme::DynamicDory(Some(data)) => {
                Some(AnyCommitmentScheme::DynamicDory(data))
            }
            AnyCommitmentScheme::HyperKzg(None) | AnyCommitmentScheme::DynamicDory(None) => None,
        }
    }
}

impl<T: GenericOverCommitment, E> AnyCommitmentScheme<ResultOkType<T, E>> {
    /// Transpose an `AnyCommitmentScheme<Result<T, E>>` to an `Result<AnyCommitmentScheme<T>, E>`.
    pub fn transpose_result(self) -> Result<AnyCommitmentScheme<T>, E> {
        match self {
            AnyCommitmentScheme::HyperKzg(Ok(data)) => Ok(AnyCommitmentScheme::HyperKzg(data)),
            AnyCommitmentScheme::DynamicDory(Ok(data)) => {
                Ok(AnyCommitmentScheme::DynamicDory(data))
            }
            AnyCommitmentScheme::HyperKzg(Err(e)) | AnyCommitmentScheme::DynamicDory(Err(e)) => {
                Err(e)
            }
        }
    }
}

impl<T: GenericOverCommitment, U: GenericOverCommitment> AnyCommitmentScheme<PairType<T, U>> {
    /// Unzips a `AnyCommitmentScheme` containing a pair into a pair of `AnyCommitmentScheme`s.
    pub fn unzip(self) -> (AnyCommitmentScheme<T>, AnyCommitmentScheme<U>) {
        match self {
            AnyCommitmentScheme::HyperKzg((left, right)) => (
                AnyCommitmentScheme::HyperKzg(left),
                AnyCommitmentScheme::HyperKzg(right),
            ),
            AnyCommitmentScheme::DynamicDory((left, right)) => (
                AnyCommitmentScheme::DynamicDory(left),
                AnyCommitmentScheme::DynamicDory(right),
            ),
        }
    }
}

impl<T> AnyCommitmentScheme<ConcreteType<T>> {
    /// Unwraps an `AnyCommitmentScheme` with a concrete type into its internal value
    pub fn unwrap(self) -> T {
        match self {
            AnyCommitmentScheme::HyperKzg(data) => data,
            AnyCommitmentScheme::DynamicDory(data) => data,
        }
    }
}

impl<T: GenericOverCommitment> From<&AnyCommitmentScheme<T>> for CommitmentScheme {
    fn from(commitment: &AnyCommitmentScheme<T>) -> Self {
        match commitment {
            AnyCommitmentScheme::HyperKzg(_) => CommitmentScheme::HyperKzg,
            AnyCommitmentScheme::DynamicDory(_) => CommitmentScheme::DynamicDory,
        }
    }
}

/// Collection of commitment-associated data, with one element per commitment scheme.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Copy)]
#[cfg_attr(feature = "substrate", derive(Decode, Encode, MaxEncodedLen, TypeInfo))]
pub struct PerCommitmentScheme<T: GenericOverCommitment> {
    /// Element with [`CommitmentScheme::Ipa`].
    pub hyper_kzg: T::WithCommitment<HyperKZGCommitment>,
    /// Element with [`CommitmentScheme::DynamicDory`].
    pub dynamic_dory: T::WithCommitment<DynamicDoryCommitment>,
}

impl<T: GenericOverCommitment> PerCommitmentScheme<T> {
    /// Maps a `PerCommitmentScheme<T>` to a `PerCommitmentScheme<M::Out>` by applying the mapper.
    pub fn map<M>(self, mapper: M) -> PerCommitmentScheme<M::Out>
    where
        M: GenericOverCommitmentFn<In = T>,
    {
        PerCommitmentScheme {
            hyper_kzg: mapper.call(self.hyper_kzg),
            dynamic_dory: mapper.call(self.dynamic_dory),
        }
    }

    /// Returns this collection including only the elements selected by `flags`.
    pub fn select(self, flags: &CommitmentSchemeFlags) -> PerCommitmentScheme<OptionType<T>> {
        PerCommitmentScheme {
            hyper_kzg: flags.hyper_kzg.then_some(self.hyper_kzg),
            dynamic_dory: flags.dynamic_dory.then_some(self.dynamic_dory),
        }
    }

    /// Zips `self` with another `PerCommitmentScheme`.
    pub fn zip<U: GenericOverCommitment>(
        self,
        other: PerCommitmentScheme<U>,
    ) -> PerCommitmentScheme<PairType<T, U>> {
        PerCommitmentScheme {
            hyper_kzg: (self.hyper_kzg, other.hyper_kzg),
            dynamic_dory: (self.dynamic_dory, other.dynamic_dory),
        }
    }
}

impl<T: GenericOverCommitment, U: GenericOverCommitment> PerCommitmentScheme<PairType<T, U>> {
    /// Unzips a `PerCommitmentScheme` containing a pair into a pair of `PerCommitmentScheme`s.
    pub fn unzip(self) -> (PerCommitmentScheme<T>, PerCommitmentScheme<U>) {
        (
            PerCommitmentScheme {
                hyper_kzg: self.hyper_kzg.0,
                dynamic_dory: self.dynamic_dory.0,
            },
            PerCommitmentScheme {
                hyper_kzg: self.hyper_kzg.1,
                dynamic_dory: self.dynamic_dory.1,
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
    fn from(
        PerCommitmentScheme {
            hyper_kzg,
            dynamic_dory,
        }: &PerCommitmentScheme<OptionType<T>>,
    ) -> Self {
        CommitmentSchemeFlags {
            hyper_kzg: hyper_kzg.is_some(),
            dynamic_dory: dynamic_dory.is_some(),
        }
    }
}

impl<T: GenericOverCommitment> Default for PerCommitmentScheme<OptionType<T>> {
    fn default() -> Self {
        PerCommitmentScheme {
            hyper_kzg: None,
            dynamic_dory: None,
        }
    }
}

impl<T: GenericOverCommitment> IntoIterator for PerCommitmentScheme<T> {
    type Item = AnyCommitmentScheme<T>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let PerCommitmentScheme {
            hyper_kzg,
            dynamic_dory,
        } = self;

        alloc::vec![
            AnyCommitmentScheme::HyperKzg(hyper_kzg),
            AnyCommitmentScheme::DynamicDory(dynamic_dory),
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
                AnyCommitmentScheme::HyperKzg(data) => PerCommitmentScheme {
                    hyper_kzg: Some(data),
                    ..acc
                },
                AnyCommitmentScheme::DynamicDory(data) => PerCommitmentScheme {
                    dynamic_dory: Some(data),
                    ..acc
                },
            })
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use proof_of_sql::base::scalar::Scalar;
    use proof_of_sql::proof_primitive::dory::DoryScalar;
    use proof_of_sql::proof_primitive::hyperkzg::BNScalar;

    use super::*;
    use crate::generic_over_commitment::{AssociatedScalarType, CommitmentType};
    use crate::generic_over_commitment_fn::tests::SomeFn;

    #[test]
    fn we_can_iterate_over_commitment_schemes_in_commitment_scheme_flags() {
        let no_flags = CommitmentSchemeFlags {
            hyper_kzg: false,
            dynamic_dory: false,
        };
        assert_eq!(Vec::from_iter(no_flags), vec![]);

        let hyper_kzg_flags = CommitmentSchemeFlags {
            hyper_kzg: true,
            dynamic_dory: false,
        };
        assert_eq!(
            Vec::from_iter(hyper_kzg_flags),
            vec![CommitmentScheme::HyperKzg]
        );

        let dory_flags = CommitmentSchemeFlags {
            hyper_kzg: false,
            dynamic_dory: true,
        };
        assert_eq!(
            Vec::from_iter(dory_flags),
            vec![CommitmentScheme::DynamicDory]
        );

        let all_flags = CommitmentSchemeFlags::all();
        assert_eq!(
            Vec::from_iter(all_flags),
            vec![CommitmentScheme::HyperKzg, CommitmentScheme::DynamicDory]
        );
    }

    #[test]
    fn we_can_collect_commitment_schemes_into_commitment_scheme_flags() {
        let no_flags = CommitmentSchemeFlags::from_iter(None);
        assert_eq!(no_flags, CommitmentSchemeFlags::default());

        let hyper_kzg_flags = CommitmentSchemeFlags::from_iter([CommitmentScheme::HyperKzg]);
        assert_eq!(
            hyper_kzg_flags,
            CommitmentSchemeFlags {
                hyper_kzg: true,
                dynamic_dory: false
            }
        );

        let dory_flags = CommitmentSchemeFlags::from_iter([CommitmentScheme::DynamicDory]);
        assert_eq!(
            dory_flags,
            CommitmentSchemeFlags {
                hyper_kzg: false,
                dynamic_dory: true
            }
        );

        let all_flags = CommitmentSchemeFlags::from_iter([
            CommitmentScheme::HyperKzg,
            CommitmentScheme::DynamicDory,
        ]);
        assert_eq!(all_flags, CommitmentSchemeFlags::all());
    }

    #[test]
    fn we_can_iterate_over_commitments_in_per_commitment_scheme() {
        let all_commitments = PerCommitmentScheme::<CommitmentType> {
            hyper_kzg: Default::default(),
            dynamic_dory: Default::default(),
        };
        assert_eq!(
            Vec::from_iter(all_commitments),
            vec![
                AnyCommitmentScheme::<CommitmentType>::HyperKzg(Default::default()),
                AnyCommitmentScheme::<CommitmentType>::DynamicDory(Default::default())
            ]
        );
    }

    #[test]
    fn we_can_convert_any_commitment_scheme_to_scheme() {
        let hyper_kzg_commitment =
            AnyCommitmentScheme::<CommitmentType>::HyperKzg(Default::default());
        assert_eq!(hyper_kzg_commitment.to_scheme(), CommitmentScheme::HyperKzg);

        let dory_commitment =
            AnyCommitmentScheme::<CommitmentType>::DynamicDory(Default::default());
        assert_eq!(dory_commitment.to_scheme(), CommitmentScheme::DynamicDory);
    }

    #[test]
    fn we_can_convert_per_commitment_scheme_to_flags() {
        let no_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: None,
            dynamic_dory: None,
        };
        assert_eq!(no_commitments.to_flags(), CommitmentSchemeFlags::default());

        let hyper_kzg_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: Some(Default::default()),
            dynamic_dory: None,
        };
        assert_eq!(
            hyper_kzg_commitments.to_flags(),
            CommitmentSchemeFlags {
                hyper_kzg: true,
                dynamic_dory: false
            }
        );

        let dory_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: None,
            dynamic_dory: Some(Default::default()),
        };
        assert_eq!(
            dory_commitments.to_flags(),
            CommitmentSchemeFlags {
                hyper_kzg: false,
                dynamic_dory: true
            }
        );

        let all_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: Some(Default::default()),
            dynamic_dory: Some(Default::default()),
        };
        assert_eq!(all_commitments.to_flags(), CommitmentSchemeFlags::all());
    }

    #[test]
    fn we_can_transpose_any_commitment_scheme_with_option_type() {
        let hyper_kzg_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::HyperKzg(Some(Default::default()));
        assert_eq!(
            hyper_kzg_commitment.transpose_option(),
            Some(AnyCommitmentScheme::HyperKzg(Default::default()))
        );

        let dory_commitment = AnyCommitmentScheme::<OptionType<CommitmentType>>::DynamicDory(Some(
            Default::default(),
        ));
        assert_eq!(
            dory_commitment.transpose_option(),
            Some(AnyCommitmentScheme::DynamicDory(Default::default()))
        );

        let hyper_kzg_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::HyperKzg(None);
        assert_eq!(hyper_kzg_commitment.transpose_option(), None);

        let dory_commitment = AnyCommitmentScheme::<OptionType<CommitmentType>>::DynamicDory(None);
        assert_eq!(dory_commitment.transpose_option(), None);
    }

    #[test]
    fn we_can_transpose_any_commitment_scheme_with_result_type() {
        let hyper_kzg_commitment =
            AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::HyperKzg(Ok(
                Default::default(),
            ));
        assert_eq!(
            hyper_kzg_commitment.transpose_result(),
            Ok(AnyCommitmentScheme::HyperKzg(Default::default()))
        );

        let dory_commitment =
            AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::DynamicDory(Ok(
                Default::default(),
            ));
        assert_eq!(
            dory_commitment.transpose_result(),
            Ok(AnyCommitmentScheme::DynamicDory(Default::default()))
        );

        let hyper_kzg_commitment =
            AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::HyperKzg(Err(1));
        assert_eq!(hyper_kzg_commitment.transpose_result(), Err(1));

        let dory_commitment =
            AnyCommitmentScheme::<ResultOkType<CommitmentType, usize>>::DynamicDory(Err(2));
        assert_eq!(dory_commitment.transpose_result(), Err(2));
    }

    #[test]
    fn we_can_collect_per_commitment_scheme_with_option_type_from_iter_and_into_flat_iter() {
        let no_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: None,
            dynamic_dory: None,
        };
        let no_iterator = vec![];
        assert_eq!(
            no_commitments.into_flat_iter().collect::<Vec<_>>(),
            no_iterator.clone()
        );
        assert_eq!(PerCommitmentScheme::from_iter(no_iterator), no_commitments);

        let hyper_kzg_commitment = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: Some(Default::default()),
            dynamic_dory: None,
        };
        let hyper_kzg_iterator = vec![AnyCommitmentScheme::<CommitmentType>::HyperKzg(
            Default::default(),
        )];
        assert_eq!(
            hyper_kzg_commitment.into_flat_iter().collect::<Vec<_>>(),
            hyper_kzg_iterator.clone(),
        );
        assert_eq!(
            PerCommitmentScheme::from_iter(hyper_kzg_iterator),
            hyper_kzg_commitment
        );

        let dory_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: None,
            dynamic_dory: Some(Default::default()),
        };
        let dory_iterator = vec![AnyCommitmentScheme::<CommitmentType>::DynamicDory(
            Default::default(),
        )];
        assert_eq!(
            dory_commitments.into_flat_iter().collect::<Vec<_>>(),
            dory_iterator.clone(),
        );
        assert_eq!(
            PerCommitmentScheme::from_iter(dory_iterator),
            dory_commitments
        );

        let all_commitments = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: Some(Default::default()),
            dynamic_dory: Some(Default::default()),
        };
        let all_iterator = vec![
            AnyCommitmentScheme::<CommitmentType>::HyperKzg(Default::default()),
            AnyCommitmentScheme::<CommitmentType>::DynamicDory(Default::default()),
        ];
        assert_eq!(
            all_commitments.into_flat_iter().collect::<Vec<_>>(),
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

        let hyper_kzg_commitment =
            AnyCommitmentScheme::<CommitmentType>::HyperKzg(Default::default());
        let some_hyper_kzg_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::HyperKzg(Some(Default::default()));
        assert_eq!(
            hyper_kzg_commitment.map(&some_fn),
            some_hyper_kzg_commitment
        );

        let dory_commitment = AnyCommitmentScheme::<CommitmentType>::HyperKzg(Default::default());
        let some_dory_commitment =
            AnyCommitmentScheme::<OptionType<CommitmentType>>::HyperKzg(Some(Default::default()));
        assert_eq!(dory_commitment.map(some_fn), some_dory_commitment);
    }

    #[test]
    fn we_can_map_per_commitment_scheme_to_another() {
        let some_fn = SomeFn::<CommitmentType>::new();

        let per_commitment_scheme = PerCommitmentScheme::<CommitmentType> {
            hyper_kzg: Default::default(),
            dynamic_dory: Default::default(),
        };
        let some_per_commitment_scheme = PerCommitmentScheme::<OptionType<CommitmentType>> {
            hyper_kzg: Some(Default::default()),
            dynamic_dory: Some(Default::default()),
        };

        assert_eq!(
            per_commitment_scheme.map(some_fn),
            some_per_commitment_scheme
        );
    }

    #[test]
    fn we_can_select_per_commitment_scheme_by_flags() {
        let per_commitment_scheme = PerCommitmentScheme::<CommitmentType> {
            hyper_kzg: Default::default(),
            dynamic_dory: Default::default(),
        };

        let no_flags = CommitmentSchemeFlags::default();
        assert_eq!(
            per_commitment_scheme.select(&no_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>>::default()
        );

        let hyper_kzg_flags = CommitmentSchemeFlags {
            hyper_kzg: true,
            ..Default::default()
        };
        assert_eq!(
            per_commitment_scheme.select(&hyper_kzg_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>> {
                hyper_kzg: Some(Default::default()),
                dynamic_dory: None,
            }
        );

        let dory_flags = CommitmentSchemeFlags {
            dynamic_dory: true,
            ..Default::default()
        };
        assert_eq!(
            per_commitment_scheme.select(&dory_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>> {
                hyper_kzg: None,
                dynamic_dory: Some(Default::default()),
            }
        );

        let all_flags = CommitmentSchemeFlags::all();
        assert_eq!(
            per_commitment_scheme.select(&all_flags),
            PerCommitmentScheme::<OptionType<CommitmentType>> {
                hyper_kzg: Some(Default::default()),
                dynamic_dory: Some(Default::default()),
            }
        );
    }

    #[test]
    fn we_can_zip_and_unzip_per_commitment_scheme() {
        let commitments = PerCommitmentScheme::<CommitmentType> {
            hyper_kzg: Default::default(),
            dynamic_dory: Default::default(),
        };

        let scalars = PerCommitmentScheme::<AssociatedScalarType> {
            hyper_kzg: BNScalar::ZERO,
            dynamic_dory: DoryScalar::ONE,
        };

        let commitments_with_scalars =
            PerCommitmentScheme::<PairType<CommitmentType, AssociatedScalarType>> {
                hyper_kzg: (Default::default(), BNScalar::ZERO),
                dynamic_dory: (Default::default(), DoryScalar::ONE),
            };

        assert_eq!(commitments.zip(scalars), commitments_with_scalars);
        assert_eq!(commitments_with_scalars.unzip(), (commitments, scalars));
    }

    #[test]
    fn we_can_unzip_any_commitment_scheme() {
        let hyper_kzg_commitment_with_scalar =
            AnyCommitmentScheme::<PairType<CommitmentType, AssociatedScalarType>>::HyperKzg((
                Default::default(),
                BNScalar::ONE,
            ));
        assert_eq!(
            hyper_kzg_commitment_with_scalar.unzip(),
            (
                AnyCommitmentScheme::<CommitmentType>::HyperKzg(Default::default()),
                AnyCommitmentScheme::<AssociatedScalarType>::HyperKzg(BNScalar::ONE)
            )
        );

        let dory_commitment_with_scalar =
            AnyCommitmentScheme::<PairType<CommitmentType, AssociatedScalarType>>::DynamicDory((
                Default::default(),
                DoryScalar::TWO,
            ));
        assert_eq!(
            dory_commitment_with_scalar.unzip(),
            (
                AnyCommitmentScheme::<CommitmentType>::DynamicDory(Default::default()),
                AnyCommitmentScheme::<AssociatedScalarType>::DynamicDory(DoryScalar::TWO)
            )
        );
    }

    #[test]
    fn we_can_unwrap_any_commitment_scheme_with_concrete_type() {
        let hyper_kzg_usize = AnyCommitmentScheme::<ConcreteType<usize>>::HyperKzg(123);
        assert_eq!(hyper_kzg_usize.unwrap(), 123);

        let dory_usize = AnyCommitmentScheme::<ConcreteType<usize>>::DynamicDory(456);
        assert_eq!(dory_usize.unwrap(), 456);
    }

    #[test]
    fn we_can_convert_commitment_scheme_into_any_commitment_scheme_with_value() {
        assert_eq!(
            CommitmentScheme::HyperKzg.into_any_concrete(123),
            AnyCommitmentScheme::HyperKzg(123)
        );
        assert_eq!(
            CommitmentScheme::DynamicDory.into_any_concrete(456),
            AnyCommitmentScheme::DynamicDory(456)
        );
    }
}
