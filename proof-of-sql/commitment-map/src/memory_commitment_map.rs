use crate::{
    commitment_map_implementor::CommitmentMapImplementor,
    commitment_scheme::{AnyCommitmentScheme, CommitmentScheme},
    generic_over_commitment::{GenericOverCommitment, OptionType},
};
use curve25519_dalek::RistrettoPoint;
use proof_of_sql::{base::database::TableRef, proof_primitive::dory::DoryCommitment};
use std::collections::HashMap;

/// Accurate implementor of [`CommitmentMap`] that stores commitments in-memory.
///
/// Intended for testing.
///
/// [`CommitmentMap`]: crate::CommitmentMap
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryCommitmentMap<V: GenericOverCommitment> {
    ipa_map: HashMap<TableRef, V::WithCommitment<RistrettoPoint>>,
    dory_map: HashMap<TableRef, V::WithCommitment<DoryCommitment>>,
}

impl<V: GenericOverCommitment> CommitmentMapImplementor<TableRef, V> for MemoryCommitmentMap<V>
where
    V::WithCommitment<DoryCommitment>: Clone,
    V::WithCommitment<RistrettoPoint>: Clone,
{
    fn has_key_and_scheme_impl(&self, key: &TableRef, scheme: &CommitmentScheme) -> bool {
        match scheme {
            CommitmentScheme::Ipa => self.ipa_map.contains_key(key),
            CommitmentScheme::Dory => self.dory_map.contains_key(key),
        }
    }

    fn get_commitment_for_any_scheme_impl(
        &self,
        key: &TableRef,
        scheme: &CommitmentScheme,
    ) -> AnyCommitmentScheme<OptionType<V>> {
        match scheme {
            CommitmentScheme::Ipa => AnyCommitmentScheme::Ipa(self.ipa_map.get(key).cloned()),
            CommitmentScheme::Dory => AnyCommitmentScheme::Dory(self.dory_map.get(key).cloned()),
        }
    }

    fn set_commitment_for_any_scheme_impl(
        &mut self,
        key: TableRef,
        commitment: AnyCommitmentScheme<V>,
    ) {
        match commitment {
            AnyCommitmentScheme::Ipa(commitment) => {
                self.ipa_map.insert(key, commitment);
            }
            AnyCommitmentScheme::Dory(commitment) => {
                self.dory_map.insert(key, commitment);
            }
        }
    }

    fn delete_commitment_for_any_scheme_impl(&mut self, key: &TableRef, scheme: &CommitmentScheme) {
        match scheme {
            CommitmentScheme::Ipa => {
                self.ipa_map.remove(key);
            }
            CommitmentScheme::Dory => {
                self.dory_map.remove(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CommitmentMap, CommitmentSchemeFlags, CommitmentSchemesMismatchError, KeyExistsError,
        PerCommitmentScheme,
    };
    use core::marker::PhantomData;
    use proof_of_sql::base::commitment::Commitment;

    /// An example of a GenericOverCommitment value for testing.
    ///
    /// We can store actual commitments in the MemoryCommitmentMap.
    /// However, generating them for testing requires the blitzar feature.
    /// Enabling the blitzar feature complicates writing substrate-oriented tests.
    #[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
    struct TestCommitmentMetadata<C: Commitment> {
        metadata: usize,
        phantom_data: PhantomData<C>,
    }

    impl<C: Commitment> TestCommitmentMetadata<C> {
        /// Construct a new [`TestCommitmentMetadata`].
        fn new(metadata: usize) -> Self {
            TestCommitmentMetadata {
                metadata,
                phantom_data: PhantomData,
            }
        }
    }

    #[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
    struct TestCommitmentMetadataType;

    impl GenericOverCommitment for TestCommitmentMetadataType {
        type WithCommitment<C: Commitment> = TestCommitmentMetadata<C>;
    }

    struct CombinationsCommitmentMapRefs {
        ipa_ref: TableRef,
        dory_ref: TableRef,
        all_ref: TableRef,
    }

    fn all_combinations_commitment_map() -> (
        MemoryCommitmentMap<TestCommitmentMetadataType>,
        CombinationsCommitmentMapRefs,
    ) {
        let ipa_ref: TableRef = "table.ipa_only".parse().unwrap();
        let ipa_ref_ipa_commitment = TestCommitmentMetadata::<RistrettoPoint>::new(1);

        let dory_ref: TableRef = "table.dory_only".parse().unwrap();
        let dory_ref_dory_commitment = TestCommitmentMetadata::<DoryCommitment>::new(2);

        let all_ref: TableRef = "table.all_schemes".parse().unwrap();
        let all_ref_ipa_commitment = TestCommitmentMetadata::<RistrettoPoint>::new(3);
        let all_ref_dory_commitment = TestCommitmentMetadata::<DoryCommitment>::new(3);

        let commitment_map = MemoryCommitmentMap {
            ipa_map: HashMap::from_iter([
                (ipa_ref, ipa_ref_ipa_commitment),
                (all_ref, all_ref_ipa_commitment),
            ]),
            dory_map: HashMap::from_iter([
                (dory_ref, dory_ref_dory_commitment),
                (all_ref, all_ref_dory_commitment),
            ]),
        };

        (
            commitment_map,
            CombinationsCommitmentMapRefs {
                ipa_ref,
                dory_ref,
                all_ref,
            },
        )
    }

    #[test]
    fn we_can_check_existence_of_tables_and_schema() {
        let (commitment_map, refs) = all_combinations_commitment_map();

        assert!(commitment_map.has_key_and_scheme(&refs.ipa_ref, &CommitmentScheme::Ipa));
        assert!(!commitment_map.has_key_and_scheme(&refs.ipa_ref, &CommitmentScheme::Dory));
        assert!(!commitment_map.has_key_and_scheme(&refs.dory_ref, &CommitmentScheme::Ipa));
        assert!(commitment_map.has_key_and_scheme(&refs.dory_ref, &CommitmentScheme::Dory));

        assert_eq!(
            commitment_map.schemes_for_key(&refs.ipa_ref),
            CommitmentSchemeFlags {
                ipa: true,
                dory: false
            }
        );
        assert_eq!(
            commitment_map.schemes_for_key(&refs.dory_ref),
            CommitmentSchemeFlags {
                ipa: false,
                dory: true,
            }
        );
        assert_eq!(
            commitment_map.schemes_for_key(&refs.all_ref),
            CommitmentSchemeFlags::all()
        );
        assert_eq!(
            commitment_map.schemes_for_key(&"does_not.exist".parse().unwrap()),
            CommitmentSchemeFlags::default()
        );

        assert!(commitment_map.has_key(&refs.ipa_ref));
        assert!(commitment_map.has_key(&refs.dory_ref));
        assert!(commitment_map.has_key(&refs.all_ref));
        assert!(!commitment_map.has_key(&"does_not.exist".parse().unwrap()));
    }

    #[test]
    fn we_can_get_table_commitments() {
        let (commitment_map, refs) = all_combinations_commitment_map();

        let none_commitments = PerCommitmentScheme::default();
        assert_eq!(
            commitment_map.get_commitments(&"does_not.exist".parse().unwrap()),
            none_commitments
        );

        let ipa_commitments = PerCommitmentScheme {
            ipa: Some(TestCommitmentMetadata::<RistrettoPoint>::new(1)),
            dory: None,
        };
        assert_eq!(
            commitment_map.get_commitments(&refs.ipa_ref),
            ipa_commitments
        );

        let dory_commitments = PerCommitmentScheme {
            ipa: None,
            dory: Some(TestCommitmentMetadata::<DoryCommitment>::new(2)),
        };
        assert_eq!(
            commitment_map.get_commitments(&refs.dory_ref),
            dory_commitments
        );

        let all_commitments = PerCommitmentScheme {
            ipa: Some(TestCommitmentMetadata::<RistrettoPoint>::new(3)),
            dory: Some(TestCommitmentMetadata::<DoryCommitment>::new(3)),
        };
        assert_eq!(
            commitment_map.get_commitments(&refs.all_ref),
            all_commitments
        );
    }

    #[test]
    fn we_can_create_tables() {
        let ipa_ref: TableRef = "table.ipa_only".parse().unwrap();
        let dory_ref: TableRef = "table.dory_only".parse().unwrap();
        let all_ref: TableRef = "table.all_schemes".parse().unwrap();

        let ipa_commitment = TestCommitmentMetadata::<RistrettoPoint>::new(1);
        let dory_commitment = TestCommitmentMetadata::<DoryCommitment>::new(2);

        let mut commitment_map = MemoryCommitmentMap::<TestCommitmentMetadataType>::default();

        commitment_map
            .create_commitments(
                ipa_ref,
                PerCommitmentScheme {
                    ipa: Some(ipa_commitment),
                    dory: None,
                },
            )
            .unwrap();
        commitment_map
            .create_commitments(
                dory_ref,
                PerCommitmentScheme {
                    ipa: None,
                    dory: Some(dory_commitment),
                },
            )
            .unwrap();
        commitment_map
            .create_commitments(
                all_ref,
                PerCommitmentScheme {
                    ipa: Some(ipa_commitment),
                    dory: Some(dory_commitment),
                },
            )
            .unwrap();

        assert_eq!(
            commitment_map.ipa_map,
            HashMap::from_iter([(ipa_ref, ipa_commitment), (all_ref, ipa_commitment)])
        );
        assert_eq!(
            commitment_map.dory_map,
            HashMap::from_iter([(dory_ref, dory_commitment), (all_ref, dory_commitment)])
        );
    }

    #[test]
    fn we_cannot_create_tables_that_already_exist() {
        let (mut commitment_map, refs) = all_combinations_commitment_map();
        let original_commitment_map = commitment_map.clone();

        let ipa_commitment = TestCommitmentMetadata::<RistrettoPoint>::new(10);
        let dory_commitment = TestCommitmentMetadata::<DoryCommitment>::new(20);

        assert!(matches!(
            commitment_map.create_commitments(refs.ipa_ref, PerCommitmentScheme::default()),
            Err(KeyExistsError { .. })
        ));
        assert!(matches!(
            commitment_map.create_commitments(
                refs.ipa_ref,
                PerCommitmentScheme {
                    ipa: Some(ipa_commitment),
                    dory: None
                }
            ),
            Err(KeyExistsError { .. })
        ));
        assert!(matches!(
            commitment_map.create_commitments(
                refs.ipa_ref,
                PerCommitmentScheme {
                    ipa: None,
                    dory: Some(dory_commitment),
                }
            ),
            Err(KeyExistsError { .. })
        ));
        assert!(matches!(
            commitment_map.create_commitments(
                refs.ipa_ref,
                PerCommitmentScheme {
                    ipa: Some(ipa_commitment),
                    dory: Some(dory_commitment),
                }
            ),
            Err(KeyExistsError { .. })
        ));

        // commitment_map was not mutated during failures
        assert_eq!(commitment_map, original_commitment_map);
    }

    #[test]
    fn we_can_delete_tables() {
        let (mut commitment_map, refs) = all_combinations_commitment_map();

        assert!(commitment_map.has_key(&refs.ipa_ref));
        commitment_map.delete_commitments(&refs.ipa_ref);
        assert!(!commitment_map.has_key(&refs.ipa_ref));

        assert!(commitment_map.has_key(&refs.all_ref));
        commitment_map.delete_commitments(&refs.all_ref);
        assert!(!commitment_map.has_key(&refs.all_ref));

        assert!(commitment_map.has_key(&refs.dory_ref));
        commitment_map.delete_commitments(&refs.dory_ref);
        assert!(!commitment_map.has_key(&refs.dory_ref));

        assert_eq!(commitment_map, MemoryCommitmentMap::default());
    }

    #[test]
    fn we_can_update_tables() {
        let (mut commitment_map, refs) = all_combinations_commitment_map();

        let new_ipa_commitment = TestCommitmentMetadata::<RistrettoPoint>::new(10);
        let new_dory_commitment = TestCommitmentMetadata::<DoryCommitment>::new(20);

        assert_ne!(
            commitment_map.ipa_map.get(&refs.ipa_ref).unwrap(),
            &new_ipa_commitment
        );
        commitment_map
            .update_commitments(
                refs.ipa_ref,
                PerCommitmentScheme {
                    ipa: Some(new_ipa_commitment),
                    dory: None,
                },
            )
            .unwrap();
        assert_eq!(
            commitment_map.ipa_map.get(&refs.ipa_ref).unwrap(),
            &new_ipa_commitment
        );

        assert_ne!(
            commitment_map.dory_map.get(&refs.dory_ref).unwrap(),
            &new_dory_commitment
        );
        commitment_map
            .update_commitments(
                refs.dory_ref,
                PerCommitmentScheme {
                    ipa: None,
                    dory: Some(new_dory_commitment),
                },
            )
            .unwrap();
        assert_eq!(
            commitment_map.dory_map.get(&refs.dory_ref).unwrap(),
            &new_dory_commitment
        );

        assert_ne!(
            commitment_map.ipa_map.get(&refs.all_ref).unwrap(),
            &new_ipa_commitment
        );
        assert_ne!(
            commitment_map.dory_map.get(&refs.all_ref).unwrap(),
            &new_dory_commitment
        );
        commitment_map
            .update_commitments(
                refs.all_ref,
                PerCommitmentScheme {
                    ipa: Some(new_ipa_commitment),
                    dory: Some(new_dory_commitment),
                },
            )
            .unwrap();
        assert_eq!(
            commitment_map.ipa_map.get(&refs.all_ref).unwrap(),
            &new_ipa_commitment
        );
        assert_eq!(
            commitment_map.dory_map.get(&refs.all_ref).unwrap(),
            &new_dory_commitment
        );
    }

    #[test]
    fn we_cannot_update_tables_with_mismatched_commitment_schemes() {
        let (mut commitment_map, refs) = all_combinations_commitment_map();
        let original_commitment_map = commitment_map.clone();

        let new_ipa_commitment = TestCommitmentMetadata::<RistrettoPoint>::new(10);
        let new_dory_commitment = TestCommitmentMetadata::<DoryCommitment>::new(20);

        let no_commitments = PerCommitmentScheme::default();
        assert!(matches!(
            commitment_map.update_commitments(refs.ipa_ref, no_commitments),
            Err(CommitmentSchemesMismatchError { .. })
        ));

        let dory_commitments = PerCommitmentScheme {
            ipa: None,
            dory: Some(new_dory_commitment),
        };
        assert!(matches!(
            commitment_map.update_commitments(refs.ipa_ref, dory_commitments),
            Err(CommitmentSchemesMismatchError { .. })
        ));

        let all_commitments = PerCommitmentScheme {
            ipa: Some(new_ipa_commitment),
            dory: Some(new_dory_commitment),
        };
        assert!(matches!(
            commitment_map.update_commitments(refs.ipa_ref, all_commitments),
            Err(CommitmentSchemesMismatchError { .. })
        ));

        // commitment_map was not mutated during failures
        assert_eq!(commitment_map, original_commitment_map);
    }
}
