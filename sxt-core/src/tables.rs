use super::ByteString;
use alloc::vec::Vec;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{storage::bounded_vec::BoundedVec, traits::ConstU32};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sp_core::RuntimeDebug;
use sqlparser::ast::ObjectName;

/// Maxiumum number of columns per table
pub const MAX_COLS_PER_TABLE: u32 = 64;

/// Maximum number of tables per identifier
pub const MAX_TABLES_PER_SCHEMA: u32 = 1024;

/// The maximum length of a URL snapshot
pub const MAX_SNAPSHOT_LEN: u32 = 2048;

/// TODO: add docs
pub type MaxColsPerTable = ConstU32<MAX_COLS_PER_TABLE>;
/// TODO: add docs
pub type MaxTablesPerSchema = ConstU32<MAX_TABLES_PER_SCHEMA>;

/// List of possible chains that the transaction node supports.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default, Serialize, Deserialize)]
pub enum Source {
    /// Ethereum mainnet
    #[default]
    Ethereum,

    /// Bitcoin mainnet
    Bitcoin,

    /// Polygon mainnet
    Polygon,

    /// zkSyncEra
    ZkSyncEra,

    /// A user created source r
    UserCreated(ByteString),
}

/// The mode that the indexer supports
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default, Serialize, Deserialize)]
pub enum IndexerMode {
    #[default]
    /// TODO: add docs
    Core,
    /// TODO: add docs
    Full,
    /// TODO: add docs
    PriceFeeds,
    /// TODO: add docs
    SmartContract(ByteString),
    /// TODO: add docs
    UserCreated(ByteString),
}

/// A request for work from an indexer
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default, Serialize, Deserialize)]
pub struct SourceAndMode {
    /// TODO: add docs
    pub source: Source,
    /// TODO: add docs
    pub mode: IndexerMode,
}

/// Two megabytes
pub const FIVE_HUNDRED_KB: u32 = 500_000;

/// Arrow schema represented by an ipc buffer https://arrow.apache.org/rust/arrow_ipc/convert/fn.try_schema_from_ipc_buffer.html
/// This is what is stored in substrate.
pub type IPCSchema = BoundedVec<u8, ConstU32<FIVE_HUNDRED_KB>>;

/// TODO: add docs
pub type TableName = ByteString;
/// TODO: add docs
pub type TableNamespace = ByteString;

/// A unique identifier for a work assignment, a key that maps to the 'TableSchema'
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default, Serialize, Deserialize)]
pub struct TableIdentifier {
    /// TODO: add docs
    pub name: TableName,
    /// TODO: add docs
    pub namespace: TableNamespace,
}

/// Errors that can occur when converting a sqlparser ObjectName to a [`TableIdentifier`].
#[derive(Debug, Snafu, PartialEq, Eq)]
pub enum ObjectNameToTableIdentifierError {
    /// Table identifiers should contain two idents like `namespace.name`.
    #[snafu(display("table identifiers should contain two idents like 'namespace.name'"))]
    NotTwoIdentifiers,
    /// Identifier exceeds maximum length.
    #[snafu(display("identifier exceeds maximum length"))]
    IdentifierExceedsMaxLength,
}

impl TryFrom<&ObjectName> for TableIdentifier {
    type Error = ObjectNameToTableIdentifierError;

