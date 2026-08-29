use core::str::from_utf8;

use pallet_permissions::Pallet;
use polkadot_sdk::frame_support::traits::fungible::Mutate;
use polkadot_sdk::frame_support::{assert_err, assert_ok};
use polkadot_sdk::sp_runtime::{BoundedVec, DispatchError, TokenError};
use polkadot_sdk::{pallet_balances, sp_runtime};
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use sqlparser::ast::{ColumnDef, DataType, ExactNumberInfo, Ident, TimezoneInfo};
use sxt_core::permissions::{
    IndexingPalletPermission,
    PermissionLevel,
    PermissionList,
    TablesPalletPermission,
};
use sxt_core::tables::{
    ColumnUuid,
    ColumnUuidList,
    CreateStatement,
    GetTableSchemaError,
    InsertQuorumSize,
    Source,
    SourceAndMode,
    TableIdentifier,
    TableMetadataBytes,
    TableName,
    TableNamespace,
    TableType,
    TableUuid,
    TryNormalize,
};
use sxt_core::ByteString;

use crate::mock::*;
use crate::{
    ColumnVersions,
    CommitmentCreationCmd,
    CreateSciTableRequest,
    CreateTableList,
    Error,
    Event,
    Identifiers,
    NamespaceVersions,
    Schemas,
    Snapshots,
    TableInsertQuorums,
    TableMetadata,
    TableOwners,
    TableSources,
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

const ETH_TEST_WALLET: &str = "44bCf7001D9C3fe8b7aA2BBaaf1B94410db31f5c";

fn test_tables() -> UpdateTableList {
    let test_identifier =
        TableIdentifier::from_str_unchecked_with_preserved_casing("BLOCKS", "ETHEREUM");

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

    BoundedVec::try_from(vec![UpdateTable {
        ident: test_identifier.clone(),
        create_statement: create_statement.clone(),
        table_type: TableType::CoreBlockchain,
        commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
        source: Source::Ethereum,
    }])
    .expect("Table list should fit in BoundedVec")
}

// Create a user from an integer and created a signed origin for it
fn user(i: u8) -> (sp_runtime::AccountId32, RuntimeOrigin) {
    let who = sp_runtime::AccountId32::new([i; 32]);
    (who.clone(), RuntimeOrigin::signed(who.clone()))
}

fn create_namespace_for_testing(namespace: &str) {
    assert_ok!(Tables::create_namespace(
        RuntimeOrigin::root(),
        namespace.as_bytes().to_vec().try_into().unwrap(),
        0,
        format!("CREATE SCHEMA IF NOT EXISTS {}", namespace)
            .as_bytes()
            .to_vec()
            .try_into()
            .unwrap(),
        TableType::CoreBlockchain,
        sxt_core::tables::Source::Ethereum,
    ));
}

/// Creates a generic community table for testing using the supplied signer
fn create_community_table_for_user(
    signer: RuntimeOrigin,
    namespace: &str,
    table_name: &str,
) -> TableIdentifier {
    let test_identifier =
        TableIdentifier::from_str_unchecked_with_preserved_casing(table_name, namespace);

    let ddl = format!(
        "CREATE TABLE IF NOT EXISTS {}.{} (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));",
        namespace, table_name
    );
    let create_statement: CreateStatement =
        BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

    let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
        ident: test_identifier.clone(),
        create_statement,
        table_type: TableType::Community,
        commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
        source: Source::UserCreated(ByteString::default()),
    }])
    .expect("Table list should fit in BoundedVec");

    assert_ok!(Tables::create_tables(signer, tables));

    test_identifier.try_normalize().unwrap()
}

/// asserts that a given AccountId has permission to insert data into a given table as well as
/// permission to give another user the right to insert data.
fn assert_has_table_owner_permissions(
    who: &sp_runtime::AccountId32,
    table_identifier: &TableIdentifier,
) {
    let submit_permission = PermissionLevel::IndexingPallet(
        IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_identifier.clone()),
    );
    assert!(
        pallet_permissions::Pallet::<Test>::has_permissions(who, &submit_permission),
        "Owner should have SubmitDataForPrivilegedQuorum permission for table {:?}",
        table_identifier
    );

    let meta_permission =
        PermissionLevel::EditSpecificPermission(Box::new(PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(table_identifier.clone()),
        )));
    assert!(
        pallet_permissions::Pallet::<Test>::has_permissions(who, &meta_permission),
        "Owner should have EditSpecificPermission for SubmitDataForPrivilegedQuorum for table {:?}",
        table_identifier
    );

    let quorum_permission = PermissionLevel::TablesPallet(
        TablesPalletPermission::UpdateTableQuorum(table_identifier.clone()),
    );
    assert!(
        pallet_permissions::Pallet::<Test>::has_permissions(who, &quorum_permission),
        "Owner should have UpdateTableQuorum permission for table {:?}",
        table_identifier
    );

    let quorum_meta_permission =
        PermissionLevel::EditSpecificPermission(Box::new(PermissionLevel::TablesPallet(
            TablesPalletPermission::UpdateTableQuorum(table_identifier.clone()),
        )));
    assert!(
        pallet_permissions::Pallet::<Test>::has_permissions(who, &quorum_meta_permission),
        "Owner should have EditSpecificPermission for UpdateTableQuorum for table {:?}",
        table_identifier
    );
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
        create_namespace_for_testing("ETHEREUM");

        let (who, signer) = user(1);

        set_permission!(who, TablesPalletPermission::EditSchema);

        assert_ok!(Tables::create_tables(signer, test_tables()), ());
    })
}

#[test]
fn update_tables_should_work_when_sudo() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        assert_ok!(
            Tables::create_tables(RuntimeOrigin::root(), test_tables()),
            ()
        );
    })
}

#[test]
fn update_tables_cannot_work_when_namespace_does_not_exist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert!(Tables::create_tables(RuntimeOrigin::root(), test_tables()).is_err());
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
fn create_namespace_should_fail_when_schema_name_does_not_match_ddl() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let schema_name = BoundedVec::try_from("TEST_A".as_bytes().to_vec()).unwrap();
        let version = 1;
        let create_statement =
            BoundedVec::try_from("CREATE SCHEMA IF NOT EXISTS TEST_B;".as_bytes().to_vec())
                .unwrap();
        let table_type = TableType::CoreBlockchain;
        let source = Source::Ethereum;

        assert_err!(
            Tables::create_namespace(
                RuntimeOrigin::root(),
                schema_name,
                version,
                create_statement,
                table_type,
                source
            ),
            Error::<Test>::InvalidNamespace
        );
    })
}

#[test]
fn create_namespace_should_work_when_casing_doesnt_match() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let schema_name = BoundedVec::try_from("test_a".as_bytes().to_vec()).unwrap();
        let version = 1;
        let create_statement =
            BoundedVec::try_from("CREATE SCHEMA IF NOT EXISTS TEST_A;".as_bytes().to_vec())
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
fn create_table_invalidates_mismatched_table_identifier() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("RIGHT");
        let tables = vec![UpdateTable {
            ident: TableIdentifier::from_str_unchecked_with_preserved_casing("NAME", "RIGHT"),
            create_statement: CreateStatement::try_from(
                b"CREATE TABLE WRONG.NAME (BLOCK_NUMBER BIGINT NOT NULL)".to_vec(),
            )
            .unwrap(),
            table_type: TableType::Community,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::UserCreated(ByteString::default()),
        }];

        assert_err!(
            Tables::create_tables(RuntimeOrigin::root(), tables.try_into().unwrap()),
            crate::Error::<Test>::TableIdentifierParsingError
        );
    });
}

