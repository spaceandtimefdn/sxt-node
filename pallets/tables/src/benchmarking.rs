//! Benchmarking setup for pallet-tables
#![cfg(feature = "runtime-benchmarks")]
use alloc::vec::Vec;
use alloc::{format, vec};

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use proof_of_sql_commitment_map::CommitmentSchemeFlags;
use sxt_core::permissions::{PermissionLevel, TablesPalletPermission};
use sxt_core::tables::{CreateStatement, Source, TableIdentifier, TableNamespace, TableType};
use sxt_core::ByteString;

use super::*;
#[allow(unused)]
use crate::Pallet as Tables;

/// Grants the given account the `EditSchema` permission.
pub fn grant_edit_schema<T: pallet_permissions::Config>(account: T::AccountId) {
    pallet_permissions::Pallet::<T>::add_proxy_permission(
        RawOrigin::Root.into(),
        account,
        PermissionLevel::TablesPallet(TablesPalletPermission::EditSchema),
    )
    .unwrap();
}

/// Grants the given account the `EditUuid` permission.
fn grant_edit_uuid<T: pallet_permissions::Config>(account: T::AccountId) {
    pallet_permissions::Pallet::<T>::add_proxy_permission(
        RawOrigin::Root.into(),
        account,
        PermissionLevel::TablesPallet(TablesPalletPermission::EditUuid),
    )
    .unwrap();
}

/// Returns some inputs that can be used to create a schema with the given name.
pub fn schema_bytes_and_ddl_and_source(
    schema_name: impl AsRef<str>,
) -> (TableNamespace, CreateStatement, Source) {
    let schema_name_bytes: ByteString =
        schema_name.as_ref().as_bytes().to_vec().try_into().unwrap();
    let create_statement = format!("CREATE SCHEMA IF NOT EXISTS {};", schema_name.as_ref())
        .into_bytes()
        .try_into()
        .unwrap();
    let source = Source::UserCreated(b"benchmark".to_vec().try_into().unwrap());

    (schema_name_bytes, create_statement, source)
}

/// Returns an `UpdateTable` for creating a table with the given ident/type.
///
/// The table will have 64 integer columns.
pub fn integers_table_definition(
    ident: TableIdentifier,
    table_type: TableType,
    commitment_schemes: CommitmentSchemeFlags,
) -> UpdateTable {
    let create_statement_columns = (0..64)
        .map(|col_num| alloc::format!("COL_{col_num} BIGINT NOT NULL"))
        .collect::<alloc::vec::Vec<_>>()
        .join(", ");

    let create_statement_table_identifier = format!(
        "{}.{}",
        core::str::from_utf8(ident.namespace.as_slice()).unwrap(),
        core::str::from_utf8(ident.name.as_slice()).unwrap()
    );

    let create_statement = alloc::format!(
        "CREATE TABLE {create_statement_table_identifier} ({create_statement_columns})"
    )
    .as_bytes()
    .to_vec()
    .try_into()
    .unwrap();

    let commitment = CommitmentCreationCmd::Empty(commitment_schemes);

    let source = Source::UserCreated(b"benchmark".to_vec().try_into().unwrap());

    UpdateTable {
        ident,
        create_statement,
        table_type,
        commitment,
        source,
    }
}

#[benchmarks(
    where
        <T as frame_system::Config>::AccountId: Ss58Codec,
)]
mod benchmarks {
    use sp_core::crypto::Ss58Codec;
    use sxt_core::tables::{InsertQuorumSize, TableUuid, MAX_TABLES_PER_SCHEMA};

    use super::*;

