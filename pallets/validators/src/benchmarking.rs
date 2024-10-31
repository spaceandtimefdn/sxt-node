//! Benchmarking setup for pallet-template
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

use super::*;
#[allow(unused)]
use crate::Pallet as ValidatorsPallet;

 
use scale_info::prelude::vec;
use sp_runtime::BoundedVec;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn add_validators_works() {
        let caller: T::ValidatorId = whitelisted_caller();

        #[extrinsic_call]
        add_validator(RawOrigin::Root, caller.clone());
    }

    #[benchmark]
    fn clear_permissions_works() {
        let caller: T::ValidatorId = whitelisted_caller();

        let validators = BoundedVec::try_from(vec![caller.clone()]).unwrap();
        
        Validators::<T>::set(validators);

        #[extrinsic_call]
        remove_validator(RawOrigin::Root, caller.clone());
    }

    impl_benchmark_test_suite!(ValidatorsPallet, crate::mock::new_test_ext(), crate::mock::Test);
}