#[test]
fn create_table_accepts_different_cased_table_identifier() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("Right");
        let tables = vec![UpdateTable {
            ident: TableIdentifier::from_str_unchecked_with_preserved_casing("NAME", "RIGHT"),
            create_statement: CreateStatement::try_from(
                b"CREATE TABLE Right.Name (BLOCK_NUMBER BIGINT NOT NULL)".to_vec(),
            )
            .unwrap(),
            table_type: TableType::Community,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::UserCreated(ByteString::default()),
        }];

        assert_ok!(Tables::create_tables(
            RuntimeOrigin::root(),
            tables.try_into().unwrap()
        ));
    });
}

#[test]
fn create_table_should_handle_withs_properly() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        let test_identifier = TableIdentifier::from_str_unchecked_with_preserved_casing(
            "BLOCKS", 
            "ETHEREUM"
        );

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

        let expected_sql = "CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, BLOCK_HASH BINARY NOT NULL, GAS_LIMIT DECIMAL(75,0) NOT NULL, GAS_USED DECIMAL(75,0) NOT NULL, MINER BINARY NOT NULL, PARENT_HASH BINARY NOT NULL, REWARD DECIMAL(75,0) NOT NULL, SIZE BIGINT NOT NULL, TRANSACTION_COUNT INT NOT NULL, NONCE BINARY NOT NULL, RECEIPTS_ROOT BINARY NOT NULL, SHA3_UNCLES BINARY NOT NULL, STATE_ROOT BINARY NOT NULL, TRANSACTIONS_ROOT BINARY NOT NULL, UNCLES_COUNT BIGINT NOT NULL, META_ROW_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER, META_ROW_NUMBER)) WITH (TABLE_UUID = F801A872785FAB3F16C51CF7A1969000);";
        let events = System::events();
        match events.last().map(|e| &e.event) {
            Some(RuntimeEvent::Tables(crate::Event::SchemaUpdated(_, list))) => {
                if let Some(first_table) = list.first() {
                    let raw = &first_table.create_statement;
                    let sql_str = String::from_utf8(raw.to_vec()).unwrap();
                    assert_eq!(sql_str, expected_sql);

                }
            }
            _ => panic!("Event not found"),
        }
    });
}

#[test]
fn create_table_should_generate_uuid_and_add_meta_column_including_with_clause() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        let test_identifier = TableIdentifier::from_str_unchecked_with_preserved_casing(
            "BLOCKS", 
            "ETHEREUM"
        );

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

        let expected = "CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, BLOCK_HASH BINARY NOT NULL, GAS_LIMIT DECIMAL(75,0) NOT NULL, GAS_USED DECIMAL(75,0) NOT NULL, MINER BINARY NOT NULL, PARENT_HASH BINARY NOT NULL, REWARD DECIMAL(75,0) NOT NULL, SIZE BIGINT NOT NULL, TRANSACTION_COUNT INT NOT NULL, NONCE BINARY NOT NULL, RECEIPTS_ROOT BINARY NOT NULL, SHA3_UNCLES BINARY NOT NULL, STATE_ROOT BINARY NOT NULL, TRANSACTIONS_ROOT BINARY NOT NULL, UNCLES_COUNT BIGINT NOT NULL, META_ROW_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER, META_ROW_NUMBER)) WITH (BLOCK_HASH = BLOCK_HASH, BLOCK_NUMBER = BLOCK_NUMBER, GAS_LIMIT = GAS_LIMIT, GAS_USED = GAS_USED, MINER = MINER, NONCE = NONCE, PARENT_HASH = PARENT_HASH, RECEIPTS_ROOT = RECEIPTS_ROOT, REWARD = REWARD, SHA3_UNCLES = SHA3_UNCLES, SIZE = SIZE, STATE_ROOT = STATE_ROOT, TABLE_UUID = CD1DEC444459D5F4B94FDB803C170305, TIME_STAMP = TIME_STAMP, TRANSACTIONS_ROOT = TRANSACTIONS_ROOT, TRANSACTION_COUNT = TRANSACTION_COUNT, UNCLES_COUNT = UNCLES_COUNT);";
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
        let (who, _) = user(1);
        set_permission!(who.clone(), TablesPalletPermission::EditUuid);

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
        set_permission!(who, TablesPalletPermission::EditUuid);

        // Call extrinsic
        assert_ok!(Tables::update_table_uuid(
            signer,
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
fn test_update_table_uuid_requires_permissions() {
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

        let (_, signer) = user(1);

        // Call extrinsic without assigning permissions to the account
        assert_err!(
            Tables::update_table_uuid(signer, table.clone(), version, new_uuid.clone()),
            pallet_permissions::Error::<Test>::InsufficientPermissions
        );

        // Make sure the storage is the same as before the call
        assert_eq!(TableVersions::<Test>::get(&table, version), old_uuid);
    })
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
            Tables::get_or_generate_uuids_for_table(statement, identifier)
                .expect("should return generated uuids");

        // Assert
        assert!(
            table_uuid != TableUuid::default(),
            "Expected a non-default table UUID"
        );

        // Must be valid UTF8
        assert_ok!(from_utf8(table_uuid.as_ref()));

        assert!(
            !column_uuids.is_empty(),
            "Expected at least one column UUID"
        );
        println!("✅ Table UUID: {:?}", table_uuid);
        println!("✅ Column UUIDs: {:?}", column_uuids);
    });
}

#[test]
fn create_table_with_submitter_column_errors() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let (who, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked_with_preserved_casing(
            "VOTES",
            "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
        );
        let ddl = "CREATE TABLE IF NOT EXISTS EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.VOTES (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, META_SUBMITTER BINARY NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::PublicPermissionless,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");

        // Permission the table creator
        assert_ok!(pallet_permissions::Pallet::<Test>::add_proxy_permission(
            RuntimeOrigin::root(),
            who.clone(),
            PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema)
        ));

        assert_err!(
            Tables::create_tables(signer, tables.clone()),
            crate::Error::<Test>::ReservedColumnName
        );
    });
}

#[test]
fn create_table_with_meta_column_errors() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let (who, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked_with_preserved_casing(
            "VOTES",
            "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
        );
        let ddl = "CREATE TABLE IF NOT EXISTS EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.VOTES (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, meta_phor BINARY NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::PublicPermissionless,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");

        // Permission the table creator
        assert_ok!(pallet_permissions::Pallet::<Test>::add_proxy_permission(
            RuntimeOrigin::root(),
            who.clone(),
            PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema)
        ));

        assert_err!(
            Tables::create_tables(signer, tables.clone()),
            crate::Error::<Test>::ReservedColumnName
        );
    });
}

#[test]
fn creating_community_table_succeeds_with_no_special_permissions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let (who, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked_with_preserved_casing(
            "MY_COMMUNITY_TABLE",
            "FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
        );
        let ddl = "CREATE TABLE IF NOT EXISTS FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.MY_COMMUNITY_TABLE (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::Community,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");        

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &who,
            crate::CREATE_COST * 100,
        ));

        System::reset_events();
        assert_ok!(Tables::create_tables(signer, tables.clone()));

        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&who),
            crate::CREATE_COST * 99
        );

        let ddl_with_uuid = "CREATE TABLE IF NOT EXISTS FUNNAME_5C62CK4URFPIBTOCMESRGF7X9YV9MN38446DHCPSI2MLHIFT.MY_COMMUNITY_TABLE (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, META_ROW_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER, META_ROW_NUMBER)) WITH (BLOCK_NUMBER = BLOCK_NUMBER, TABLE_UUID = D24EBCF10F7CB9EBDC65F9E6823AB72D, TIME_STAMP = TIME_STAMP);";
        let events = System::events();
        match events.last().map(|e| &e.event) {
            Some(RuntimeEvent::Tables(crate::Event::SchemaUpdated(_, list))) => {
                if let Some(first_table) = list.first() {
                    assert_eq!(
                        from_utf8(&first_table.create_statement).unwrap(),
                        ddl_with_uuid
                    )
                } else {
                    panic!("Schema update event had no statements");
                }
            }
            _ => panic!("Expected SchemaUpdated event not found"),
        }

    });
}
#[test]
fn creating_community_table_fails_with_insufficient_funds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let (who, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked_with_preserved_casing(
            "MY_COMMUNITY_TABLE",
            "FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
        );
        let ddl = "CREATE TABLE IF NOT EXISTS FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.MY_COMMUNITY_TABLE (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::Community,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");        

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &who,
            crate::CREATE_COST / 2,
        ));

        assert_err!(Tables::create_tables(signer, tables.clone()), TokenError::FundsUnavailable);
    });
}

