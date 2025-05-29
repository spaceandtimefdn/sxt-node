extern crate alloc;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::str::{from_utf8, Utf8Error};

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::storage::bounded_vec::BoundedVec;
use frame_support::traits::ConstU32;
use proof_of_sql::base::database::TableRef;
use regex::Regex;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sp_core::{RuntimeDebug, U256};
use sp_runtime::DispatchError;
use sp_runtime_interface::pass_by::PassByCodec;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::ObjectName;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use super::{ByteString, IDENT_LENGTH};

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

    /// Ethereum Testnet
    Sepolia,

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

/// 500,000 Bytes
pub const FIVE_HUNDRED_KB: u32 = 500_000;

/// Arrow schema represented by an ipc buffer https://arrow.apache.org/rust/arrow_ipc/convert/fn.try_schema_from_ipc_buffer.html
/// This is what is stored in substrate.
pub type IPCSchema = BoundedVec<u8, ConstU32<FIVE_HUNDRED_KB>>;

/// TODO: add docs
pub type TableName = ByteString;
/// TODO: add docs
pub type TableNamespace = ByteString;

/// Version of a given table's schema as a simple incrementing count
pub type TableVersion = u16;

const UUID_MAX_LEN: u32 = IDENT_LENGTH;
/// The UUID for a given table
pub type TableUuid = BoundedVec<u8, ConstU32<UUID_MAX_LEN>>;

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
    /// The name of the table, utf8-encoded
    pub name: TableName,
    /// The namespace of the table, utf8-encoded
    pub namespace: TableNamespace,
}
impl TableIdentifier {
    /// Check to see if the namespace for the table denotes it as a Staking table
    /// important to the system.
    pub fn is_staking_table(&self) -> bool {
        let system_staking: TableNamespace =
            TableNamespace::try_from("SXT_SYSTEM_STAKING".as_bytes().to_vec()).unwrap();
        self.namespace == system_staking
    }

    /// Takes a given Table Identifier and coerces it to uppercase
    pub fn normalized(ident: TableIdentifier) -> Self {
        let name = from_utf8(&ident.name).unwrap();
        let namespace = from_utf8(&ident.namespace).unwrap();
        Self::from_str_unchecked(name, namespace)
    }

    /// Optimistically create a Table Identifier from a given name and namespace. If the
    /// provided str is too long for the destination, this will panic
    pub fn from_str_unchecked(name: &str, namespace: &str) -> Self {
        TableIdentifier {
            name: TableName::try_from(name.to_uppercase().as_bytes().to_vec()).unwrap(),
            namespace: TableNamespace::try_from(namespace.to_uppercase().as_bytes().to_vec())
                .unwrap(),
        }
    }
}

impl TryFrom<&TableIdentifier> for String {
    type Error = Utf8Error;

    fn try_from(table_identifier: &TableIdentifier) -> Result<Self, Self::Error> {
        Ok(format!(
            "{}.{}",
            from_utf8(&table_identifier.namespace)?,
            from_utf8(&table_identifier.name)?
        ))
    }
}

/// A list of UUIDs associated with the columns of a table
pub type ColumnUuidList = BoundedVec<ColumnUuid, ConstU32<MAX_COLS_PER_TABLE>>;

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
/// The type used to define the UUID of a Column within a table
pub struct ColumnUuid {
    /// The name of the column
    pub name: ByteString,
    /// The uuid of the column
    pub uuid: TableUuid,
}

/// Errors that can occur when converting foreign table identifiers to a [`TableIdentifier`].
#[derive(Debug, Snafu, PartialEq, Eq)]
pub enum TableIdentifierConversionError {
    /// Table identifiers should contain two idents like `namespace.name`.
    #[snafu(display("table identifiers should contain two idents like 'namespace.name'"))]
    NotTwoIdentifiers,
    /// Identifier exceeds maximum length.
    #[snafu(display("identifier exceeds maximum length"))]
    IdentifierExceedsMaxLength,
}

impl TryFrom<&ObjectName> for TableIdentifier {
    type Error = TableIdentifierConversionError;

