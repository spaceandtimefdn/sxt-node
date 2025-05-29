use std::str::from_utf8;

use frame_support::assert_ok;
use frame_support::traits::OriginTrait;
use pallet_permissions::Pallet;
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use sp_core::ConstU32;
use sp_runtime::BoundedVec;
use sxt_core::permissions::{PermissionLevel, PermissionList, TablesPalletPermission};
use sxt_core::tables::{
    CreateStatement,
    Source,
    SourceAndMode,
    TableIdentifier,
    TableName,
    TableNamespace,
    TableType,
    TableUuid,
    TableVersion,
};

use crate::mock::*;
use crate::{
    CommitmentCreationCmd,
    CreateTableList,
    Event,
    NamespaceVersions,
    TableVersions,
    UpdateTable,
    UpdateTableList,
};

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

#[test]
fn create_table_should_handle_withs_properly() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let test_identifier = TableIdentifier {
            name: Default::default(),
            namespace: Default::default(),
        };

        let ddl = r#"CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (
            TIME_STAMP TIMESTAMP NOT NULL,
            BLOCK_NUMBER BIGINT NOT NULL,
            BLOCK_HASH BINARY NOT NULL,
            GAS_LIMIT DECIMAL(75, 0) NOT NULL,
            GAS_USED DECIMAL(75, 0) NOT NULL,
            MINER BINARY NOT NULL,
            PARENT_HASH BINARY NOT NULL,
            REWARD DECIMAL(75, 0) NOT NULL,
            SIZE BIGINT NOT NULL,
            TRANSACTION_COUNT INT NOT NULL,
            NONCE BINARY NOT NULL,
            RECEIPTS_ROOT BINARY NOT NULL,
            SHA3_UNCLES BINARY NOT NULL,
            STATE_ROOT BINARY NOT NULL,
            TRANSACTIONS_ROOT BINARY NOT NULL,
            UNCLES_COUNT BIGINT NOT NULL,
            PRIMARY KEY (BLOCK_NUMBER)
        ) WITH (TABLE_UUID=F801A872785FAB3F16C51CF7A1969000);"#;

        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::CoreBlockchain,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
            .expect("Table list should fit in BoundedVec");

        assert_ok!(Tables::create_tables(RuntimeOrigin::root(), tables.clone()));

        let expected_uuid =
            TableUuid::try_from("F801A872785FAB3F16C51CF7A1969000".as_bytes().to_vec()).unwrap();
        assert!(TableVersions::<Test>::contains_key(&test_identifier, 0));
        assert_eq!(
            TableVersions::<Test>::get(&test_identifier, 0),
            expected_uuid
        );

        let expected_sql = "CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, BLOCK_HASH BINARY NOT NULL, GAS_LIMIT DECIMAL(75,0) NOT NULL, GAS_USED DECIMAL(75,0) NOT NULL, MINER BINARY NOT NULL, PARENT_HASH BINARY NOT NULL, REWARD DECIMAL(75,0) NOT NULL, SIZE BIGINT NOT NULL, TRANSACTION_COUNT INT NOT NULL, NONCE BINARY NOT NULL, RECEIPTS_ROOT BINARY NOT NULL, SHA3_UNCLES BINARY NOT NULL, STATE_ROOT BINARY NOT NULL, TRANSACTIONS_ROOT BINARY NOT NULL, UNCLES_COUNT BIGINT NOT NULL, META_ROW_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER)) WITH (TABLE_UUID=F801A872785FAB3F16C51CF7A1969000);";
        let expected_sql: BoundedVec<u8, ConstU32<8192>> = BoundedVec::try_from(expected_sql.as_bytes().to_vec()).unwrap();

        let events = System::events();
        match events.last().map(|e| &e.event) {
            Some(RuntimeEvent::Tables(crate::Event::SchemaUpdated(_, list))) => {
                if let Some(first_table) = list.first() {
                    let raw = &first_table.create_statement;
                    assert_eq!(*raw, expected_sql);

                }
            }
            _ => panic!("Event not found"),
        }
    });
}