#[test]
fn creating_many_community_tables_assigns_correct_permissions() {
    /// How many tables we should try creating
    const TABLE_COUNT: u32 = 50;

    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let namespace = "COMMUNITY_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT";
        create_namespace_for_testing(namespace);

        let (who, signer) = user(1);

        // Fund the account with enough to create all the tables
        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &who,
            crate::CREATE_COST * (TABLE_COUNT as u128 + 1),
        ));

        let tables = (0..TABLE_COUNT).map(|i| {
            let table_name = format!("TABLE_{}", i);
            create_community_table_for_user(signer.clone(), namespace, &table_name)
        });

        // Verify all tables exist and have correct permissions
        for (i, table_identifier) in tables.enumerate() {
            // Verify the table was created
            assert!(
                TableVersions::<Test>::contains_key(&table_identifier, 0),
                "Table {} should exist in TableVersions",
                i
            );

            // Verify the table owner is set correctly
            assert_eq!(
                TableOwners::<Test>::get(&table_identifier),
                Some(who.clone()),
                "Table {} should have correct owner",
                i
            );

            // Verify the permissions were assigned correctly
            assert_has_table_owner_permissions(&who, &table_identifier);
        }
    });
}

#[test]
fn creating_public_permissionless_table_automatically_adds_submitter_column() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let (who, signer) = user(1);
        let test_identifier =
            TableIdentifier::from_str_unchecked_with_preserved_casing("VOTES", "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let ddl = "CREATE TABLE IF NOT EXISTS EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.VOTES (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::PublicPermissionless,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");

        // Permission the table creator
        assert_ok!(pallet_permissions::Pallet::<Test>::add_proxy_permission(
            RuntimeOrigin::root(),
            who.clone(),
            PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema)
        ));

        assert_ok!(Tables::create_tables(signer, tables.clone()));

        let expected_ddl = "CREATE TABLE IF NOT EXISTS EXAMPLE_5C62CK4URFPIBTOCMESRGF7X9YV9MN38446DHCPSI2MLHIFT.VOTES (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, META_SUBMITTER BINARY NOT NULL, META_ROW_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER, META_ROW_NUMBER)) WITH (BLOCK_NUMBER = BLOCK_NUMBER, TABLE_UUID = CCA1F51C06FE00EB3E489D1A083162B5, TIME_STAMP = TIME_STAMP);";

        let events = System::events();
        match events.last().map(|e| &e.event) {
            Some(RuntimeEvent::Tables(crate::Event::SchemaUpdated(_, list))) => {
                if let Some(first_table) = list.first() {
                    assert_eq!(
                        from_utf8(&first_table.create_statement).unwrap(),
                        expected_ddl
                    )
                } else {
                    panic!("Schema update event had no statements");
                }
            }
            _ => panic!("Expected SchemaUpdated event not found"),
        }
    });
}

#[test]
fn create_table_sets_table_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let (who, _) = user(1);
        let test_identifier =
            TableIdentifier::from_str_unchecked_with_preserved_casing("VOTES", "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let ddl = "CREATE TABLE IF NOT EXISTS EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.VOTES (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::PublicPermissionless,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");

        // Permission the table creator
        assert_ok!(pallet_permissions::Pallet::<Test>::add_proxy_permission(
            RuntimeOrigin::root(),
            who.clone(),
            PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema)
        ));

        assert_ok!(Tables::create_tables(
            RuntimeOrigin::signed(who.clone()),
            tables.clone()
        ));

        let normalized_test_identifier = test_identifier.try_normalize().unwrap();
        assert!(TableVersions::<Test>::contains_key(&normalized_test_identifier, 0));

        assert_eq!(TableOwners::<Test>::get(&normalized_test_identifier), Some(who));
    });
}

#[test]
fn creating_a_table_should_automatically_permission_table_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let (who, signer) = user(1);
        let test_identifier =
            TableIdentifier::from_str_unchecked_with_preserved_casing("VOTES", "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");
        let ddl = "CREATE TABLE IF NOT EXISTS EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.VOTES (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::PublicPermissionless,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");

        let edit_permission = PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema);

        // Permission the table creator
        assert_ok!(pallet_permissions::Pallet::<Test>::add_proxy_permission(
            RuntimeOrigin::root(),
            who.clone(),
            edit_permission.clone(),
        ));

        assert!(pallet_permissions::Pallet::<Test>::has_permissions(
            &who,
            &edit_permission
        ));

        assert_ok!(Tables::create_tables(signer, tables.clone()));

        let normalized_test_identifier = test_identifier.try_normalize().unwrap();
        assert!(TableVersions::<Test>::contains_key(&normalized_test_identifier, 0));

        assert_eq!(
            TableOwners::<Test>::get(&normalized_test_identifier),
            Some(who.clone())
        );

        let submit_permission = PermissionLevel::IndexingPallet(
            IndexingPalletPermission::SubmitDataForPrivilegedQuorum(normalized_test_identifier.clone()),
        );

        let meta_permission =
            PermissionLevel::EditSpecificPermission(Box::new(PermissionLevel::IndexingPallet(
                IndexingPalletPermission::SubmitDataForPrivilegedQuorum(normalized_test_identifier.clone()),
            )));

        assert!(pallet_permissions::Pallet::<Test>::has_permissions(
            &who,
            &submit_permission
        ));
        assert!(pallet_permissions::Pallet::<Test>::has_permissions(
            &who,
            &meta_permission
        ));
    });
}

#[test]
fn we_can_get_table_schemas() {
    new_test_ext().execute_with(|| {
        create_namespace_for_testing("ETHEREUM");
        let test_identifier = TableIdentifier {
            name: b"BLOCKS".to_vec().try_into().unwrap(),
            namespace: b"ETHEREUM".to_vec().try_into().unwrap(),
        };

        assert!(matches!(
            Tables::table_schema(test_identifier.clone()),
            Err(GetTableSchemaError::NoSuchTable)
        ));

        let ddl = r#"CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (
            TIME_STAMP TIMESTAMP NOT NULL,
            BLOCK_NUMBER BIGINT NOT NULL,
            BLOCK_HASH BINARY NOT NULL,
            GAS_LIMIT DECIMAL(75, 0) NOT NULL,
            TRANSACTION_COUNT INT NOT NULL,
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

        Tables::create_tables(RuntimeOrigin::root(), tables.clone()).unwrap();

        let table_schema = Tables::table_schema(test_identifier)
            .unwrap()
            .into_iter()
            .map(|column_schema| ColumnDef::try_from(column_schema).unwrap())
            .collect::<Vec<_>>();

        let expected_columns = [
            (
                Ident::new("TIME_STAMP"),
                DataType::Timestamp(None, TimezoneInfo::None),
            ),
            (Ident::new("BLOCK_NUMBER"), DataType::BigInt(None)),
            (Ident::new("BLOCK_HASH"), DataType::Binary(None)),
            (
                Ident::new("GAS_LIMIT"),
                DataType::Decimal(ExactNumberInfo::PrecisionAndScale(75, 0)),
            ),
            (Ident::new("TRANSACTION_COUNT"), DataType::Int(None)),
        ]
        .map(|(name, data_type)| ColumnDef {
            name,
            data_type,
            options: Vec::new(),
            collation: None,
        });

        assert_eq!(table_schema, expected_columns);
    });
}

