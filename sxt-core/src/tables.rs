use alloc::string::ToString;
use alloc::vec::Vec;
use core::str::{from_utf8, Utf8Error};

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::storage::bounded_vec::BoundedVec;
use frame_support::traits::ConstU32;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sp_core::RuntimeDebug;
use sp_runtime_interface::pass_by::PassByCodec;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::ObjectName;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use super::ByteString;

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
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
)]
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
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
)]
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
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
)]
pub struct SourceAndMode {
    /// TODO: add docs
    pub source: Source,
    /// TODO: add docs
    pub mode: IndexerMode,
}

/// An object to contain everything needed to create a table at genesis
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
)]
pub struct GenesisTable {
    /// A DDL Statement to create the table
    pub statement: CreateStatement,
    /// The Base URL of the snapshot
    pub url: SnapshotUrl,
    /// The name and namespace for the table
    pub identifier: TableIdentifier,
}

/// A List of Genesis Tables
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
)]
pub struct GenesisTableList {
    /// A list of Genesis Tables
    pub tables: BoundedVec<GenesisTable, ConstU32<128>>,
}

/// 500,000 Bytes
pub const FIVE_HUNDRED_KB: u32 = 500_000;

/// Arrow schema represented by an ipc buffer https://arrow.apache.org/rust/arrow_ipc/convert/fn.try_schema_from_ipc_buffer.html
/// This is what is stored in substrate.
pub type IPCSchema = BoundedVec<u8, ConstU32<FIVE_HUNDRED_KB>>;

/// TODO: add docs
pub type TableName = ByteString;
/// TODO: add docs
pub type TableNamespace = ByteString;

/// A unique identifier for a work assignment, a key that maps to the 'TableSchema'
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    PassByCodec,
    Default,
    Serialize,
    Deserialize,
)]
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

/// Identifier for the scope of a quorum procedure.
#[derive(
    Copy,
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
pub enum QuorumScope {
    /// Refers to public quorum.
    Public,
    /// Refers to privileged quorum.
    Privileged,
}

/// Quorum sizes to exceed to insert to a table for all [`QuorumScope`]s.
#[derive(
    Copy,
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
)]
pub struct InsertQuorumSize {
    /// Number of matching submissions from any indexer to exceed to reach quorum.
    ///
    /// `None` disables the ability to insert to this table via public quorum.
    pub public: Option<u8>,
    /// Number of matching submissions from priveleged indexers to exceed to reach quorum.
    ///
    /// `None` disables the ability to insert to this table via priveleged quorum.
    pub privileged: Option<u8>,
}

impl InsertQuorumSize {
    /// Returns the quorum size to exceed to reach quorum in the given quorum scope.
    pub fn of_scope(&self, quorum_scope: &QuorumScope) -> &Option<u8> {
        match quorum_scope {
            QuorumScope::Public => &self.public,
            QuorumScope::Privileged => &self.privileged,
        }
    }
}

/// TODO: add docs
pub type UpdateTableCmd = (TableIdentifier, CreateStatement);
/// TODO: add docs
pub type UpdateTableList = BoundedVec<UpdateTableCmd, ConstU32<MAX_TABLES_PER_SCHEMA>>;

/// The maximum number of identifiers allowed per source and mode.
/// This constant defines an upper limit for the number of `TableIdentifier` elements
/// that can be associated with a single source and mode.
pub const MAX_IDENTIFIERS_PER_SOURCE_AND_MODE: u32 = 1024;

/// A type alias for the constant representing the maximum number of identifiers per source and mode.
/// Used to constrain the size of collections in storage or logic that rely on this limit.
pub type MaxIdentifiersPerSourceAndMode = ConstU32<MAX_IDENTIFIERS_PER_SOURCE_AND_MODE>;

/// A bounded vector of `TableIdentifier` elements, constrained by the maximum
/// number of identifiers per source and mode (`MaxIdentifiersPerSourceAndMode`).
/// This type ensures that no more than `MAX_IDENTIFIERS_PER_SOURCE_AND_MODE` identifiers
/// can be stored for a source and mode, improving storage efficiency and preventing overflows.
pub type IdentifierList = BoundedVec<TableIdentifier, MaxIdentifiersPerSourceAndMode>;

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

