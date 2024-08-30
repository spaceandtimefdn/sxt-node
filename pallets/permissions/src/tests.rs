use crate::{mock::*, Error, Event};
use frame_support::{assert_err, assert_ok};
use sp_runtime::{traits::BadOrigin, BoundedVec};
use sxt_core::permissions::*;

/// Calling set_permissions should fail when the signer is not root or does not have the proper permissions set
#[test]
fn set_permissions_should_fail_when_not_root_and_not_permissioned() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signer = RuntimeOrigin::signed(1);
        let permissions: PermissionList = BoundedVec::default();

        assert_err!(
            Permissions::set_permissions(signer, 1, permissions),
            Error::<Test>::InsufficientPermissions
        );
    })
}

#[test]
fn set_permissions_should_work_permissioned() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let root = RuntimeOrigin::root();
        let who = 1;

        let permissions =
            PermissionList::try_from(vec![PermissionLevel::UpdatePermissions]).unwrap();

        assert_ok!(Permissions::set_permissions(root, who, permissions), ());

        let created = PermissionLevel::UserCreated(
            sxt_core::permissions::UserCreatedPermissionLevel::try_from(
                "ThreatLevelMidnight".as_bytes().to_vec(),
            )
            .unwrap(),
        );

        let permissions = PermissionList::try_from(vec![created]).unwrap();

        let omega_user = 2;

        assert_ok!(
            Permissions::set_permissions(RuntimeOrigin::signed(who), omega_user, permissions),
            ()
        )
    })
}

#[test]
fn set_permissions_should_work_sudo() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signer = RuntimeOrigin::root();
        let permissions: PermissionList =
            BoundedVec::try_from(vec![PermissionLevel::UpdatePermissions]).unwrap();

        assert_ok!(Permissions::set_permissions(signer, 1, permissions), (),);
    })
}

#[test]
fn ensure_root_or_permissioned_should_work_root() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert!(Permissions::ensure_root_or_permissioned(
            RuntimeOrigin::root(),
            &PermissionLevel::UpdatePermissions
        )
        .is_ok());
    })
}

#[test]
fn ensure_root_or_permissioned_should_work_permissioned() {
    new_test_ext().execute_with(|| {
        let permissions =
            PermissionList::try_from(vec![PermissionLevel::UpdatePermissions]).unwrap();

        assert_ok!(
            Permissions::set_permissions(RuntimeOrigin::root(), 1, permissions),
            (),
        );

        assert!(Permissions::ensure_root_or_permissioned(
            RuntimeOrigin::signed(1),
            &PermissionLevel::UpdatePermissions
        )
        .is_ok());
    })
}

#[test]
fn set_permissions_should_emit_event_when_successful() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signer = RuntimeOrigin::root();
        let permissions: PermissionList =
            BoundedVec::try_from(vec![PermissionLevel::UpdatePermissions]).unwrap();
        let user = 1;

        assert_ok!(
            Permissions::set_permissions(signer, user, permissions.clone()),
            (),
        );
        System::assert_last_event(Event::<Test>::PermissionsSet(user, permissions).into());
    })
}

#[test]
fn clear_permissions_should_work_when_permissioned() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let root = RuntimeOrigin::root();
        let who = 1;

        let permissions =
            PermissionList::try_from(vec![PermissionLevel::UpdatePermissions]).unwrap();

        assert_ok!(Permissions::set_permissions(root, who, permissions), ());

        let test_user = 2;

        assert_ok!(
            Permissions::clear_permissions(RuntimeOrigin::signed(who), test_user,),
            ()
        )
    })
}

#[test]
fn clear_permissions_should_work_when_sudo() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let test_user = 2;

        assert_ok!(
            Permissions::clear_permissions(RuntimeOrigin::root(), test_user,),
            ()
        )
    })
}

#[test]
fn clear_permissions_should_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let signer = RuntimeOrigin::root();
        let new_permissions: PermissionList =
            BoundedVec::try_from(vec![PermissionLevel::UpdatePermissions]).unwrap();
        let user = 1;

        assert_ok!(
            Permissions::set_permissions(signer.clone(), user, new_permissions),
            (),
        );
        assert_ok!(Permissions::clear_permissions(signer, user), ());

        let p = Permissions::permissions(user);

        assert!(p.is_none());
    })
}

#[test]
fn set_permissions_should_fail_with_empty_list() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let signer = RuntimeOrigin::root();
        let user = 1;
        let permissions = PermissionList::default();

        assert_err!(
            Permissions::set_permissions(signer, user, permissions),
            Error::<Test>::EmptyPermissionsListError
        );
    });
}

#[test]
fn clear_permissions_should_work_with_no_permissions_set() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let signer = RuntimeOrigin::root();
        let user = 1;

        assert_ok!(Permissions::clear_permissions(signer, user), ());
    })
}

#[test]
fn ensure_root_or_permissioned_should_fail_when_not_signed() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let signer = RuntimeOrigin::none();

        assert_err!(
            Permissions::ensure_root_or_permissioned(signer, &PermissionLevel::UpdatePermissions),
            Error::<Test>::UnsignedTransaction,
        );
    })
}