use polkadot_sdk::sp_core::crypto::Ss58Codec;
#[test]
fn ensure_safe_name_works_for_substrate_address() {
    let (test_account, _) = user(1);
    let ss58 = Ss58Codec::to_ss58check(&test_account);
    println!("{ss58:?}");
    let test_identifier = TableIdentifier::from_str_unchecked(
        "TABLE",
        "SCHEMA_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
    );

    assert_ok!(crate::ensure_safe_namespace::<Test>(
        &test_account,
        &test_identifier.namespace
    ));
}

#[test]
fn we_can_identify_an_eth_address() {
    let account = eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();
    assert!(crate::is_ethereum_address::<Test>(account));
}

use sxt_core::utils::eth_address_to_substrate_account_id;
#[test]
fn ensure_safe_name_works_for_ethereum_address() {
    let test_account = eth_address_to_substrate_account_id::<Test>(ETH_TEST_WALLET).unwrap();

    let test_identifier = TableIdentifier::from_str_unchecked(
        "TABLE",
        "SCHEMA_44bCf7001D9C3fe8b7aA2BBaaf1B94410db31f5c",
    );

    assert_ok!(crate::ensure_safe_namespace::<Test>(
        &test_account,
        &test_identifier.namespace
    ));
}

#[test]
fn table_removal_cleans_up_all_collections() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        let test_identifier = TableIdentifier {
            name: b"BLOCKS".to_vec().try_into().unwrap(),
            namespace: b"ETHEREUM".to_vec().try_into().unwrap(),
        };
        let table_type = TableType::CoreBlockchain;
        let source = Source::Ethereum;

        // Create the table first
        let ddl = r#"CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (
            TIME_STAMP TIMESTAMP NOT NULL,
            BLOCK_NUMBER BIGINT NOT NULL,
            BLOCK_HASH BINARY NOT NULL,
            GAS_LIMIT DECIMAL(75, 0) NOT NULL,
            TRANSACTION_COUNT INT NOT NULL,
            PRIMARY KEY (BLOCK_NUMBER)
        ) WITH (TABLE_UUID=F801A872785FAB3F16C51CF7A1969000);"#;
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: table_type.clone(),
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: source.clone(),
        }])
        .expect("Table list should fit in BoundedVec");

        assert_ok!(Tables::create_tables(RuntimeOrigin::root(), tables));

        // Verify the table exists in all collections before removal
        assert!(Identifiers::<Test>::get(&table_type).contains(&test_identifier));
        assert!(Schemas::<Test>::contains_key(
            &test_identifier.namespace,
            &test_identifier.name
        ));
        assert!(TableInsertQuorums::<Test>::contains_key(&test_identifier));
        assert!(TableSources::<Test>::contains_key(&test_identifier));
        assert!(TableOwners::<Test>::contains_key(&test_identifier));

        // Drop the table
        assert_ok!(Tables::drop_table(
            RuntimeOrigin::root(),
            table_type.clone(),
            test_identifier.clone(),
            source
        ));

        // Verify the table has been removed from all collections
        assert!(!Identifiers::<Test>::get(&table_type).contains(&test_identifier));
        assert!(!Schemas::<Test>::contains_key(
            &test_identifier.namespace,
            &test_identifier.name
        ));
        assert!(!TableInsertQuorums::<Test>::contains_key(&test_identifier));
        assert!(!TableSources::<Test>::contains_key(&test_identifier));
        assert!(!TableOwners::<Test>::contains_key(&test_identifier));

        // Verify Snapshots is also cleaned up (if it was populated)
        assert!(!Snapshots::<Test>::contains_key(&test_identifier));
    });
}

#[test]
fn table_removal_cleans_up_multiple_versions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let test_identifier = TableIdentifier {
            name: TableName::try_from("MULTI_VERSION_TABLE".as_bytes().to_vec()).unwrap(),
            namespace: TableNamespace::try_from("TEST_NAMESPACE".as_bytes().to_vec()).unwrap(),
        };
        let table_type = TableType::CoreBlockchain;
        let source = Source::Ethereum;

        // Manually create multiple versions in TableVersions to test cleanup
        let uuid1 = TableUuid::try_from("UUID1".as_bytes().to_vec()).unwrap();
        let uuid2 = TableUuid::try_from("UUID2".as_bytes().to_vec()).unwrap();
        let column_uuids: ColumnUuidList = BoundedVec::try_from(vec![
            ColumnUuid {
                name: ByteString::try_from("COL1".as_bytes().to_vec()).unwrap(),
                uuid: uuid1.clone(),
            },
            ColumnUuid {
                name: ByteString::try_from("COL2".as_bytes().to_vec()).unwrap(),
                uuid: uuid1.clone(),
            }
        ]).unwrap();

        TableVersions::<Test>::insert(&test_identifier, 0, uuid1);
        TableVersions::<Test>::insert(&test_identifier, 1, uuid2);
        ColumnVersions::<Test>::insert(&test_identifier, 0, column_uuids.clone());
        ColumnVersions::<Test>::insert(&test_identifier, 1, column_uuids.clone());

        // Set up other collections
        let mut identifiers = Identifiers::<Test>::get(&table_type);
        identifiers.try_push(test_identifier.clone()).unwrap();
        Identifiers::<Test>::insert(&table_type, identifiers);

        let ddl = "CREATE TABLE IF NOT EXISTS TEST_NAMESPACE.MULTI_VERSION_TABLE (ID BIGINT NOT NULL, PRIMARY KEY (ID));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        Schemas::<Test>::insert(&test_identifier.namespace, &test_identifier.name, create_statement);
        TableInsertQuorums::<Test>::insert(&test_identifier, InsertQuorumSize {
            privileged: None,
            public: None,
        });
        TableSources::<Test>::insert(&test_identifier, source.clone());
        TableOwners::<Test>::insert(&test_identifier, Some(user(1).0));

        // Verify multiple versions exist
        assert!(TableVersions::<Test>::contains_key(&test_identifier, 0));
        assert!(TableVersions::<Test>::contains_key(&test_identifier, 1));
        assert!(ColumnVersions::<Test>::contains_key(&test_identifier, 0));
        assert!(ColumnVersions::<Test>::contains_key(&test_identifier, 1));

        // Drop the table
        assert_ok!(Tables::drop_table(
            RuntimeOrigin::root(),
            table_type.clone(),
            test_identifier.clone(),
            source
        ));

        // Verify other collections are also cleaned up
        assert!(!Identifiers::<Test>::get(&table_type).contains(&test_identifier));
        assert!(!Schemas::<Test>::contains_key(&test_identifier.namespace, &test_identifier.name));
        assert!(!TableInsertQuorums::<Test>::contains_key(&test_identifier));
        assert!(!TableSources::<Test>::contains_key(&test_identifier));
        assert!(!TableOwners::<Test>::contains_key(&test_identifier));
    });
}