/// Errors that can occur when converting to/from a create statement.
#[derive(Snafu, Debug)]
pub enum CreateStatementParseError {
    /// String representation of table definition exceeds maximum size.
    #[snafu(display("String representation of table definition exceeds maximum size."))]
    StatementTooLarge,
    /// Create statement does not store valid utf8.
    #[snafu(
        display("Create statement does not store valid utf8: {source}"),
        context(false)
    )]
    Utf8 {
        /// The source utf8 error.
        source: Utf8Error,
    },
    /// String representation of table definition exceeds maximum size.
    #[snafu(display("Encountered sqlparser error: {error}"))]
    Sqlparser {
        /// The source parser error.
        error: sqlparser::parser::ParserError,
    },
}

impl From<sqlparser::parser::ParserError> for CreateStatementParseError {
    fn from(error: sqlparser::parser::ParserError) -> Self {
        CreateStatementParseError::Sqlparser { error }
    }
}

/// Convert a sqlparser `CreateTableBuilder` to a [`CreateStatement`].
pub fn sqlparser_to_create_statement(
    create_table: CreateTableBuilder,
) -> Result<CreateStatement, CreateStatementParseError> {
    CreateStatement::try_from(create_table.build().to_string().as_bytes().to_vec())
        .map_err(|_| CreateStatementParseError::StatementTooLarge)
}

/// Convert a [`CreateStatement`] to a sqlparser `CreateTableBuilder`.
pub fn create_statement_to_sqlparser(
    create_statement: CreateStatement,
) -> Result<CreateTableBuilder, CreateStatementParseError> {
    Ok(Parser::new(&PostgreSqlDialect {})
        .try_with_sql(from_utf8(&create_statement)?)?
        .parse_statement()?
        .try_into()?)
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;

    use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
    use sqlparser::ast::{ColumnDef, DataType, Ident};
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

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

    #[test]
    fn we_can_convert_to_and_from_create_statement() {
        let expected_create_statement =
            create_statement("CREATE TABLE test.table (int_col BIGINT)");

        let create_table =
            create_statement_to_sqlparser(expected_create_statement.clone()).unwrap();

        let create_statement = sqlparser_to_create_statement(create_table).unwrap();

        assert_eq!(create_statement, expected_create_statement);
    }

    #[test]
    fn we_cannot_convert_to_too_large_create_statement() {
        let create_statement = create_statement("CREATE TABLE test.table ()");

        let mut create_table = create_statement_to_sqlparser(create_statement.clone()).unwrap();

        create_table.columns = vec![
            ColumnDef {
                name: Ident::new("col"),
                data_type: DataType::BigInt(None),
                collation: None,
                options: vec![],
            };
            1000
        ];

        assert!(matches!(
            sqlparser_to_create_statement(create_table),
            Err(CreateStatementParseError::StatementTooLarge)
        ));
    }

    #[test]
    fn we_cannot_convert_from_create_statement_with_invalid_utf8() {
        let create_statement = CreateStatement::try_from(vec![0xc3]).unwrap();

        assert!(matches!(
            create_statement_to_sqlparser(create_statement.clone()),
            Err(CreateStatementParseError::Utf8 { .. })
        ));
    }

    #[test]
    fn we_cannot_convert_from_create_statement_with_invalid_statement() {
        let create_statement = create_statement("CREATE TABLE 12345 ()");

        assert!(matches!(
            create_statement_to_sqlparser(create_statement.clone()),
            Err(CreateStatementParseError::Sqlparser { .. })
        ));
    }

    #[test]
    fn we_can_get_quorum_size_of_given_scope() {
        let none_insert_quorum_size = InsertQuorumSize::default();

        assert_eq!(
            *none_insert_quorum_size.of_scope(&QuorumScope::Public),
            None
        );
        assert_eq!(
            *none_insert_quorum_size.of_scope(&QuorumScope::Privileged),
            None
        );

        let some_insert_quorum_size = InsertQuorumSize {
            public: Some(3),
            privileged: Some(0),
        };

        assert_eq!(
            *some_insert_quorum_size.of_scope(&QuorumScope::Public),
            Some(3)
        );
        assert_eq!(
            *some_insert_quorum_size.of_scope(&QuorumScope::Privileged),
            Some(0)
        );
    }
}
