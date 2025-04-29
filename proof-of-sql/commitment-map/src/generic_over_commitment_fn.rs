use proof_of_sql::base::commitment::Commitment;

use crate::generic_over_commitment::GenericOverCommitment;

/// Trait for writing functions that are generic over commitments.
///
/// Can map one [`GenericOverCommitment::WithCommitment<C>`] to another, and be supplied to..
/// - [`AnyCommitmentScheme::map`] for dynamic use cases
/// - [`PerCommitmentScheme::map`] for atomic use cases
pub trait GenericOverCommitmentFn {
    /// The input type of the mapper.
    type In: GenericOverCommitment;
    /// The output type of the mapper.
    type Out: GenericOverCommitment;

    /// Mapping function that is generic over commitment.
    fn call<C: Commitment>(
        &self,
        input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C>;
}

impl<F> GenericOverCommitmentFn for &F
where
    F: GenericOverCommitmentFn,
{
    type In = F::In;
    type Out = F::Out;

    fn call<C: Commitment>(
        &self,
        input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
    ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
        F::call(self, input)
    }
}

#[cfg(test)]
pub mod tests {
    use core::marker::PhantomData;

    use curve25519_dalek::RistrettoPoint;
    use proof_of_sql::proof_primitive::dory::DynamicDoryCommitment;

    use super::*;
    use crate::generic_over_commitment::{CommitmentType, OptionType};

    pub struct SomeFn<T: GenericOverCommitment>(PhantomData<T>);

    impl<T: GenericOverCommitment> SomeFn<T> {
        pub fn new() -> Self {
            SomeFn(PhantomData)
        }
    }

    impl<T: GenericOverCommitment> GenericOverCommitmentFn for SomeFn<T> {
        type In = T;
        type Out = OptionType<T>;

        fn call<C: Commitment>(
            &self,
            input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
        ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
            Some(input)
        }
    }

    #[test]
    fn we_can_call_generic_over_commitment_fn() {
        let some_fn = SomeFn::<CommitmentType>::new();

        assert_eq!(
            some_fn.call::<RistrettoPoint>(Default::default()),
            Some(Default::default())
        );

        assert_eq!(
            some_fn.call::<DynamicDoryCommitment>(Default::default()),
            Some(Default::default())
        );
    }
}