#[test]
fn table_removal_only_affects_target_table() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("TEST_NAMESPACE");

        let table1 = TableIdentifier {
            name: TableName::try_from("TABLE_ONE".as_bytes().to_vec()).unwrap(),
            namespace: TableNamespace::try_from("TEST_NAMESPACE".as_bytes().to_vec()).unwrap(),
        };
        let table2 = TableIdentifier {
            name: TableName::try_from("TABLE_TWO".as_bytes().to_vec()).unwrap(),
            namespace: TableNamespace::try_from("TEST_NAMESPACE".as_bytes().to_vec()).unwrap(),
        };
        let table_type = TableType::CoreBlockchain;
        let source = Source::Ethereum;

        // Create both tables
        let ddl1 = "CREATE TABLE IF NOT EXISTS TEST_NAMESPACE.TABLE_ONE (ID BIGINT NOT NULL, PRIMARY KEY (ID));";
        let ddl2 = "CREATE TABLE IF NOT EXISTS TEST_NAMESPACE.TABLE_TWO (ID BIGINT NOT NULL, PRIMARY KEY (ID));";

        let create_statement1: CreateStatement =
            BoundedVec::try_from(ddl1.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        let create_statement2: CreateStatement =
            BoundedVec::try_from(ddl2.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![
            UpdateTable {
                ident: table1.clone(),
                create_statement: create_statement1,
                table_type: table_type.clone(),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                source: source.clone(),
            },
            UpdateTable {
                ident: table2.clone(),
                create_statement: create_statement2,
                table_type: table_type.clone(),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                source: source.clone(),
            }
        ])
        .expect("Table list should fit in BoundedVec");

        assert_ok!(Tables::create_tables(RuntimeOrigin::root(), tables));

        // Verify both tables exist
        let identifiers_before = Identifiers::<Test>::get(&table_type);
        assert!(identifiers_before.contains(&table1));
        assert!(identifiers_before.contains(&table2));
        assert!(Schemas::<Test>::contains_key(&table1.namespace, &table1.name));
        assert!(Schemas::<Test>::contains_key(&table2.namespace, &table2.name));

        // Drop only table1
        assert_ok!(Tables::drop_table(
            RuntimeOrigin::root(),
            table_type.clone(),
            table1.clone(),
            source
        ));

        // Verify table1 is removed but table2 remains
        let identifiers_after = Identifiers::<Test>::get(&table_type);
        assert!(!identifiers_after.contains(&table1));
        assert!(identifiers_after.contains(&table2));

        assert!(!Schemas::<Test>::contains_key(&table1.namespace, &table1.name));
        assert!(Schemas::<Test>::contains_key(&table2.namespace, &table2.name));

        assert!(!TableInsertQuorums::<Test>::contains_key(&table1));
        assert!(TableInsertQuorums::<Test>::contains_key(&table2));

        assert!(!TableSources::<Test>::contains_key(&table1));
        assert!(TableSources::<Test>::contains_key(&table2));

        assert!(!TableOwners::<Test>::contains_key(&table1));
        assert!(TableOwners::<Test>::contains_key(&table2));
    });
}

#[test]
fn update_quorum_size_for_existing_table_works_and_emits_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");
        let test_identifier = TableIdentifier {
            name: b"BLOCKS".to_vec().try_into().unwrap(),
            namespace: b"ETHEREUM".to_vec().try_into().unwrap(),
        };

        assert!(matches!(
            Tables::table_schema(test_identifier.clone()),
            Err(GetTableSchemaError::NoSuchTable)
        ));

        let ddl = r#"CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS (
            TIME_STAMP TIMESTAMP NOT NULL,
            BLOCK_NUMBER BIGINT NOT NULL,
            BLOCK_HASH BINARY NOT NULL,
            GAS_LIMIT DECIMAL(75, 0) NOT NULL,
            TRANSACTION_COUNT INT NOT NULL,
            PRIMARY KEY (BLOCK_NUMBER)
        ) WITH (TABLE_UUID=F801A872785FAB3F16C51CF7A1969000);"#;

        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let table_type = TableType::CoreBlockchain;

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::CoreBlockchain,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");

        assert_ok!(Tables::create_tables(RuntimeOrigin::root(), tables.clone()));

        let new_quorum = InsertQuorumSize {
            public: None,
            privileged: Some(0),
        };

        // Try to update the quorum on the test table
        assert_ok!(Tables::update_table_quorum(
            RuntimeOrigin::root(),
            test_identifier.clone(),
            new_quorum,
        ));

        // Verify the event was emitted as expected with the correct new and old quorums
        System::assert_has_event(RuntimeEvent::Tables(Event::QuorumUpdated {
            table: test_identifier.clone(),
            old_quorum: Some(InsertQuorumSize::from(table_type)),
            new_quorum,
        }));

        assert_eq!(TableInsertQuorums::<Test>::get(test_identifier), new_quorum);
    });
}

#[test]
fn updating_quorum_for_schema_updates_only_intended_tables_and_emits_events() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("TARGET_NAMESPACE");
        create_namespace_for_testing("OTHER_NAMESPACE");
        // Start by creating the storage state for our test setup
        let target_namespace = TableNamespace::try_from("TARGET_NAMESPACE".as_bytes().to_vec()).unwrap();
        let target_table_one = "TABLE_ONE";
        let target_table_two = "TABLE_TWO";

        let table1 = TableIdentifier {
            name: TableName::try_from(target_table_one.as_bytes().to_vec()).unwrap(),
            namespace: target_namespace.clone(),
        };
        let table2 = TableIdentifier {
            name: TableName::try_from(target_table_two.as_bytes().to_vec()).unwrap(),
            namespace: target_namespace.clone(),
        };
        let table3 = TableIdentifier {
            name: TableName::try_from("A_DIFFERENT_NAMESPACE".as_bytes().to_vec()).unwrap(),
            namespace: TableNamespace::try_from("OTHER_NAMESPACE".as_bytes().to_vec()).unwrap(),
        };
        let table_type = TableType::CoreBlockchain;
        let source = Source::Ethereum;

        // Create both tables
        let ddl1 = "CREATE TABLE IF NOT EXISTS TARGET_NAMESPACE.TABLE_ONE (ID BIGINT NOT NULL, PRIMARY KEY (ID));";
        let ddl2 = "CREATE TABLE IF NOT EXISTS TARGET_NAMESPACE.TABLE_TWO (ID BIGINT NOT NULL, PRIMARY KEY (ID));";
        let ddl3 = "CREATE TABLE IF NOT EXISTS OTHER_NAMESPACE.A_DIFFERENT_NAMESPACE (ID BIGINT NOT NULL, PRIMARY KEY (ID));";

        let create_statement1: CreateStatement =
            BoundedVec::try_from(ddl1.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        let create_statement2: CreateStatement =
            BoundedVec::try_from(ddl2.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        let create_statement3: CreateStatement =
            BoundedVec::try_from(ddl3.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![
            UpdateTable {
                ident: table1.clone(),
                create_statement: create_statement1,
                table_type: table_type.clone(),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                source: source.clone(),
            },
            UpdateTable {
                ident: table2.clone(),
                create_statement: create_statement2,
                table_type: table_type.clone(),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                source: source.clone(),
            },
            UpdateTable {
                ident: table3.clone(),
                create_statement: create_statement3,
                table_type: table_type.clone(),
                commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                source: source.clone(),
            }
        ])
        .expect("Table list should fit in BoundedVec");

        assert_ok!(Tables::create_tables(RuntimeOrigin::root(), tables));

        // Now that the tables are ready, let's test

        let new_quorum = InsertQuorumSize { public: None, privileged: Some(3)};

        assert_ok!(Tables::update_schema_quorum(RuntimeOrigin::root(), target_namespace, new_quorum));

        let old_quorum = InsertQuorumSize::from(table_type);

        System::assert_has_event(RuntimeEvent::Tables(Event::QuorumUpdated { table: table1.clone(), old_quorum: Some(old_quorum), new_quorum }));
        System::assert_has_event(RuntimeEvent::Tables(Event::QuorumUpdated { table: table2.clone(), old_quorum: Some(old_quorum), new_quorum }));

        // Make sure that table1 and table2 are updated to the new qourum
        assert_eq!(TableInsertQuorums::<Test>::get(table1), new_quorum);
        assert_eq!(TableInsertQuorums::<Test>::get(table2), new_quorum);

        // Also make sure that the other table in the other namespace was unaffected
        assert_eq!(TableInsertQuorums::<Test>::get(table3), old_quorum);
    });
}

