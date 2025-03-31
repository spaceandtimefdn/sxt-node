use frame_support::assert_ok;
use pallet_permissions::Pallet;
use sp_runtime::BoundedVec;
use sxt_core::permissions::{PermissionLevel, PermissionList, TablesPalletPermission};
use sxt_core::tables::{Source, SourceAndMode, TableType};

use crate::mock::*;
use crate::{CreateTableList, UpdateTableList};

// Give $who permission $p
macro_rules! set_permission {
    ($who: expr, $p: expr) => {
        assert_ok!(
            Pallet::<Test>::set_permissions(
                RuntimeOrigin::root(),
                $who,
                PermissionList::try_from(vec![PermissionLevel::TablesPallet($p)]).unwrap()
            ),
            ()
        );
    };
}

// Create a user from an integer and created a signed origin for it
fn user(i: u64) -> (u64, RuntimeOrigin) {
    (i, RuntimeOrigin::signed(i))
}

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

        let (who, signer) = user(1);

        set_permission!(who, TablesPalletPermission::EditSchema);

        assert_ok!(
            Tables::create_tables(signer, UpdateTableList::default()),
            ()
        );
    })
}

#[test]
fn update_tables_should_work_when_sudo() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(
            Tables::create_tables(RuntimeOrigin::root(), UpdateTableList::default()),
            ()
        );
    })
}

#[test]
fn create_tables_should_work_when_sudo() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(
            Tables::create_tables_with_snapshot_and_commitment(
                RuntimeOrigin::root(),
                SourceAndMode::default(),
                CreateTableList::default(),
            ),
            ()
        );
    })
}

#[test]
fn create_tables_should_work_when_permissioned() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let (who, signer) = user(1);

        set_permission!(who, TablesPalletPermission::EditSchema);

        assert_ok!(
            Tables::create_tables_with_snapshot_and_commitment(
                signer,
                SourceAndMode::default(),
                CreateTableList::default(),
            ),
            ()
        );
    })
}

#[test]
fn create_namespace_should_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let schema_name = BoundedVec::try_from("TEST_GEORGE".as_bytes().to_vec()).unwrap();
        let version = 1;
        let create_statement = BoundedVec::try_from(
            "CREATE SCHEMA IF NOT EXISTS TEST_GEORGE;"
                .as_bytes()
                .to_vec(),
        )
        .unwrap();
        let table_type = TableType::CoreBlockchain;
        let source = Source::Ethereum;

        assert_ok!(Tables::create_namespace(
            RuntimeOrigin::root(),
            schema_name,
            version,
            create_statement,
            table_type,
            source
        ));
    })
}
