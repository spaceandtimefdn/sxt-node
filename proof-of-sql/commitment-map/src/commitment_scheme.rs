use crate::generic_over_commitment::{GenericOverCommitment, OptionType};
use curve25519_dalek::RistrettoPoint;
#[cfg(feature = "substrate")]
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen};
use proof_of_sql::proof_primitive::dory::DoryCommitment;
#[cfg(feature = "substrate")]
use scale_info::TypeInfo;

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
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
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
pub struct PerCommitmentScheme<T: GenericOverCommitment> {
    /// Element with [`CommitmentScheme::Ipa`].
    pub ipa: T::WithCommitment<RistrettoPoint>,
    /// Element with [`CommitmentScheme::Dory`].
    pub dory: T::WithCommitment<DoryCommitment>,
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
    use super::*;
    use crate::generic_over_commitment::CommitmentType;
    use alloc::{vec, vec::Vec};

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
}