#[test]
fn creating_community_table_fails_if_wrong_namespace_given() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (_, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked(
            "MY_COMMUNITY_TABLE",
            "FUNNAME_SOMEOTHERNAMESPACEATHATSWRONG",
        );
        let ddl = "CREATE TABLE IF NOT EXISTS FUNNAME_SOMEOTHERNAMESPACEATHATSWRONG.MY_COMMUNITY_TABLE (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
            ident: test_identifier.clone(),
            create_statement: create_statement.clone(),
            table_type: TableType::Community,
            commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
            source: Source::Ethereum,
        }])
        .expect("Table list should fit in BoundedVec");        

        System::reset_events();
        assert_err!(Tables::create_tables(signer, tables.clone()), Error::<Test>::InvalidNamespace);
    });
}

#[test]
fn creating_community_namespace_fails_if_wrong_namespace_given() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (account, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked(
            "MY_COMMUNITY_TABLE",
            "FUNNAME_SOMEOTHERNAMESPACEATHATSWRONG",
        );
        let ddl = "CREATE SCHEMA IF NOT EXISTS FUNNAME_SOMEOTHERNAMESPACEATHATSWRONG;";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &account,
            crate::CREATE_COST * 100,
        ));

        System::reset_events();
        assert_err!(
            Tables::create_namespace(
                signer,
                test_identifier.namespace,
                1,
                create_statement,
                TableType::Community,
                Source::UserCreated("ASDF".as_bytes().to_vec().try_into().unwrap()),
            ),
            Error::<Test>::InvalidNamespace
        );
    });
}

#[test]
fn creating_public_permissionless_namespace_fails_if_wrong_namespace_given() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (account, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked(
            "MY_COMMUNITY_TABLE",
            "FUNNAME_SOMEOTHERNAMESPACEATHATSWRONG",
        );
        let ddl = "CREATE SCHEMA IF NOT EXISTS FUNNAME_SOMEOTHERNAMESPACEATHATSWRONG;";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &account,
            crate::CREATE_COST * 100,
        ));

        System::reset_events();
        assert_err!(
            Tables::create_namespace(
                signer,
                test_identifier.namespace,
                1,
                create_statement,
                TableType::PublicPermissionless,
                Source::UserCreated("ASDF".as_bytes().to_vec().try_into().unwrap()),
            ),
            Error::<Test>::InvalidNamespace
        );
    });
}

#[test]
fn creating_community_namespace_works_with_valid_namespace() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (account, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked(
            "VOTES",
            "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
        );
        let ddl =
            "CREATE SCHEMA IF NOT EXISTS EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT;";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        let test_source = Source::UserCreated("ASDF".as_bytes().to_vec().try_into().unwrap());

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &account,
            crate::CREATE_COST * 100,
        ));

        System::reset_events();
        assert_ok!(Tables::create_namespace(
            signer,
            test_identifier.namespace,
            1,
            create_statement.clone(),
            TableType::Community,
            test_source.clone(),
        ),);

        assert_eq!(
            pallet_balances::Pallet::<Test>::free_balance(&account),
            crate::CREATE_COST * 99
        );

        // Make sure the event was emitted as expected
        System::assert_has_event(RuntimeEvent::Tables(Event::NamespaceCreated {
            create_schema: create_statement,
            version: 1,
            namespace_uuid: "E14F48497ECCA646C05217FF10D72A43"
                .as_bytes()
                .to_vec()
                .try_into()
                .unwrap(),
            table_type: TableType::Community,
            source: test_source,
        }));
    });
}
#[test]
fn creating_community_namespace_with_insufficient_funds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (account, signer) = user(1);
        let test_identifier = TableIdentifier::from_str_unchecked(
            "VOTES",
            "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
        );
        let ddl =
            "CREATE SCHEMA IF NOT EXISTS EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT;";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        let test_source = Source::UserCreated("ASDF".as_bytes().to_vec().try_into().unwrap());

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &account,
            crate::CREATE_COST / 2,
        ));

        assert_err!(
            Tables::create_namespace(
                signer,
                test_identifier.namespace,
                1,
                create_statement.clone(),
                TableType::Community,
                test_source.clone(),
            ),
            TokenError::FundsUnavailable
        );
    });
}

#[test]
fn creating_non_public_namespace_fails_if_not_sudo() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (account, signer) = user(1);
        let test_schema: ByteString = "SXT_SYSTEM_STAKING".as_bytes().to_vec().try_into().unwrap();
        let ddl = "CREATE SCHEMA IF NOT EXISTS SXT_SYSTEM_STAKING;";
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        let test_source = Source::Ethereum;

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &account,
            crate::CREATE_COST * 100,
        ));

        System::reset_events();
        assert_err!(
            Tables::create_namespace(
                signer.clone(),
                test_schema.clone(),
                1,
                create_statement.clone(),
                TableType::CoreBlockchain,
                test_source.clone(),
            ),
            pallet_permissions::Error::<Test>::InsufficientPermissions
        );

        assert_err!(
            Tables::create_namespace(
                signer.clone(),
                test_schema.clone(),
                1,
                create_statement.clone(),
                TableType::Testing(InsertQuorumSize {
                    public: Some(0),
                    privileged: Some(0)
                }),
                test_source.clone(),
            ),
            pallet_permissions::Error::<Test>::InsufficientPermissions
        );

        for e in System::events() {
            if let RuntimeEvent::Tables(Event::NamespaceCreated { .. }) = e.event {
                panic!("Namespace created event was emitted!")
            }
        }
    });
}

#[test]
fn creating_sci_namespace_works_without_special_permissions() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let (account, signer) = user(1);
        let ddl =
            "CREATE SCHEMA IF NOT EXISTS SCI_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT;";
        let schema_name: ByteString = "SCI_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT"
            .as_bytes()
            .to_vec()
            .try_into()
            .unwrap();
        let create_statement: CreateStatement =
            BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL should fit in BoundedVec");
        let test_source = Source::Ethereum;

        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &account,
            crate::CREATE_COST * 10,
        ));

        assert_ok!(Tables::create_namespace(
            signer,
            schema_name,
            1,
            create_statement,
            TableType::SCI,
            test_source,
        ));
    });
}

#[test]
fn create_tables_should_fail_when_there_are_too_many_tables_in_a_namespace() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Tables::create_namespace(
            RuntimeOrigin::root(),
            b"TEST_NAMESPACE".to_vec().try_into().unwrap(),
            0,
            b"CREATE SCHEMA IF NOT EXISTS TEST_NAMESPACE"
                .to_vec()
                .try_into()
                .unwrap(),
            TableType::CoreBlockchain,
            Source::Ethereum,
        ));

        assert_ok!(Tables::create_tables(
            RuntimeOrigin::root(),
            (0..sxt_core::tables::MAX_TABLES_PER_SCHEMA)
                .map(|i| {
                    let table_name = format!("TABLE_{}", i);
                    let ident = TableIdentifier::from_str_unchecked(&table_name, "TEST_NAMESPACE");
                    let ddl = format!(
                        "CREATE TABLE IF NOT EXISTS TEST_NAMESPACE.{} (A BIGINT NOT NULL)",
                        table_name
                    );

                    UpdateTable {
                        ident,
                        create_statement: ddl.as_bytes().to_vec().try_into().unwrap(),
                        table_type: TableType::CoreBlockchain,
                        commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                        source: Source::Ethereum,
                    }
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        ));
        assert_err!(
            Tables::create_tables(
                RuntimeOrigin::root(),
                vec![UpdateTable {
                    ident: TableIdentifier::from_str_unchecked("TEST_TABLE", "TEST_NAMESPACE"),
                    create_statement:
                        "CREATE TABLE IF NOT EXISTS TEST_NAMESPACE.TEST_TABLE (A BIGINT NOT NULL)"
                            .as_bytes()
                            .to_vec()
                            .try_into()
                            .unwrap(),
                    table_type: TableType::CoreBlockchain,
                    commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
                    source: Source::Ethereum,
                }]
                .try_into()
                .unwrap(),
            ),
            DispatchError::Other("Max tables per schema exceeded")
        );
    })
}

