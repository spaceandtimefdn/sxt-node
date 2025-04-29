use std::collections::HashMap;

use proof_of_sql::base::database::TableRef;
use proof_of_sql::proof_primitive::dory::DynamicDoryCommitment;
use proof_of_sql::proof_primitive::hyperkzg::HyperKZGCommitment;

use crate::commitment_map_implementor::CommitmentMapImplementor;
use crate::commitment_scheme::{AnyCommitmentScheme, CommitmentScheme};
use crate::generic_over_commitment::{GenericOverCommitment, OptionType};

/// Accurate implementor of [`CommitmentMap`] that stores commitments in-memory.
///
/// Intended for testing.
///
/// [`CommitmentMap`]: crate::CommitmentMap
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryCommitmentMap<V: GenericOverCommitment> {
    hyper_kzg_map: HashMap<TableRef, V::WithCommitment<HyperKZGCommitment>>,
    dory_map: HashMap<TableRef, V::WithCommitment<DynamicDoryCommitment>>,
}

impl<V: GenericOverCommitment> CommitmentMapImplementor<TableRef, V> for MemoryCommitmentMap<V>
where
    V::WithCommitment<DynamicDoryCommitment>: Clone,
    V::WithCommitment<HyperKZGCommitment>: Clone,
{
    fn has_key_and_scheme_impl(&self, key: &TableRef, scheme: &CommitmentScheme) -> bool {
        match scheme {
            CommitmentScheme::HyperKzg => self.hyper_kzg_map.contains_key(key),
            CommitmentScheme::DynamicDory => self.dory_map.contains_key(key),
        }
    }

    fn get_commitment_for_any_scheme_impl(
        &self,
        key: &TableRef,
        scheme: &CommitmentScheme,
    ) -> AnyCommitmentScheme<OptionType<V>> {
        match scheme {
            CommitmentScheme::HyperKzg => {
                AnyCommitmentScheme::HyperKzg(self.hyper_kzg_map.get(key).cloned())
            }
            CommitmentScheme::DynamicDory => {
                AnyCommitmentScheme::DynamicDory(self.dory_map.get(key).cloned())
            }
        }
    }

    fn set_commitment_for_any_scheme_impl(
        &mut self,
        key: TableRef,
        commitment: AnyCommitmentScheme<V>,
    ) {
        match commitment {
            AnyCommitmentScheme::HyperKzg(commitment) => {
                self.hyper_kzg_map.insert(key, commitment);
            }
            AnyCommitmentScheme::DynamicDory(commitment) => {
                self.dory_map.insert(key, commitment);
            }
        }
    }

    fn delete_commitment_for_any_scheme_impl(&mut self, key: &TableRef, scheme: &CommitmentScheme) {
        match scheme {
            CommitmentScheme::HyperKzg => {
                self.hyper_kzg_map.remove(key);
            }
            CommitmentScheme::DynamicDory => {
                self.dory_map.remove(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::marker::PhantomData;

    use proof_of_sql::base::commitment::Commitment;

    use super::*;
    use crate::{
        CommitmentMap,
        CommitmentSchemeFlags,
        CommitmentSchemesMismatchError,
        KeyExistsError,
        PerCommitmentScheme,
    };

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
        hyper_kzg_ref: TableRef,
        dory_ref: TableRef,
        all_ref: TableRef,
    }

    fn all_combinations_commitment_map() -> (
        MemoryCommitmentMap<TestCommitmentMetadataType>,
        CombinationsCommitmentMapRefs,
    ) {
        let hyper_kzg_ref: TableRef = "table.hyper_kzg_only".parse().unwrap();
        let hyper_kzg_ref_hyper_kzg_commitment =
            TestCommitmentMetadata::<HyperKZGCommitment>::new(1);

        let dory_ref: TableRef = "table.dory_only".parse().unwrap();
        let dory_ref_dory_commitment = TestCommitmentMetadata::<DynamicDoryCommitment>::new(2);

        let all_ref: TableRef = "table.all_schemes".parse().unwrap();
        let all_ref_hyper_kzg_commitment = TestCommitmentMetadata::<HyperKZGCommitment>::new(3);
        let all_ref_dory_commitment = TestCommitmentMetadata::<DynamicDoryCommitment>::new(3);

        let commitment_map = MemoryCommitmentMap {
            hyper_kzg_map: HashMap::from_iter([
                (hyper_kzg_ref.clone(), hyper_kzg_ref_hyper_kzg_commitment),
                (all_ref.clone(), all_ref_hyper_kzg_commitment),
            ]),
            dory_map: HashMap::from_iter([
                (dory_ref.clone(), dory_ref_dory_commitment),
                (all_ref.clone(), all_ref_dory_commitment),
            ]),
        };

        (
            commitment_map,
            CombinationsCommitmentMapRefs {
                hyper_kzg_ref,
                dory_ref,
                all_ref,
            },
        )
    }

    #[test]
    fn we_can_check_existence_of_tables_and_schema() {
        let (commitment_map, refs) = all_combinations_commitment_map();

        assert!(commitment_map.has_key_and_scheme(&refs.hyper_kzg_ref, &CommitmentScheme::HyperKzg));
        assert!(
            !commitment_map.has_key_and_scheme(&refs.hyper_kzg_ref, &CommitmentScheme::DynamicDory)
        );
        assert!(!commitment_map.has_key_and_scheme(&refs.dory_ref, &CommitmentScheme::HyperKzg));
        assert!(commitment_map.has_key_and_scheme(&refs.dory_ref, &CommitmentScheme::DynamicDory));

        assert_eq!(
            commitment_map.schemes_for_key(&refs.hyper_kzg_ref),
            CommitmentSchemeFlags {
                hyper_kzg: true,
                dynamic_dory: false
            }
        );
        assert_eq!(
            commitment_map.schemes_for_key(&refs.dory_ref),
            CommitmentSchemeFlags {
                hyper_kzg: false,
                dynamic_dory: true,
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

        assert!(commitment_map.has_key(&refs.hyper_kzg_ref));
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

        let hyper_kzg_commitments = PerCommitmentScheme {
            hyper_kzg: Some(TestCommitmentMetadata::<HyperKZGCommitment>::new(1)),
            dynamic_dory: None,
        };
        assert_eq!(
            commitment_map.get_commitments(&refs.hyper_kzg_ref),
            hyper_kzg_commitments
        );

        let dory_commitments = PerCommitmentScheme {
            hyper_kzg: None,
            dynamic_dory: Some(TestCommitmentMetadata::<DynamicDoryCommitment>::new(2)),
        };
        assert_eq!(
            commitment_map.get_commitments(&refs.dory_ref),
            dory_commitments
        );

        let all_commitments = PerCommitmentScheme {
            hyper_kzg: Some(TestCommitmentMetadata::<HyperKZGCommitment>::new(3)),
            dynamic_dory: Some(TestCommitmentMetadata::<DynamicDoryCommitment>::new(3)),
        };
        assert_eq!(
            commitment_map.get_commitments(&refs.all_ref),
            all_commitments
        );
    }

    #[test]
    fn we_can_create_tables() {
        let hyper_kzg_ref: TableRef = "table.hyper_kzg_only".parse().unwrap();
        let dory_ref: TableRef = "table.dory_only".parse().unwrap();
        let all_ref: TableRef = "table.all_schemes".parse().unwrap();

        let hyper_kzg_commitment = TestCommitmentMetadata::<HyperKZGCommitment>::new(1);
        let dory_commitment = TestCommitmentMetadata::<DynamicDoryCommitment>::new(2);

        let mut commitment_map = MemoryCommitmentMap::<TestCommitmentMetadataType>::default();

        commitment_map
            .create_commitments(
                hyper_kzg_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: Some(hyper_kzg_commitment),
                    dynamic_dory: None,
                },
            )
            .unwrap();
        commitment_map
            .create_commitments(
                dory_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: None,
                    dynamic_dory: Some(dory_commitment),
                },
            )
            .unwrap();
        commitment_map
            .create_commitments(
                all_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: Some(hyper_kzg_commitment),
                    dynamic_dory: Some(dory_commitment),
                },
            )
            .unwrap();

        assert_eq!(
            commitment_map.hyper_kzg_map,
            HashMap::from_iter([
                (hyper_kzg_ref, hyper_kzg_commitment),
                (all_ref.clone(), hyper_kzg_commitment)
            ])
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

        let hyper_kzg_commitment = TestCommitmentMetadata::<HyperKZGCommitment>::new(10);
        let dory_commitment = TestCommitmentMetadata::<DynamicDoryCommitment>::new(20);

        assert!(matches!(
            commitment_map
                .create_commitments(refs.hyper_kzg_ref.clone(), PerCommitmentScheme::default()),
            Err(KeyExistsError { .. })
        ));
        assert!(matches!(
            commitment_map.create_commitments(
                refs.hyper_kzg_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: Some(hyper_kzg_commitment),
                    dynamic_dory: None
                }
            ),
            Err(KeyExistsError { .. })
        ));
        assert!(matches!(
            commitment_map.create_commitments(
                refs.hyper_kzg_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: None,
                    dynamic_dory: Some(dory_commitment),
                }
            ),
            Err(KeyExistsError { .. })
        ));
        assert!(matches!(
            commitment_map.create_commitments(
                refs.hyper_kzg_ref,
                PerCommitmentScheme {
                    hyper_kzg: Some(hyper_kzg_commitment),
                    dynamic_dory: Some(dory_commitment),
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

        assert!(commitment_map.has_key(&refs.hyper_kzg_ref));
        commitment_map.delete_commitments(&refs.hyper_kzg_ref);
        assert!(!commitment_map.has_key(&refs.hyper_kzg_ref));

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

        let new_hyper_kzg_commitment = TestCommitmentMetadata::<HyperKZGCommitment>::new(10);
        let new_dory_commitment = TestCommitmentMetadata::<DynamicDoryCommitment>::new(20);

        assert_ne!(
            commitment_map
                .hyper_kzg_map
                .get(&refs.hyper_kzg_ref)
                .unwrap(),
            &new_hyper_kzg_commitment
        );
        commitment_map
            .update_commitments(
                refs.hyper_kzg_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: Some(new_hyper_kzg_commitment),
                    dynamic_dory: None,
                },
            )
            .unwrap();
        assert_eq!(
            commitment_map
                .hyper_kzg_map
                .get(&refs.hyper_kzg_ref)
                .unwrap(),
            &new_hyper_kzg_commitment
        );

        assert_ne!(
            commitment_map.dory_map.get(&refs.dory_ref).unwrap(),
            &new_dory_commitment
        );
        commitment_map
            .update_commitments(
                refs.dory_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: None,
                    dynamic_dory: Some(new_dory_commitment),
                },
            )
            .unwrap();
        assert_eq!(
            commitment_map.dory_map.get(&refs.dory_ref).unwrap(),
            &new_dory_commitment
        );

        assert_ne!(
            commitment_map.hyper_kzg_map.get(&refs.all_ref).unwrap(),
            &new_hyper_kzg_commitment
        );
        assert_ne!(
            commitment_map.dory_map.get(&refs.all_ref).unwrap(),
            &new_dory_commitment
        );
        commitment_map
            .update_commitments(
                refs.all_ref.clone(),
                PerCommitmentScheme {
                    hyper_kzg: Some(new_hyper_kzg_commitment),
                    dynamic_dory: Some(new_dory_commitment),
                },
            )
            .unwrap();
        assert_eq!(
            commitment_map.hyper_kzg_map.get(&refs.all_ref).unwrap(),
            &new_hyper_kzg_commitment
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

        let new_hyper_kzg_commitment = TestCommitmentMetadata::<HyperKZGCommitment>::new(10);
        let new_dory_commitment = TestCommitmentMetadata::<DynamicDoryCommitment>::new(20);

        let no_commitments = PerCommitmentScheme::default();
        assert!(matches!(
            commitment_map.update_commitments(refs.hyper_kzg_ref.clone(), no_commitments),
            Err(CommitmentSchemesMismatchError { .. })
        ));

        let dory_commitments = PerCommitmentScheme {
            hyper_kzg: None,
            dynamic_dory: Some(new_dory_commitment),
        };
        assert!(matches!(
            commitment_map.update_commitments(refs.hyper_kzg_ref.clone(), dory_commitments),
            Err(CommitmentSchemesMismatchError { .. })
        ));

        let all_commitments = PerCommitmentScheme {
            hyper_kzg: Some(new_hyper_kzg_commitment),
            dynamic_dory: Some(new_dory_commitment),
        };
        assert!(matches!(
            commitment_map.update_commitments(refs.hyper_kzg_ref, all_commitments),
            Err(CommitmentSchemesMismatchError { .. })
        ));

        // commitment_map was not mutated during failures
        assert_eq!(commitment_map, original_commitment_map);
    }
}
