use crate::mock::*;

use frame_support::assert_ok;
use sxt_core::tables::{SourceAndMode, UpdateTableList};

#[test]
fn test_pallet() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
    })
}

#[test]
fn update_tables_should_work_when_permissioned() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // The account id of the user in question
        let user = 1;

        // give the user permission to edit the tables pallet schema
        let permissions = sxt_core::permissions::TablesPalletPermission::EditSchema;
        let permissions = sxt_core::permissions::PermissionLevel::TablesPallet(permissions);
        let permissions =
            sxt_core::permissions::PermissionList::try_from(vec![permissions]).unwrap();
        assert_ok!(
            pallet_permissions::Pallet::<Test>::set_permissions(
                RuntimeOrigin::root(),
                user,
                permissions
            ),
            ()
        );

        let source_and_mode = SourceAndMode::default();
        let tables = UpdateTableList::default();

        assert_ok!(
            Tables::update_tables(RuntimeOrigin::signed(user), source_and_mode, tables),
            ()
        );
    })
}

#[test]
fn update_tables_should_work_when_sudo() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(
            Tables::update_tables(
                RuntimeOrigin::root(),
                SourceAndMode::default(),
                UpdateTableList::default()
            ),
            ()
        );
    })
}