#[test]
fn setting_block_enforcement_works() {
    new_test_ext().execute_with(|| {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"TEST_NAMESPACE".to_vec()).unwrap(),
            name: TableName::try_from(b"TEST_TABLE".to_vec()).unwrap(),
        };

        assert!(Tables::block_enforcement(&table_id).is_none());

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(crate::pallet::BlockEnforcementMode::Increasing),
        ));

        assert_eq!(
            Tables::block_enforcement(&table_id),
            Some(crate::pallet::BlockEnforcementMode::Increasing),
        );
    });
}

#[test]
fn clearing_block_enforcement_works() {
    new_test_ext().execute_with(|| {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"TEST_NAMESPACE".to_vec()).unwrap(),
            name: TableName::try_from(b"TEST_TABLE".to_vec()).unwrap(),
        };

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            Some(crate::pallet::BlockEnforcementMode::Increasing),
        ));
        assert!(Tables::block_enforcement(&table_id).is_some());

        assert_ok!(Tables::set_block_enforcement(
            RuntimeOrigin::root(),
            table_id.clone(),
            None,
        ));
        assert!(Tables::block_enforcement(&table_id).is_none());
    });
}

#[test]
fn set_block_enforcement_fails_for_non_sudo() {
    new_test_ext().execute_with(|| {
        let table_id = TableIdentifier {
            namespace: TableNamespace::try_from(b"TEST_NAMESPACE".to_vec()).unwrap(),
            name: TableName::try_from(b"TEST_TABLE".to_vec()).unwrap(),
        };

        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));
        assert_err!(
            Tables::set_block_enforcement(
                signer,
                table_id,
                Some(crate::pallet::BlockEnforcementMode::Increasing),
            ),
            sp_runtime::DispatchError::BadOrigin,
        );
    });
}

#[test]
fn set_table_metadata_sets_value() {
    new_test_ext().execute_with(|| {
        let domain: ByteString = b"test_domain".to_vec().try_into().unwrap();
        let table = TableIdentifier {
            namespace: TableNamespace::try_from(b"SCI_SCHEMA".to_vec()).unwrap(),
            name: TableName::try_from(b"SCI_TABLE".to_vec()).unwrap(),
        };
        let metadata: TableMetadataBytes = b"some metadata bytes".to_vec().try_into().unwrap();

        assert_ok!(Tables::set_table_metadata(
            RuntimeOrigin::root(),
            domain.clone(),
            table.clone(),
            Some(metadata.clone()),
        ));
        assert_eq!(TableMetadata::<Test>::get(&domain, &table), Some(metadata));
    });
}

#[test]
fn set_table_metadata_replaces_existing_value() {
    new_test_ext().execute_with(|| {
        let domain: ByteString = b"test_domain".to_vec().try_into().unwrap();
        let table = TableIdentifier {
            namespace: TableNamespace::try_from(b"SCI_SCHEMA".to_vec()).unwrap(),
            name: TableName::try_from(b"SCI_TABLE".to_vec()).unwrap(),
        };
        let metadata1: TableMetadataBytes = b"first metadata".to_vec().try_into().unwrap();
        let metadata2: TableMetadataBytes = b"second metadata".to_vec().try_into().unwrap();

        assert_ok!(Tables::set_table_metadata(
            RuntimeOrigin::root(),
            domain.clone(),
            table.clone(),
            Some(metadata1),
        ));
        assert_ok!(Tables::set_table_metadata(
            RuntimeOrigin::root(),
            domain.clone(),
            table.clone(),
            Some(metadata2.clone()),
        ));
        assert_eq!(TableMetadata::<Test>::get(&domain, &table), Some(metadata2));
    });
}

#[test]
fn set_table_metadata_clears_entry_when_none() {
    new_test_ext().execute_with(|| {
        let domain: ByteString = b"test_domain".to_vec().try_into().unwrap();
        let table = TableIdentifier {
            namespace: TableNamespace::try_from(b"SCI_SCHEMA".to_vec()).unwrap(),
            name: TableName::try_from(b"SCI_TABLE".to_vec()).unwrap(),
        };
        let metadata: TableMetadataBytes = b"some metadata bytes".to_vec().try_into().unwrap();

        assert_ok!(Tables::set_table_metadata(
            RuntimeOrigin::root(),
            domain.clone(),
            table.clone(),
            Some(metadata),
        ));
        assert_ok!(Tables::set_table_metadata(
            RuntimeOrigin::root(),
            domain.clone(),
            table.clone(),
            None,
        ));
        assert_eq!(TableMetadata::<Test>::get(&domain, &table), None);
    });
}

#[test]
fn set_table_metadata_fails_for_non_sudo() {
    new_test_ext().execute_with(|| {
        let domain: ByteString = b"test_domain".to_vec().try_into().unwrap();
        let table = TableIdentifier {
            namespace: TableNamespace::try_from(b"SCI_SCHEMA".to_vec()).unwrap(),
            name: TableName::try_from(b"SCI_TABLE".to_vec()).unwrap(),
        };
        let metadata: TableMetadataBytes = b"some metadata bytes".to_vec().try_into().unwrap();
        let signer = RuntimeOrigin::signed(sp_runtime::AccountId32::new([1; 32]));

        assert_err!(
            Tables::set_table_metadata(signer, domain, table, Some(metadata)),
            sp_runtime::DispatchError::BadOrigin,
        );
    });
}

// ---------------------------------------------------------------------------
// create_table_with_sci_metadata
// ---------------------------------------------------------------------------

fn make_sci_table(namespace: &str, table_name: &str) -> CreateSciTableRequest {
    let ident = TableIdentifier::from_str_unchecked_with_preserved_casing(table_name, namespace);
    let ddl = format!(
        "CREATE TABLE IF NOT EXISTS {}.{} (BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));",
        namespace, table_name
    );
    let create_statement: CreateStatement =
        BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("DDL fits");
    CreateSciTableRequest {
        ident,
        create_statement,
        source: Source::Ethereum,
        commitment_scheme_flags: CommitmentSchemeFlags::default(),
    }
}

#[test]
fn create_table_with_sci_metadata_stores_metadata_and_sets_contiguous_enforcement() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        let table = make_sci_table("ETHEREUM", "SCI_TEST");
        let expected_ident = table.ident.clone().try_normalize().unwrap();
        let metadata: TableMetadataBytes =
            BoundedVec::try_from(b"raw metadata bytes".to_vec()).unwrap();

        assert_ok!(Tables::create_table_with_sci_metadata(
            RuntimeOrigin::root(),
            table,
            metadata.clone(),
        ));

        // Metadata stored under the normalised identifier with the "sci" domain.
        let sci_domain: ByteString = crate::pallet::SCI_METADATA_DOMAIN
            .to_vec()
            .try_into()
            .unwrap();
        assert_eq!(
            TableMetadata::<Test>::get(&sci_domain, &expected_ident),
            Some(metadata)
        );

        // Block enforcement set to Contiguous.
        assert_eq!(
            Tables::block_enforcement(&expected_ident),
            Some(crate::pallet::BlockEnforcementMode::Contiguous),
        );

        // SciTableCreated event emitted.
        System::assert_has_event(
            Event::<Test>::SciTableCreated {
                table: expected_ident,
            }
            .into(),
        );
    });
}

#[test]
fn create_table_with_sci_metadata_works_when_permissioned() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        let (who, signer) = user(1);
        set_permission!(who, TablesPalletPermission::EditSchema);

        let table = make_sci_table("ETHEREUM", "SCI_PERM");
        let metadata: TableMetadataBytes = BoundedVec::try_from(b"perm metadata".to_vec()).unwrap();

        assert_ok!(Tables::create_table_with_sci_metadata(
            signer, table, metadata
        ));
    });
}

