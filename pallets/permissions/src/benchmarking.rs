//! Benchmarking setup for pallet-template
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use scale_info::prelude::vec;

use super::*;
#[allow(unused)]
use crate::Pallet as PermissionPallet;

#[benchmarks]
mod benchmarks {
    use sxt_core::permissions::{PermissionLevel, PermissionList};

    use super::*;

    #[benchmark]
    fn set_permissions() {
        let caller: T::AccountId = whitelisted_caller();

        let permission_level = PermissionLevel::UpdatePermissions;
        let permission_list = PermissionList::try_from(vec![permission_level]).unwrap();

        #[extrinsic_call]
        set_permissions(RawOrigin::Root, caller.clone(), permission_list.clone());

        assert_eq!(Permissions::<T>::get(caller), Some(permission_list));
    }

    #[benchmark]
    fn clear_permissions() {
        let caller: T::AccountId = whitelisted_caller();

        let permission_level = PermissionLevel::UpdatePermissions;
        let permission_list = PermissionList::try_from(vec![permission_level]).unwrap();

        Permissions::<T>::insert(caller.clone(), permission_list);

        #[extrinsic_call]
        clear_permissions(RawOrigin::Root, caller.clone());

        assert_eq!(Permissions::<T>::get(caller), None);
    }

    impl_benchmark_test_suite!(
        PermissionPallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}
