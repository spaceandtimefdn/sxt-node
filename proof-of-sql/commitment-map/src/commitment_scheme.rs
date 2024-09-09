use crate::generic_over_commitment::GenericOverCommitment;
use curve25519_dalek::RistrettoPoint;
use proof_of_sql::proof_primitive::dory::DoryCommitment;

/// Identifier for proof-of-sql commitment schemes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
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

impl<T: GenericOverCommitment> From<&AnyCommitmentScheme<T>> for CommitmentScheme {
    fn from(commitment: &AnyCommitmentScheme<T>) -> Self {
        match commitment {
            AnyCommitmentScheme::Ipa(_) => CommitmentScheme::Ipa,
            AnyCommitmentScheme::Dory(_) => CommitmentScheme::Dory,
        }
    }
}

/// Collection of commitment-associated data, with at most one element per commitment scheme.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PerCommitmentScheme<T: GenericOverCommitment> {
    /// Element with [`CommitmentScheme::Ipa`], if it exists.
    pub ipa: Option<T::WithCommitment<RistrettoPoint>>,
    /// Element with [`CommitmentScheme::Dory`], if it exists.
    pub dory: Option<T::WithCommitment<DoryCommitment>>,
}

impl<T: GenericOverCommitment> PerCommitmentScheme<T> {
    /// Returns the schemes present in this collection as a [`CommitmentSchemeFlags`].
    pub fn to_flags(&self) -> CommitmentSchemeFlags {
        self.into()
    }
}

impl<T: GenericOverCommitment> From<&PerCommitmentScheme<T>> for CommitmentSchemeFlags {
    fn from(PerCommitmentScheme { ipa, dory }: &PerCommitmentScheme<T>) -> Self {
        CommitmentSchemeFlags {
            ipa: ipa.is_some(),
            dory: dory.is_some(),
        }
    }
}

impl<T: GenericOverCommitment> IntoIterator for PerCommitmentScheme<T> {
    type Item = AnyCommitmentScheme<T>;
    type IntoIter =
        core::iter::Chain<core::option::IntoIter<Self::Item>, core::option::IntoIter<Self::Item>>;

    fn into_iter(self) -> Self::IntoIter {
        let PerCommitmentScheme { ipa, dory } = self;

        itertools::chain!(
            ipa.map(AnyCommitmentScheme::Ipa),
            dory.map(AnyCommitmentScheme::Dory)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_over_commitment::CommitmentType;

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
        let no_commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: None,
            dory: None,
        };
        assert_eq!(Vec::from_iter(no_commitments), vec![]);

        let ipa_commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: Some(Default::default()),
            dory: None,
        };
        assert_eq!(
            Vec::from_iter(ipa_commitments),
            vec![AnyCommitmentScheme::Ipa(Default::default())]
        );

        let dory_commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: None,
            dory: Some(Default::default()),
        };
        assert_eq!(
            Vec::from_iter(dory_commitments),
            vec![AnyCommitmentScheme::Dory(Default::default())]
        );

        let all_commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: Some(Default::default()),
            dory: Some(Default::default()),
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
        let no_commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: None,
            dory: None,
        };
        assert_eq!(no_commitments.to_flags(), CommitmentSchemeFlags::default());

        let ipa_commitments = PerCommitmentScheme::<CommitmentType> {
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

        let dory_commitments = PerCommitmentScheme::<CommitmentType> {
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

        let all_commitments = PerCommitmentScheme::<CommitmentType> {
            ipa: Some(Default::default()),
            dory: Some(Default::default()),
        };
        assert_eq!(all_commitments.to_flags(), CommitmentSchemeFlags::all());
    }
}