    fn try_from(value: &ObjectName) -> Result<Self, Self::Error> {
        let [namespace, name] = value
            .0
            .iter()
            .map(|ident| {
                ByteString::try_from(ident.value.as_bytes().to_vec())
                    .map_err(|_| ObjectNameToTableIdentifierError::IdentifierExceedsMaxLength)
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| ObjectNameToTableIdentifierError::NotTwoIdentifiers)?;

        Ok(TableIdentifier { namespace, name })
    }
}

/// Maximum primary keys for a table
// TODO find suitable values for both of these
pub const MAX_PRIMARY_KEYS: u32 = 32;
/// TODO: add docs
pub type PrimaryKey = ByteString;
/// TODO: add docs
pub type PrimaryKeys = BoundedVec<PrimaryKey, ConstU32<MAX_PRIMARY_KEYS>>;

/// Maximum foreign keys for a table
pub const MAX_FOREIGN_KEYS: u32 = 32;
/// TODO: add docs
pub type ForeignKey = ByteString;
/// TODO: add docs
pub type ForeignKeys = BoundedVec<ForeignKey, ConstU32<MAX_FOREIGN_KEYS>>;

/// TODO: add docs
pub const CREATE_STMNT_LENGTH: u32 = 8192;
/// TODO: add docs
pub type CreateStatement = BoundedVec<u8, ConstU32<CREATE_STMNT_LENGTH>>;

/// TODO: add docs
pub type CreateStatements = BoundedVec<CreateStatement, ConstU32<MAX_TABLES_PER_SCHEMA>>;

/// TODO: add docs
pub type UpdateTableCmd = (TableIdentifier, CreateStatement);
/// TODO: add docs
pub type UpdateTableList = BoundedVec<UpdateTableCmd, ConstU32<MAX_TABLES_PER_SCHEMA>>;

/// A url that points to a known snapshot of a table in storage
pub type SnapshotUrl = BoundedVec<u8, ConstU32<MAX_SNAPSHOT_LEN>>;

/// Create a table identifier from a name and namespace
///
/// This function does no checking of the lengths of name and namespace and will panic!
/// Use it only on known good values and never with user submitted data.
/// This should only be used in the creation of the genesis chain spec, that is a single atomic operation which must run end to end with no failures, which is why we are fine calling unwrap
#[cfg(feature = "std")]
pub fn table_identifier(name: &str, namespace: &str) -> TableIdentifier {
    TableIdentifier {
        name: TableName::try_from(String::from(name).as_bytes().to_vec()).unwrap(),
        namespace: TableNamespace::try_from(String::from(namespace).as_bytes().to_vec()).unwrap(),
    }
}

/// Create a CreateStatement from a &str. This can be combined with the include_str! macro to easily bring in tables from DDL file.
///
/// This function does no checking of the lengths of the data and will panic!
/// Use it only on known good values and never with user submitted data.
/// This should only be used in the creation of the genesis chain spec, that is a single atomic operation which must run end to end with no failures, which is why we are fine calling unwrap
#[cfg(feature = "std")]
pub fn create_statement(stmnt: &str) -> CreateStatement {
    CreateStatement::try_from(String::from(stmnt).as_bytes().to_vec()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{format, string::String};
    use sqlparser::{
        ast::helpers::stmt_create_table::CreateTableBuilder, dialect::PostgreSqlDialect,
        parser::Parser,
    };

    #[test]
    fn we_can_convert_object_name_to_table_identifier() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql("CREATE TABLE namespace.name ()")
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let table_identifier = TableIdentifier::try_from(&create_table.name).unwrap();
        let expected = TableIdentifier {
            namespace: b"namespace".to_vec().try_into().unwrap(),
            name: b"name".to_vec().try_into().unwrap(),
        };

        assert_eq!(table_identifier, expected);
    }

    #[test]
    fn we_cannot_convert_object_name_with_bad_ident_count_to_table_identifier() {
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql("CREATE TABLE database.namespace.name ()")
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(
            TableIdentifier::try_from(&create_table.name),
            Err(ObjectNameToTableIdentifierError::NotTwoIdentifiers)
        );

        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql("CREATE TABLE name ()")
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(
            TableIdentifier::try_from(&create_table.name),
            Err(ObjectNameToTableIdentifierError::NotTwoIdentifiers)
        );
    }

    #[test]
    fn we_cannot_convert_object_name_with_long_ident_to_table_identifier() {
        let long_name = String::from_iter(['a'; 65]);
        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&format!("CREATE TABLE namespace.{long_name} ()"))
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        assert_eq!(
            TableIdentifier::try_from(&create_table.name),
            Err(ObjectNameToTableIdentifierError::IdentifierExceedsMaxLength)
        );
    }
}