#[test]
fn create_table_should_generate_uuid_and_add_meta_column_without_with_clause() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let test_identifier = TableIdentifier {
            name: Default::default(),
            namespace: Default::default(),
        };

        let ddl = r#"CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (
            TIME_STAMP TIMESTAMP NOT NULL,
            BLOCK_NUMBER BIGINT NOT NULL,
            BLOCK_HASH BINARY NOT NULL,
            GAS_LIMIT DECIMAL(75, 0) NOT NULL,
            GAS_USED DECIMAL(75, 0) NOT NULL,
            MINER BINARY NOT NULL,
            PARENT_HASH BINARY NOT NULL,
            REWARD DECIMAL(75, 0) NOT NULL,
            SIZE BIGINT NOT NULL,
            TRANSACTION_COUNT INT NOT NULL,
            NONCE BINARY NOT NULL,
            RECEIPTS_ROOT BINARY NOT NULL,
            SHA3_UNCLES BINARY NOT NULL,
            STATE_ROOT BINARY NOT NULL,
            TRANSACTIONS_ROOT BINARY NOT NULL,
            UNCLES_COUNT BIGINT NOT NULL,
            PRIMARY KEY (BLOCK_NUMBER)
        );"#;

        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::CoreBlockchain,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
            .expect("Table list should fit in BoundedVec");

        assert_ok!(Tables::create_tables(RuntimeOrigin::root(), tables.clone()));

        assert!(TableVersions::<Test>::contains_key(&test_identifier, 0));
        let generated_uuid = TableVersions::<Test>::get(&test_identifier, 0);
        assert!(!generated_uuid.is_empty());

        let expected = "CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, BLOCK_HASH BINARY NOT NULL, GAS_LIMIT DECIMAL(75,0) NOT NULL, GAS_USED DECIMAL(75,0) NOT NULL, MINER BINARY NOT NULL, PARENT_HASH BINARY NOT NULL, REWARD DECIMAL(75,0) NOT NULL, SIZE BIGINT NOT NULL, TRANSACTION_COUNT INT NOT NULL, NONCE BINARY NOT NULL, RECEIPTS_ROOT BINARY NOT NULL, SHA3_UNCLES BINARY NOT NULL, STATE_ROOT BINARY NOT NULL, TRANSACTIONS_ROOT BINARY NOT NULL, UNCLES_COUNT BIGINT NOT NULL, META_ROW_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let events = System::events();
        match events.last().map(|e| &e.event) {
            Some(RuntimeEvent::Tables(crate::Event::SchemaUpdated(_, list))) => {
                if let Some(first_table) = list.first() {
                    let raw = &first_table.create_statement;
                    let sql_str = String::from_utf8(raw.to_vec()).unwrap();
                    assert_eq!(expected, sql_str);
                }
            }
            _ => panic!("Expected SchemaUpdated event not found"),
        }
    });
}

#[test]
fn update_namespace_uuid_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let namespace: TableNamespace =
            TableNamespace::try_from("TEST_NAMESPACE".as_bytes().to_vec()).unwrap();
        let version = 1;
        let old_uuid: TableUuid = TableUuid::try_from("TEST-ID-OLD".as_bytes().to_vec()).unwrap();
        let new_uuid =
            TableUuid::try_from("F801A872785FAB3F16C51CF7A1969000".as_bytes().to_vec()).unwrap();

        // Simulate original UUID
        NamespaceVersions::<Test>::insert(&namespace, version, old_uuid.clone());

        // Grant permission
        let (who, signer) = user(1);
        set_permission!(who, TablesPalletPermission::EditSchema);

        // Call extrinsic
        assert_ok!(Tables::update_namespace_uuid(
            RuntimeOrigin::signed(who),
            namespace.clone(),
            version,
            new_uuid.clone()
        ));

        // Check storage
        assert_eq!(
            NamespaceVersions::<Test>::get(&namespace, version),
            new_uuid
        );

        // Check event
        System::assert_last_event(
            Event::NamespaceUuidUpdated {
                old_uuid,
                new_uuid,
                version,
                namespace,
            }
            .into(),
        );
    });
}

#[test]
fn update_table_uuid_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let table = TableIdentifier {
            namespace: TableNamespace::try_from("TEST_NAMESPACE".as_bytes().to_vec()).unwrap(),
            name: TableName::try_from("TEST_TABLE".as_bytes().to_vec()).unwrap(),
        };
        let version = 1;

        let old_uuid: TableUuid = TableUuid::try_from("TEST-ID-OLD".as_bytes().to_vec()).unwrap();
        let new_uuid =
            TableUuid::try_from("F801A872785FAB3F16C51CF7A1969000".as_bytes().to_vec()).unwrap();

        // Simulate original UUID
        TableVersions::<Test>::insert(&table, version, old_uuid.clone());

        // Grant permission
        let (who, signer) = user(1);
        set_permission!(who, TablesPalletPermission::EditSchema);

        // Call extrinsic
        assert_ok!(Tables::update_table_uuid(
            RuntimeOrigin::signed(who),
            table.clone(),
            version,
            new_uuid.clone()
        ));

        // Check storage
        assert_eq!(TableVersions::<Test>::get(&table, version), new_uuid);

        System::assert_last_event(
            Event::TableUuidUpdated {
                old_uuid,
                new_uuid,
                version,
                table,
            }
            .into(),
        );
    });
}

#[test]
fn test_get_or_generate_uuids_for_table_generates_uuids_if_missing() {
    new_test_ext().execute_with(|| {
        // Arrange
        let ddl = "CREATE TABLE ETHEREUM.TEST (COL1 BIGINT NOT NULL);";
        let statement = BoundedVec::try_from(ddl.as_bytes().to_vec()).unwrap();

        let identifier = TableIdentifier {
            namespace: b"ETHEREUM".to_vec().try_into().unwrap(),
            name: b"TEST".to_vec().try_into().unwrap(),
        };

        // Act
        let (table_uuid, column_uuids) =
            Tables::get_or_generate_uuids_for_table2(statement, identifier)
                .expect("should return generated uuids");

        // Assert
        assert!(
            table_uuid != TableUuid::default(),
            "Expected a non-default table UUID"
        );
        assert!(
            !column_uuids.is_empty(),
            "Expected at least one column UUID"
        );
        println!("✅ Table UUID: {:?}", table_uuid);
        println!("✅ Column UUIDs: {:?}", column_uuids);
    });
}