    fn try_from(value: &ObjectName) -> Result<Self, Self::Error> {
        let [namespace, name] = value
            .0
            .iter()
            .map(|ident| {
                ByteString::try_from(ident.value.to_uppercase().as_bytes().to_vec())
                    .map_err(|_| TableIdentifierConversionError::IdentifierExceedsMaxLength)
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| TableIdentifierConversionError::NotTwoIdentifiers)?;

        Ok(TableIdentifier { namespace, name })
    }
}

impl TryFrom<TableRef> for TableIdentifier {
    type Error = TableIdentifierConversionError;

    fn try_from(value: TableRef) -> Result<Self, Self::Error> {
        let namespace = value
            .schema_id()
            .ok_or(TableIdentifierConversionError::NotTwoIdentifiers)?
            .value
            .to_uppercase()
            .as_bytes()
            .to_vec()
            .try_into()
            .map_err(|_| TableIdentifierConversionError::IdentifierExceedsMaxLength)?;

        let name = value
            .table_id()
            .value
            .to_uppercase()
            .as_bytes()
            .to_vec()
            .try_into()
            .map_err(|_| TableIdentifierConversionError::IdentifierExceedsMaxLength)?;

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

impl QuorumScope {
    /// Number of scopes.
    ///
    /// Replace with core::mem::variant_count when it is stable/no_std.
    pub const VARIANT_COUNT: usize = 2;
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

/// A table commitment
pub type CommitmentBytes = BoundedVec<u8, ConstU32<8192>>;

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
/// A wrapper around all the data needed to update or create a table
pub struct UpdateTableRequest {
    /// The table identifier for the table
    pub table: TableIdentifier,
    /// The quorum size rules for the table
    pub quorum_size: InsertQuorumSize,
    /// The create statement for the table, to be passed on
    pub create_statement: CreateStatement,
    /// The uuid of the table. It will be automatically generated if not supplied
    pub table_uuid: Option<TableUuid>,
    /// The uuid of the namespace. It will be automatically generated if not supplied
    pub namespace_uuid: Option<TableUuid>,
}

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

/// Strips the WITH clause from a CREATE TABLE statement, preserving formatting.
fn strip_with_clause(sql: &str) -> (&str, Option<&str>) {
    if let Some(idx) = sql.rfind("WITH") {
        let (before_with, with_and_rest) = sql.split_at(idx);
        let with_clause = with_and_rest.trim_end_matches(';').trim();
        (before_with.trim_end(), Some(with_clause))
    } else {
        (sql.trim_end_matches(';').trim_end(), None)
    }
}

/// Convert a sqlparser `CreateTableBuilder` to a [`CreateStatement`].
pub fn sqlparser_to_create_statement(
    create_table: CreateTableBuilder,
) -> Result<CreateStatement, CreateStatementParseError> {
    CreateStatement::try_from(create_table.build().to_string().as_bytes().to_vec())
        .map_err(|_| CreateStatementParseError::StatementTooLarge)
}

/// todo
pub fn create_statement_to_sqlparser(
    create_statement: CreateStatement,
) -> Result<CreateTableBuilder, CreateStatementParseError> {
    let raw_sql = from_utf8(&create_statement)?;

    Ok(Parser::new(&PostgreSqlDialect {})
        .try_with_sql(raw_sql)?
        .parse_statement()?
        .try_into()?)
}

/// todo
pub fn create_statement_to_sqlparser_remove_with(
    create_statement: CreateStatement,
) -> Result<(CreateTableBuilder, Option<Vec<u8>>), CreateStatementParseError> {
    // Convert to &str
    let raw_sql = from_utf8(&create_statement)?;

    // Strip WITH clause before parsing
    let (stripped_sql, with_options) = strip_with_clause(raw_sql);

    // Parse the cleaned SQL
    let builder: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
        .try_with_sql(stripped_sql)?
        .parse_statement()?
        .try_into()?;

    let with_bytes = with_options.map(|s| s.as_bytes().to_vec());

    Ok((builder, with_bytes))
}
/// Takes a SQL compatible CREATE TABLE statement and converts the WITH statement from an
/// standard format of `WITH (key=value)` to an Ignite compatible format of `WITH "key=value"`
pub fn convert_sql_to_ignite_create_statement(statement: &str) -> String {
    let re = Regex::new(r#"WITH\s+\(([^)]+)\)"#).unwrap();

    re.replace_all(statement, |caps: &regex::Captures| {
        let kv = &caps[1];
        format!("WITH \"{}\"", kv)
    })
    .into_owned()
}

/// Takes an Ignite compatible CREATE TABLE statement and converts the WITH statement from an
/// Ignite format of `WITH "key=value"` to a SQL format of `WITH (key=value)`
pub fn convert_ignite_create_statement(statement: &str) -> String {
    let re = Regex::new(r#"WITH\s+"([^"]+)""#).unwrap();

    re.replace_all(statement, |caps: &regex::Captures| {
        let kv = &caps[1];
        format!("WITH ({})", kv)
    })
    .into_owned()
}

/// Extracts the value of `SCHEMA_UUID` from a `CREATE SCHEMA` statement.
///
/// # Arguments
/// * `sql` - The SQL statement as a `&str`.
///
/// # Returns
/// * `Some(&str)` containing the value of `SCHEMA_UUID`, if found.
/// * `None` if `SCHEMA_UUID` is not present.
pub fn extract_schema_uuid(sql: &str) -> Option<&str> {
    // Find the "WITH (" part
    let with_start = sql.find("WITH (")?;
    let with_part = &sql[with_start + 6..]; // Skip "WITH ("

    // Find the closing parenthesis `)`
    let end_index = with_part.find(')')?;
    let options_str = &with_part[..end_index];

    // Iterate through comma-separated key-value pairs
    for option in options_str.split(',') {
        let mut parts = option.splitn(2, '=').map(str::trim);

        // Extract key and value
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            if key.eq_ignore_ascii_case("SCHEMA_UUID") {
                return Some(value);
            }
        }
    }

    None
}

fn extract_column_name(input: &str) -> Option<&str> {
    if input.starts_with("column_") && input.ends_with("_uuid") {
        let start = "column_".len();
        let end = input.len() - "_uuid".len();
        return Some(&input[start..end]);
    }
    None
}

/// Convenience wrapper around uuids_from_sqlparser that accepts a raw CreateStatement
pub fn uuids_from_create_statement(
    create_statement: CreateStatement,
) -> Option<(TableUuid, ColumnUuidList)> {
    match create_statement_to_sqlparser(create_statement) {
        Ok(create_table) => Some(uuids_from_sqlparser(create_table)),
        _ => None,
    }
}

/// Extract Table and Column UUIDs for a given CREATE TABLE statement
pub fn uuids_from_sqlparser(create_table: CreateTableBuilder) -> (TableUuid, ColumnUuidList) {
    let options = create_table.with_options;
    let mut table_uuid: TableUuid = TableUuid::default();
    let column_id_list = ColumnUuidList::try_from(
        options
            .iter()
            .filter_map(|opt| {
                extract_column_name(opt.name.value.as_str()).map(|name| ColumnUuid {
                    name: ByteString::try_from(name.as_bytes().to_vec()).unwrap(),
                    uuid: TableUuid::try_from(opt.value.to_string().as_bytes().to_vec()).unwrap(),
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| {
        panic!(
            "Column Ids List must not contain more than {}",
            MAX_COLS_PER_TABLE
        )
    });

    for opt in options.iter() {
        if opt.name.value.to_lowercase() == "table_uuid" {
            table_uuid = TableUuid::try_from(opt.value.to_string().as_bytes().to_vec()).unwrap();
        }
    }

    (table_uuid, column_id_list)
}

/// Generate a new UUID for a given table name and namespace.
pub fn generate_table_uuid(
    block_number: U256,
    namespace: &str,
    name: &str,
) -> Result<TableUuid, DispatchError> {
    let source = format!("{}{}{}", block_number, namespace, name);
    generate_uuid(source)
}

/// Generate a new UUID for a given namespace
pub fn generate_namespace_uuid(
    block_number: U256,
    namespace: &str,
) -> Result<TableUuid, DispatchError> {
    let source = format!("{}{}", block_number, namespace);
    generate_uuid(source)
}

/// Generate a new UUID for a given column_name. For now we are using the column_name as
/// the id, until additional support is introduced on the database side.
pub fn generate_column_uuid_list(create_statement: CreateStatement) -> ColumnUuidList {
    let builder = create_statement_to_sqlparser(create_statement.clone()).unwrap();

    let uuid_list = builder
        .columns
        .iter()
        .map(|c| ColumnUuid {
            name: ByteString::try_from(c.name.value.as_bytes().to_vec()).unwrap(),
            uuid: TableUuid::try_from(c.name.value.as_bytes().to_vec()).unwrap(),
        })
        .collect::<Vec<ColumnUuid>>();

    ColumnUuidList::try_from(uuid_list).unwrap()
}

/// V2
pub fn generate_column_uuid_list2(
    create_statement: CreateStatement,
) -> Result<ColumnUuidList, DispatchError> {
    let builder = create_statement_to_sqlparser(create_statement)
        .map_err(|_| DispatchError::Other("Failed to parse CreateStatement"))?;

    let uuids: Vec<ColumnUuid> = builder
        .columns
        .iter()
        .map(|col| {
            let name = ByteString::try_from(col.name.value.as_bytes().to_vec())
                .map_err(|_| DispatchError::Other("Invalid ByteString"))?;
            let uuid = TableUuid::try_from(col.name.value.as_bytes().to_vec())
                .map_err(|_| DispatchError::Other("Invalid TableUuid"))?;
            Ok::<_, DispatchError>(ColumnUuid { name, uuid })
        })
        .collect::<Result<_, _>>()?;

    ColumnUuidList::try_from(uuids).map_err(|_| DispatchError::Other("Too many columns"))
}

/// Generate a new UUID from a given source string
pub fn generate_uuid(source: String) -> Result<TableUuid, DispatchError> {
    // Hash the source
    let hash = sp_core::twox_256(source.as_bytes()).to_vec();
    Ok(TableUuid::try_from(hash).unwrap())
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::{format, vec};

    use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
    use sqlparser::ast::{ColumnDef, DataType, Ident};
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn test_generate_namespace_uuid_deterministic() {
        let block_number = U256::from(123456u64);
        let namespace = "my-namespace";

        let uuid1 = generate_namespace_uuid(block_number, namespace).unwrap();
        let uuid2 = generate_namespace_uuid(block_number, namespace).unwrap();

        assert_eq!(
            uuid1, uuid2,
            "UUIDs should be deterministic for the same input"
        );
    }

    #[test]
    fn test_generate_namespace_uuid_diff_block_numbers() {
        let namespace = "static-ns";

        let uuid1 = generate_namespace_uuid(U256::from(1u64), namespace).unwrap();
        let uuid2 = generate_namespace_uuid(U256::from(2u64), namespace).unwrap();

        assert_ne!(
            uuid1, uuid2,
            "Different block numbers should yield different UUIDs"
        );
    }

    #[test]
    fn test_generate_namespace_uuid_diff_namespaces() {
        let block_number = U256::from(42u64);

        let uuid1 = generate_namespace_uuid(block_number, "ns1").unwrap();
        let uuid2 = generate_namespace_uuid(block_number, "ns2").unwrap();

        assert_ne!(
            uuid1, uuid2,
            "Different namespaces should yield different UUIDs"
        );
    }

    #[test]
    fn test_uuid_has_correct_length() {
        let uuid = generate_namespace_uuid(U256::from(9999), "check-len").unwrap();
        assert_eq!(uuid.len(), 32, "UUID must be 32 bytes long");
    }

    #[test]
    fn we_can_parse_schema_uuid_from_create_schema() {
        let sql = "CREATE SCHEMA SOUTH WITH (SCHEMA_UUID=ABC123);";
        assert_eq!(extract_schema_uuid(sql), Some("ABC123"));

        let sql2 = "CREATE SCHEMA NORTH WITH (ANOTHER_OPT=XYZ, SCHEMA_UUID=XYZ789);";
        assert_eq!(extract_schema_uuid(sql2), Some("XYZ789"));

        let sql3 = "CREATE SCHEMA TEST;";
        assert_eq!(extract_schema_uuid(sql3), None);
    }

    #[test]
    fn we_can_parse_uuids_from_ddl_statement() {
        let expected_uuid = TableUuid::try_from("abc678".as_bytes().to_vec()).unwrap();
        let expected_columns = ColumnUuidList::try_from(vec![
            ColumnUuid {
                name: ByteString::try_from("id".as_bytes().to_vec()).unwrap(),
                uuid: TableUuid::try_from("abc".as_bytes().to_vec()).unwrap(),
            },
            ColumnUuid {
                name: ByteString::try_from("name".as_bytes().to_vec()).unwrap(),
                uuid: TableUuid::try_from("def".as_bytes().to_vec()).unwrap(),
            },
        ])
        .unwrap();
        let sample_statement = CreateStatement::try_from("CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) ) WITH (table_uuid=abc678, column_id_uuid=abc, column_name_uuid=def, public_key=A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026,access_type=public_read,immutable=true)".as_bytes().to_vec()).unwrap();

        let create_table = create_statement_to_sqlparser(sample_statement).unwrap();

        let (table_uuid, columns) = uuids_from_sqlparser(create_table);

        assert_eq!(table_uuid, expected_uuid);
        assert_eq!(columns, expected_columns);
    }

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
            namespace: b"NAMESPACE".to_vec().try_into().unwrap(),
            name: b"NAME".to_vec().try_into().unwrap(),
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
            Err(TableIdentifierConversionError::NotTwoIdentifiers)
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
            Err(TableIdentifierConversionError::NotTwoIdentifiers)
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
            Err(TableIdentifierConversionError::IdentifierExceedsMaxLength)
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

    #[test]
    fn we_can_convert_table_identifier_to_string() {
        let table_identifier = TableIdentifier {
            namespace: b"SCHEMA".to_vec().try_into().unwrap(),
            name: b"TABLE".to_vec().try_into().unwrap(),
        };

        assert_eq!(
            String::try_from(&table_identifier).unwrap(),
            "SCHEMA.TABLE".to_string()
        );
    }

    #[test]
    fn we_cannot_convert_table_identifier_with_invalid_utf8_to_string() {
        let table_identifier = TableIdentifier {
            namespace: b"SCHEMA".to_vec().try_into().unwrap(),
            name: vec![255].try_into().unwrap(),
        };

        assert!(String::try_from(&table_identifier).is_err());
    }

    #[test]
    fn we_can_convert_ignite_statements_and_then_parse_the_with_statement() {
        let test_val = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) ) WITH \"table_uuid=abc678, column_id_uuid=abc, column_name_uuid=def, public_key=A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026,access_type=public_read,immutable=true\";";
        let expected = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) ) WITH (table_uuid=abc678, column_id_uuid=abc, column_name_uuid=def, public_key=A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026,access_type=public_read,immutable=true);";
        let output = convert_ignite_create_statement(test_val);

        assert_eq!(output, expected);

        // Try again with statements that _don't_ end in a semicolon
        let test_val = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) ) WITH \"table_uuid=abc678, column_id_uuid=abc, column_name_uuid=def, public_key=A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026,access_type=public_read,immutable=true\"";
        let expected = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) ) WITH (table_uuid=abc678, column_id_uuid=abc, column_name_uuid=def, public_key=A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026,access_type=public_read,immutable=true)";
        let output = convert_ignite_create_statement(test_val);

        assert_eq!(output, expected);
    }
}

/// The type of table that we are indexing
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
pub enum TableType {
    /// Core Blockchain table
    #[default]
    CoreBlockchain,

    /// Smart Contract Indexing
    SCI,

    /// Community Owned Table
    Community,

    /// Testing type
    Testing(InsertQuorumSize),
}

impl From<TableType> for InsertQuorumSize {
    fn from(table_type: TableType) -> Self {
        match table_type {
            TableType::CoreBlockchain => InsertQuorumSize {
                public: Some(3),
                privileged: None,
            },
            TableType::SCI => InsertQuorumSize {
                public: Some(1),
                privileged: None,
            },
            TableType::Community => InsertQuorumSize {
                public: None,
                privileged: Some(0),
            },
            TableType::Testing(quorum) => quorum,
        }
    }
}

/// Commitment schemes
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
pub enum CommitmentScheme {
    /// HyperKzg
    HyperKzg,
    /// dynamic dory
    DynamicDory,
}