    #[benchmark]
    fn create_zero_tables() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());

        #[extrinsic_call]
        Tables::<T>::create_tables(RawOrigin::Signed(creator), vec![].try_into().unwrap());
    }

    #[benchmark]
    fn create_one_table() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());

        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("SCHEMA");

        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator.clone()).into(),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        )
        .unwrap();

        let existing_tables = (0..(MAX_TABLES_PER_SCHEMA - 1))
            .map(|i| {
                let table_identifier =
                    TableIdentifier::from_str_unchecked(&format!("EXISTING{}", i), "SCHEMA");
                integers_table_definition(
                    table_identifier.clone(),
                    TableType::Community,
                    CommitmentSchemeFlags::all(),
                )
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Tables::<T>::create_tables(RawOrigin::Signed(creator.clone()).into(), existing_tables)
            .unwrap();

        let table_identifier = TableIdentifier::from_str_unchecked("ONE", "SCHEMA");

        let update_tables = vec![integers_table_definition(
            table_identifier.clone(),
            TableType::Community,
            CommitmentSchemeFlags::all(),
        )]
        .try_into()
        .unwrap();

        #[extrinsic_call]
        Tables::<T>::create_tables(RawOrigin::Signed(creator), update_tables);

        assert!(TableVersions::<T>::contains_key(&table_identifier, 0));
    }

    #[benchmark]
    fn clear_tables() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());

        (0..500u32.div_ceil(MAX_TABLES_PER_SCHEMA))
            .map(|i| schema_bytes_and_ddl_and_source(format!("SCHEMA{}", i)))
            .for_each(|(schema_name_bytes, create_statement, source)| {
                Tables::<T>::create_namespace(
                    RawOrigin::Signed(creator.clone()).into(),
                    schema_name_bytes.clone(),
                    0,
                    create_statement,
                    TableType::Community,
                    source,
                )
                .unwrap();
            });

        let update_tables = (0..500)
            .map(|i| {
                let table_identifier = TableIdentifier::from_str_unchecked(
                    &format!("TABLE{}", i % MAX_TABLES_PER_SCHEMA),
                    &format!("SCHEMA{}", i / MAX_TABLES_PER_SCHEMA),
                );
                integers_table_definition(
                    table_identifier.clone(),
                    TableType::Community,
                    CommitmentSchemeFlags::all(),
                )
            })
            .collect::<Vec<_>>();

        Tables::<T>::create_tables(
            RawOrigin::Signed(creator).into(),
            update_tables.try_into().unwrap(),
        )
        .unwrap();
        assert_eq!(Schemas::<T>::iter().count(), 500);
        assert_eq!(TableInsertQuorums::<T>::iter().count(), 500);
        assert_eq!(
            pallet_commitments::CommitmentStorageMap::<T>::iter().count(),
            1000
        );

        #[extrinsic_call]
        Tables::<T>::clear_tables(RawOrigin::Root);

        assert_eq!(Schemas::<T>::iter().count(), 0);
        assert_eq!(TableInsertQuorums::<T>::iter().count(), 0);
        assert_eq!(
            pallet_commitments::CommitmentStorageMap::<T>::iter().count(),
            0
        );
    }

    #[benchmark]
    fn create_namespace() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());

        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("MY_SCHEMA");

        #[extrinsic_call]
        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        );

        assert!(NamespaceVersions::<T>::contains_key(schema_name_bytes, 0));
    }

    #[benchmark]
    fn drop_table() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());

        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("SCHEMA");

        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator.clone()).into(),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        )
        .unwrap();
        let table_identifier = TableIdentifier::from_str_unchecked("DROP", "SCHEMA");
        let table_type = TableType::Community;

        let table_definition = integers_table_definition(
            table_identifier.clone(),
            table_type.clone(),
            CommitmentSchemeFlags::all(),
        );

        Tables::<T>::create_tables(
            RawOrigin::Signed(creator.clone()).into(),
            vec![table_definition.clone()].try_into().unwrap(),
        )
        .unwrap();

        assert!(Schemas::<T>::contains_key(
            &table_identifier.namespace,
            &table_identifier.name,
        ));

        #[extrinsic_call]
        Tables::<T>::drop_table(
            RawOrigin::Signed(creator),
            table_type,
            table_identifier.clone(),
            table_definition.source,
        );

        assert!(!Schemas::<T>::contains_key(
            &table_identifier.namespace,
            &table_identifier.name,
        ));
    }

    #[benchmark]
    fn drop_invalid_commits() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());
        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("SCHEMA");

        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator.clone()).into(),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        )
        .unwrap();

        let table_identifier = TableIdentifier::from_str_unchecked("DROP", "SCHEMA");
        let table_type = TableType::Community;

        let table_definition = integers_table_definition(
            table_identifier.clone(),
            table_type.clone(),
            CommitmentSchemeFlags::all(),
        );

        Tables::<T>::create_tables(
            RawOrigin::Signed(creator.clone()).into(),
            vec![table_definition.clone()].try_into().unwrap(),
        )
        .unwrap();

        CommitmentSchemeFlags::all()
            .into_iter()
            .for_each(|commitment_scheme| {
                assert!(pallet_commitments::CommitmentStorageMap::<T>::contains_key(
                    &table_identifier,
                    commitment_scheme
                ));
            });

        #[extrinsic_call]
        Tables::<T>::drop_invalid_commits(RawOrigin::Root, table_identifier.clone());

        CommitmentSchemeFlags::all()
            .into_iter()
            .for_each(|commitment_scheme| {
                assert!(
                    !pallet_commitments::CommitmentStorageMap::<T>::contains_key(
                        &table_identifier,
                        commitment_scheme
                    )
                );
            });
    }

    #[benchmark]
    fn update_namespace_uuid() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());
        grant_edit_uuid::<T>(creator.clone());

        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("MY_SCHEMA");

        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator.clone()).into(),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        )
        .unwrap();

        let new_uuid =
            TableUuid::try_from("F801A872785FAB3F16C51CF7A1969000".as_bytes().to_vec()).unwrap();

        assert_ne!(NamespaceVersions::<T>::get(&schema_name_bytes, 0), new_uuid);
        assert_ne!(NamespaceVersions::<T>::get(&schema_name_bytes, 1), new_uuid);

        // Call extrinsic
        #[extrinsic_call]
        Tables::update_namespace_uuid(
            RawOrigin::Signed(creator),
            schema_name_bytes.clone(),
            1,
            new_uuid.clone(),
        );

        // Check storage
        assert_ne!(NamespaceVersions::<T>::get(&schema_name_bytes, 0), new_uuid);
        assert_eq!(NamespaceVersions::<T>::get(&schema_name_bytes, 1), new_uuid);
    }

    #[benchmark]
    fn update_table_uuid() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());
        grant_edit_uuid::<T>(creator.clone());

        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("SCHEMA");

        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator.clone()).into(),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        )
        .unwrap();

        let table_identifier = TableIdentifier::from_str_unchecked("UUID", "SCHEMA");
        let table_type = TableType::Community;

        let table_definition = integers_table_definition(
            table_identifier.clone(),
            table_type.clone(),
            CommitmentSchemeFlags::all(),
        );

        Tables::<T>::create_tables(
            RawOrigin::Signed(creator.clone()).into(),
            vec![table_definition.clone()].try_into().unwrap(),
        )
        .unwrap();

        let new_uuid =
            TableUuid::try_from("F801A872785FAB3F16C51CF7A1969000".as_bytes().to_vec()).unwrap();

        assert_ne!(TableVersions::<T>::get(&table_identifier, 0), new_uuid);
        assert_ne!(TableVersions::<T>::get(&table_identifier, 1), new_uuid);

        // Call extrinsic
        #[extrinsic_call]
        Tables::update_table_uuid(
            RawOrigin::Signed(creator),
            table_identifier.clone(),
            1,
            new_uuid.clone(),
        );

        // Check storage
        assert_ne!(TableVersions::<T>::get(&table_identifier, 0), new_uuid);
        assert_eq!(TableVersions::<T>::get(&table_identifier, 1), new_uuid);
    }

    #[benchmark]
    fn update_table_quorum() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());
        grant_edit_uuid::<T>(creator.clone());

        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("MY_SCHEMA");

        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator.clone()).into(),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        )
        .unwrap();

        let table_identifier = TableIdentifier::from_str_unchecked("QUORUM", "MY_SCHEMA");
        let table_type = TableType::Community;

        let table_definition = integers_table_definition(
            table_identifier.clone(),
            table_type.clone(),
            CommitmentSchemeFlags::all(),
        );

        Tables::<T>::create_tables(
            RawOrigin::Signed(creator.clone()).into(),
            vec![table_definition.clone()].try_into().unwrap(),
        )
        .unwrap();

        let new_insert_quorum_size = InsertQuorumSize {
            public: Some(10),
            privileged: Some(0),
        };

        assert_ne!(
            TableInsertQuorums::<T>::get(&table_identifier),
            new_insert_quorum_size
        );

        #[extrinsic_call]
        Tables::update_table_quorum(
            RawOrigin::Signed(creator),
            table_identifier.clone(),
            new_insert_quorum_size,
        );

        assert_eq!(
            TableInsertQuorums::<T>::get(&table_identifier),
            new_insert_quorum_size
        );
    }

    #[benchmark]
    fn update_schema_quorum() {
        let creator: T::AccountId = whitelisted_caller();
        grant_edit_schema::<T>(creator.clone());
        grant_edit_uuid::<T>(creator.clone());

        let (schema_name_bytes, create_statement, source) =
            schema_bytes_and_ddl_and_source("MY_SCHEMA");

        Tables::<T>::create_namespace(
            RawOrigin::Signed(creator.clone()).into(),
            schema_name_bytes.clone(),
            0,
            create_statement,
            TableType::Community,
            source,
        )
        .unwrap();

        let new_insert_quorum_size = InsertQuorumSize {
            public: Some(10),
            privileged: Some(0),
        };

        (0..MAX_TABLES_PER_SCHEMA).for_each(|i| {
            let table_identifier =
                TableIdentifier::from_str_unchecked(&format!("QUORUM{}", i), "MY_SCHEMA");
            let table_type = TableType::Community;

            let table_definition = integers_table_definition(
                table_identifier.clone(),
                table_type.clone(),
                CommitmentSchemeFlags::all(),
            );

            Tables::<T>::create_tables(
                RawOrigin::Signed(creator.clone()).into(),
                vec![table_definition.clone()].try_into().unwrap(),
            )
            .unwrap();

            assert_ne!(
                TableInsertQuorums::<T>::get(&table_identifier),
                new_insert_quorum_size
            );
        });

        #[extrinsic_call]
        Tables::update_schema_quorum(
            RawOrigin::Signed(creator),
            schema_name_bytes,
            new_insert_quorum_size,
        );

        (0..MAX_TABLES_PER_SCHEMA).for_each(|i| {
            let table_identifier =
                TableIdentifier::from_str_unchecked(&format!("QUORUM{}", i), "MY_SCHEMA");
            assert_eq!(
                TableInsertQuorums::<T>::get(&table_identifier),
                new_insert_quorum_size
            );
        })
    }

    impl_benchmark_test_suite!(Tables, crate::mock::new_test_ext(), crate::mock::Test);
}