#[test]
fn create_table_with_sci_metadata_fails_when_namespace_does_not_exist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let table = make_sci_table("NO_SUCH_NS", "SCI_MISSING");
        let metadata: TableMetadataBytes = BoundedVec::try_from(b"meta".to_vec()).unwrap();

        assert!(
            Tables::create_table_with_sci_metadata(RuntimeOrigin::root(), table, metadata).is_err()
        );
    });
}

#[test]
fn create_table_with_sci_metadata_fails_for_unpermissioned_user_with_incorrect_namespace() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        let (_who, signer) = user(2);
        let table = make_sci_table("ETHEREUM", "SCI_NOPERM");
        let metadata: TableMetadataBytes = BoundedVec::try_from(b"meta".to_vec()).unwrap();

        assert_err!(
            Tables::create_table_with_sci_metadata(signer, table, metadata),
            crate::Error::<Test>::InvalidNamespace,
        );
    });
}

#[test]
fn sci_table_succeeds_for_unpermissioned_user_in_owned_namespace() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("SCI_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");

        let (who, signer) = user(1);
        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &who,
            crate::CREATE_COST * 10,
        ));

        let table = make_sci_table(
            "SCI_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
            "MY_TABLE",
        );
        let metadata: TableMetadataBytes = BoundedVec::try_from(b"meta".to_vec()).unwrap();

        assert_ok!(Tables::create_table_with_sci_metadata(
            signer, table, metadata
        ));
    });
}

#[test]
fn update_table_quorum_succeeds_with_per_table_permission() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let namespace = "EXAMPLE_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT";
        create_namespace_for_testing(namespace);

        let (who, signer) = user(1);
        assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
            &who,
            crate::CREATE_COST * 10,
        ));
        let table_identifier = create_community_table_for_user(signer, namespace, "MY_TABLE");

        // The table owner should automatically receive UpdateTableQuorum for their table
        let quorum_permission = PermissionLevel::TablesPallet(
            TablesPalletPermission::UpdateTableQuorum(table_identifier.clone()),
        );
        assert!(pallet_permissions::Pallet::<Test>::has_permissions(
            &who,
            &quorum_permission
        ));

        let new_quorum = InsertQuorumSize {
            public: None,
            privileged: Some(5),
        };
        assert_ok!(Tables::update_table_quorum(
            RuntimeOrigin::signed(who),
            table_identifier,
            new_quorum,
        ));
    });
}

#[test]
fn update_table_quorum_succeeds_with_edit_schema_permission() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        let (who, signer) = user(1);
        set_permission!(who.clone(), TablesPalletPermission::EditSchema);

        assert_ok!(Tables::create_tables(signer, test_tables()));

        let table_identifier = TableIdentifier {
            name: b"BLOCKS".to_vec().try_into().unwrap(),
            namespace: b"ETHEREUM".to_vec().try_into().unwrap(),
        };

        let new_quorum = InsertQuorumSize {
            public: None,
            privileged: Some(3),
        };
        assert_ok!(Tables::update_table_quorum(
            RuntimeOrigin::signed(who),
            table_identifier,
            new_quorum,
        ));
    });
}

#[test]
fn update_table_quorum_fails_without_any_permission() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("ETHEREUM");

        assert_ok!(Tables::create_tables(RuntimeOrigin::root(), test_tables()));

        let table_identifier = TableIdentifier {
            name: b"BLOCKS".to_vec().try_into().unwrap(),
            namespace: b"ETHEREUM".to_vec().try_into().unwrap(),
        };

        let new_quorum = InsertQuorumSize {
            public: None,
            privileged: Some(3),
        };
        assert_err!(
            Tables::update_table_quorum(user(99).1, table_identifier, new_quorum,),
            pallet_permissions::Error::<Test>::InsufficientPermissions
        );
    });
}

/// Helper: create a community table owned by `user(1)` in the test namespace.
/// Returns the normalised `TableIdentifier`.
fn create_community_table_for_user1() -> TableIdentifier {
    let (who, signer) = user(1);
    let test_identifier = TableIdentifier::from_str_unchecked_with_preserved_casing(
        "VOTES",
        "FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT",
    );
    let ddl = "CREATE TABLE IF NOT EXISTS FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT.VOTES (TIME_STAMP TIMESTAMP NOT NULL, BLOCK_NUMBER BIGINT NOT NULL, PRIMARY KEY (BLOCK_NUMBER));";
    let create_statement: CreateStatement =
        BoundedVec::try_from(ddl.as_bytes().to_vec()).expect("fits");
    let tables: UpdateTableList = BoundedVec::try_from(vec![UpdateTable {
        ident: test_identifier.clone(),
        create_statement,
        table_type: TableType::Community,
        commitment: CommitmentCreationCmd::Empty(CommitmentSchemeFlags::default()),
        source: Source::Ethereum,
    }])
    .expect("fits");
    assert_ok!(pallet_balances::Pallet::<Test>::mint_into(
        &who,
        crate::CREATE_COST * 10,
    ));
    assert_ok!(Tables::create_tables(signer, tables));
    test_identifier.try_normalize().unwrap()
}

#[test]
fn transfer_table_ownership_by_owner_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");

        let (who1, signer1) = user(1);
        let (who2, _) = user(2);
        let ident = create_community_table_for_user1();
        assert_eq!(TableOwners::<Test>::get(&ident), Some(who1.clone()));

        System::reset_events();
        assert_ok!(Tables::transfer_table_ownership(
            signer1,
            ident.clone(),
            Some(who2.clone()),
        ));

        assert_eq!(TableOwners::<Test>::get(&ident), Some(who2.clone()));
        System::assert_has_event(RuntimeEvent::Tables(Event::TableOwnershipTransferred {
            table: ident,
            old_owner: Some(who1),
            new_owner: Some(who2),
        }));
    });
}

#[test]
fn transfer_table_ownership_by_sudo_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");

        let (who1, _) = user(1);
        let (who2, _) = user(2);
        let ident = create_community_table_for_user1();
        assert_eq!(TableOwners::<Test>::get(&ident), Some(who1.clone()));

        assert_ok!(Tables::transfer_table_ownership(
            RuntimeOrigin::root(),
            ident.clone(),
            Some(who2.clone()),
        ));

        assert_eq!(TableOwners::<Test>::get(&ident), Some(who2));
    });
}

#[test]
fn transfer_table_ownership_fails_for_non_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");

        let ident = create_community_table_for_user1();
        let (who2, signer2) = user(2);

        assert_err!(
            Tables::transfer_table_ownership(signer2, ident, Some(who2)),
            pallet_permissions::Error::<Test>::InsufficientPermissions,
        );
    });
}

#[test]
fn transfer_table_ownership_to_none_removes_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        create_namespace_for_testing("FUNNAME_5C62Ck4UrFPiBtoCmeSrgF7x9yv9mn38446dhCpsi2mLHiFT");

        let (who1, signer1) = user(1);
        let ident = create_community_table_for_user1();
        assert_eq!(TableOwners::<Test>::get(&ident), Some(who1.clone()));

        System::reset_events();
        assert_ok!(Tables::transfer_table_ownership(
            signer1,
            ident.clone(),
            None,
        ));

        assert_eq!(TableOwners::<Test>::get(&ident), None);
        System::assert_has_event(RuntimeEvent::Tables(Event::TableOwnershipTransferred {
            table: ident,
            old_owner: Some(who1),
            new_owner: None,
        }));
    });
}

#[test]
fn transfer_table_ownership_fails_when_no_owner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let table = TableIdentifier::from_str_unchecked("MY_TABLE", "TEST_NAMESPACE");
        let (who, signer) = user(1);

        assert_err!(
            Tables::transfer_table_ownership(signer, table, Some(who)),
            pallet_permissions::Error::<Test>::InsufficientPermissions,
        );
    });
}
