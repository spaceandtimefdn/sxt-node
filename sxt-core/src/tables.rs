extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};
use core::str::{from_utf8, Utf8Error};

use codec::{Decode, Encode, MaxEncodedLen};
use polkadot_sdk::frame_support::storage::bounded_vec::BoundedVec;
use polkadot_sdk::frame_support::traits::ConstU32;
use polkadot_sdk::sp_core::{blake2_256, RuntimeDebug, U256};
use polkadot_sdk::sp_runtime::DispatchError;
use polkadot_sdk::sp_runtime_interface::pass_by::PassByCodec;
use proof_of_sql::base::database::TableRef;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use regex::Regex;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
use sqlparser::ast::{ColumnDef, Expr, Ident, ObjectName, SqlOption, Value};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

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
#[cfg_attr(feature = "std", derive(Hash))]
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

    /// Optimistically create a Table Identifier from a given name and namespace. If the
    /// provided str is too long for the destination, this will panic
    /// NOTE: This will uppercase both the name and namespace
    pub fn from_str_unchecked(name: &str, namespace: &str) -> Self {
        TableIdentifier {
            name: TableName::try_from(name.to_uppercase().as_bytes().to_vec()).unwrap(),
            namespace: TableNamespace::try_from(namespace.to_uppercase().as_bytes().to_vec())
                .unwrap(),
        }
    }

    /// Optimistically create a Table Identifier from a given name and namespace. If the
    /// provided str is too long for the destination, this will panic
    /// NOTE: This will preserve the casing of both the name and namespace
    pub fn from_str_unchecked_with_preserved_casing(name: &str, namespace: &str) -> Self {
        TableIdentifier {
            name: TableName::try_from(name.as_bytes().to_vec()).unwrap(),
            namespace: TableNamespace::try_from(namespace.as_bytes().to_vec()).unwrap(),
        }
    }
}

/// Trait for types that can be normalized/capitalized
pub trait TryNormalize {
    /// The error type returned if normalization fails
    type Error;
    /// Attempt to normalize the value
    fn try_normalize(self) -> Result<Self, Self::Error>
    where
        Self: core::marker::Sized;
}
impl<S> TryNormalize for BoundedVec<u8, S>
where
    Self: TryFrom<Vec<u8>>,
{
    type Error = ();

    fn try_normalize(self) -> Result<Self, Self::Error>
    where
        Self: core::marker::Sized,
    {
        from_utf8(&self)
            .map_err(|_| ())?
            .to_uppercase()
            .as_bytes()
            .to_vec()
            .try_into()
            .map_err(|_| ())
    }
}
impl TryNormalize for TableIdentifier {
    type Error = ();
    fn try_normalize(self) -> Result<Self, Self::Error>
    where
        Self: core::marker::Sized,
    {
        Ok(TableIdentifier {
            name: self.name.try_normalize()?,
            namespace: self.namespace.try_normalize()?,
        })
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

impl core::fmt::Display for TableIdentifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let ns = from_utf8(&self.namespace).unwrap_or("?");
        let name = from_utf8(&self.name).unwrap_or("?");
        write!(f, "{}.{}", ns, name)
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

/// Maximum byte length for table metadata.
pub const MAX_TABLE_METADATA_LENGTH: u32 = 8192;

/// Raw metadata bytes stored for tables.
pub type TableMetadataBytes = BoundedVec<u8, ConstU32<MAX_TABLE_METADATA_LENGTH>>;

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
pub const MAX_IDENTIFIERS_PER_SOURCE_AND_MODE: u32 = 2 << 20;

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
#[derive(Snafu, Debug, PartialEq, Eq)]
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

/// Errors that can occur when extracting UUIDs using sqlparser.
#[derive(Debug, Snafu, PartialEq, Eq)]
pub enum UuidExtractionError {
    /// Attempt to set column uuid for a column name that does not exist in the table definition.
    #[snafu(display("Column name not found"))]
    ColumnNameNotFound,
    /// String representation of column name exceeds maximum size.
    #[snafu(display("String representation of column name exceeds maximum size."))]
    ColumnNameTooLarge,
    /// String representation of Column UUID exceeds maximum size.
    #[snafu(display("String representation of Column UUID exceeds maximum size."))]
    ColumnUuidTooLarge,
    /// String representation of Table UUID exceeds maximum size.
    #[snafu(display("String representation of Table UUID exceeds maximum size."))]
    TableUuidTooLarge,
    /// Too many columns in the table definition.
    #[snafu(display("Too many columns in the table definition."))]
    TooManyColumns,
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

/// Errors that can occure when updated a UUID in a SQL statement
#[derive(Snafu, Debug)]
pub enum UpdateUuidError {
    /// The uuid supplied to inject was not valid utf8
    InvalidUuid,
    /// There was an error parsing the SQL statement into or from its object form
    ParseError {
        /// The parsing error
        error: CreateStatementParseError,
    },
}

/// Attempts to insert the provided UUID into the WITH clause of the provided create
/// statement maintaining any other previously assigned options
pub fn update_uuid_in_create_table_statement(
    uuid: TableUuid,
    column_uuids: ColumnUuidList,
    statement: CreateStatement,
) -> Result<CreateStatement, UpdateUuidError> {
    let table: CreateTableBuilder = create_statement_to_sqlparser(statement)
        .map_err(|error| UpdateUuidError::ParseError { error })?;

    let table_uuid_str = from_utf8(uuid.as_slice()).map_err(|_| UpdateUuidError::InvalidUuid)?;

    // Turn the UUID entries into SqlOption
    let table_entry = SqlOption {
        name: "TABLE_UUID".into(),
        value: Expr::Value(Value::UnQuotedString(table_uuid_str.to_string())),
    };

    let mut entries: Vec<SqlOption> = column_uuids
        .iter()
        .filter_map(|entry| {
            match (
                from_utf8(entry.name.as_slice()),
                from_utf8(entry.uuid.as_slice()),
            ) {
                (Ok(name), Ok(val)) => Some(SqlOption {
                    name: name.into(),
                    value: Expr::Value(Value::UnQuotedString(val.to_string())),
                }),
                (_, _) => None,
            }
        })
        .chain(Some(table_entry))
        .chain(table.clone().with_options)
        .collect::<Vec<SqlOption>>();

    // Dedup everything as uppercase to ensure case-insensitive handling
    entries.sort_by(|a, b| {
        a.name
            .value
            .to_uppercase()
            .cmp(&b.name.value.to_uppercase())
    });
    entries.dedup_by(|a, b| a.name.value.to_uppercase() == b.name.value.to_uppercase());

    let table = table.with_options(entries);

    sqlparser_to_create_statement(table).map_err(|error| UpdateUuidError::ParseError { error })
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
        .with_recursion_limit(2)
        .try_with_sql(raw_sql)?
        .parse_statement()?
        .try_into()?)
}
/// Extracts the schema name from a `CREATE SCHEMA` statement.
pub fn extract_create_schema_namespace(raw_sql: &str) -> Option<String> {
    let (stripped_sql, _with_options) = strip_with_clause(raw_sql);

    let statement = Parser::new(&PostgreSqlDialect {})
        .with_recursion_limit(2)
        .try_with_sql(stripped_sql)
        .ok()?
        .parse_statement()
        .ok()?;

    match statement {
        sqlparser::ast::Statement::CreateSchema {
            schema_name: sqlparser::ast::SchemaName::Simple(ObjectName(idents)),
            if_not_exists: true,
        } => match &idents[..] {
            [Ident {
                value,
                quote_style: None,
            }] => Some(value.to_uppercase()),
            _ => None,
        },
        _ => None,
    }
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
        .with_recursion_limit(2)
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
    if input.starts_with("column_")
        && input.ends_with("_uuid")
        && input.len() > "column__uuid".len()
    {
        let start = "column_".len();
        let end = input.len() - "_uuid".len();
        Some(&input[start..end])
    } else {
        None
    }
}

/// Extract Table and Column UUIDs for a given CREATE TABLE statement
pub fn uuids_from_sqlparser(
    create_table: CreateTableBuilder,
) -> Result<(TableUuid, ColumnUuidList), UuidExtractionError> {
    let options = create_table.with_options;
    let mut table_uuid: TableUuid = TableUuid::default();
    let column_names = create_table
        .columns
        .iter()
        .map(|col| col.name.value.to_uppercase())
        .collect::<Vec<String>>();

    let column_uuids: Vec<ColumnUuid> = options
        .iter()
        .filter_map(|opt| {
            extract_column_name(opt.name.value.as_str()).map(|name| {
                if !column_names.contains(&name.to_uppercase()) {
                    return Err(UuidExtractionError::ColumnNameNotFound);
                }
                let name_bytes = ByteString::try_from(name.as_bytes().to_vec())
                    .map_err(|_| UuidExtractionError::ColumnNameTooLarge)?;
                let uuid_bytes = TableUuid::try_from(opt.value.to_string().as_bytes().to_vec())
                    .map_err(|_| UuidExtractionError::ColumnUuidTooLarge)?;
                Ok(ColumnUuid {
                    name: name_bytes,
                    uuid: uuid_bytes,
                })
            })
        })
        .collect::<Result<Vec<_>, UuidExtractionError>>()?;

    let column_id_list =
        ColumnUuidList::try_from(column_uuids).map_err(|_| UuidExtractionError::TooManyColumns)?;

    for opt in options.iter() {
        if opt.name.value.to_lowercase() == "table_uuid" {
            table_uuid = TableUuid::try_from(opt.value.to_string().as_bytes().to_vec())
                .map_err(|_| UuidExtractionError::TableUuidTooLarge)?;
        }
    }

    Ok((table_uuid, column_id_list))
}

/// Generate a new UUID for a given table name and namespace.
pub fn generate_table_uuid(block_number: U256, namespace: &str, name: &str) -> Option<TableUuid> {
    let source = format!("{}{}{}", block_number, namespace, name);
    generate_uuid(source)
}

/// Generate a new UUID for a given namespace
pub fn generate_namespace_uuid(block_number: U256, namespace: &str) -> Option<TableUuid> {
    let source = format!("{}{}", block_number, namespace);
    generate_uuid(source)
}

/// Generate a new UUID for a given column_name. For now we are using the column_name as
/// the id, until additional support is introduced on the database side.
pub fn generate_column_uuid_list(
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
pub fn generate_uuid(source: String) -> Option<TableUuid> {
    let seed = blake2_256(source.as_bytes());
    let mut rng = ChaCha20Rng::from_seed(seed);

    let part1 = (0b1100_u64 << 60) | rng.next_u64();
    let part2 = rng.next_u64();

    let mut hex = format!("{:016X}{:016X}", part1, part2);

    if hex.as_bytes()[0].is_ascii_digit() {
        hex.replace_range(0..1, "A");
    }

    hex.truncate(32);
    TableUuid::try_from(hex.as_bytes().to_vec()).ok()
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

    /// Public permissionless, adds the writer to a "submitter column"
    PublicPermissionless,
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
            TableType::PublicPermissionless => InsertQuorumSize {
                public: Some(0),
                privileged: Some(0),
            },
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

/// A list of column names to column types.
pub type TableSchema = Vec<ScaleColumnSchema>;

/// Simple column schema in a form that can be communicated in chain APIs.
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, TypeInfo)]
pub struct ScaleColumnSchema {
    /// The name of the column.
    name: String,
    /// The data type of the column, as plain SQL.
    data_type: String,
}

impl From<ColumnDef> for ScaleColumnSchema {
    fn from(
        ColumnDef {
            name, data_type, ..
        }: ColumnDef,
    ) -> Self {
        ScaleColumnSchema {
            name: name.value,
            data_type: data_type.to_string(),
        }
    }
}

impl TryFrom<ScaleColumnSchema> for ColumnDef {
    type Error = sqlparser::parser::ParserError;

    fn try_from(
        ScaleColumnSchema { name, data_type }: ScaleColumnSchema,
    ) -> Result<Self, Self::Error> {
        let name = Ident::new(name);

        let parser = Parser::new(&PostgreSqlDialect {});

        let data_type = parser.try_with_sql(&data_type).and_then(|mut parser| {
            let data_type = parser.parse_data_type()?;

            if parser.peek_token() == Token::EOF {
                Ok(data_type)
            } else {
                Err(sqlparser::parser::ParserError::ParserError(
                    "expected data type string to be a single data type".to_string(),
                ))
            }
        })?;

        Ok(ColumnDef {
            name,
            data_type,
            collation: None,
            options: Vec::new(),
        })
    }
}

fn table_schema_from_create_table(create_table: CreateTableBuilder) -> TableSchema {
    create_table
        .columns
        .into_iter()
        .map(ScaleColumnSchema::from)
        .collect()
}

/// Returns a [`TableSchema`] inferred from the given [`CreateStatement`].
pub fn table_schema_from_create_statement(
    create_statement: CreateStatement,
) -> Result<TableSchema, CreateStatementParseError> {
    create_statement_to_sqlparser(create_statement).map(table_schema_from_create_table)
}

/// Errors that can occur when getting a table schema from chain state.
#[derive(Snafu, Debug, Decode, Encode, TypeInfo)]
#[snafu(module)]
pub enum GetTableSchemaError {
    #[snafu(display("table does not exist"))]
    /// Table does not exist.
    NoSuchTable,
    #[snafu(display("stored table definition exceeds maximum size"))]
    /// Stored table definition exceeds maximum size.
    StatementTooLarge,
    #[snafu(display("create statement does not store valid utf8"))]
    /// Create statement does not store valid utf8.
    NotUtf8,
    #[snafu(display("failed to parse create statement"))]
    /// Failed to parse create statement.
    Parse,
}

impl From<CreateStatementParseError> for GetTableSchemaError {
    fn from(error: CreateStatementParseError) -> Self {
        match error {
            CreateStatementParseError::StatementTooLarge => GetTableSchemaError::StatementTooLarge,
            CreateStatementParseError::Utf8 { .. } => GetTableSchemaError::NotUtf8,
            CreateStatementParseError::Sqlparser { .. } => GetTableSchemaError::Parse,
        }
    }
}

fn submitter_col_ident() -> Ident {
    sqlparser::ast::Ident::new("META_SUBMITTER")
}

/// A helper function to inject the submitter address bytes into all rows in the OnChainTable provided
pub fn inject_submitter_data(
    row_data: on_chain_table::OnChainTable,
    submitter: Vec<u8>,
) -> Result<on_chain_table::OnChainTable, on_chain_table::OnChainTableError> {
    let column = on_chain_table::OnChainColumn::VarBinary(vec![submitter; row_data.num_rows()]);
    let column_ready_for_chaining = vec![(submitter_col_ident(), column)];
    on_chain_table::OnChainTable::try_from_iter(
        row_data.into_iter().chain(column_ready_for_chaining),
    )
}

/// Inject a submitter varchar column to a CreateTableBuilder
pub fn inject_submitter_column(mut table: CreateTableBuilder) -> CreateTableBuilder {
    let submitter_col = sqlparser::ast::ColumnDef {
        name: submitter_col_ident(),
        data_type: sqlparser::ast::DataType::Binary(None),
        collation: None,
        options: vec![sqlparser::ast::ColumnOptionDef {
            name: None,
            option: sqlparser::ast::ColumnOption::NotNull,
        }],
    };

    table.columns.push(submitter_col);
    table
}

/// Checks the provided table for any columns with the same name as the
/// system's 'SUBMITTER' column. Returns true if the table conflicts
/// with the system column, false otherwise.
pub fn has_submitter_column(table: &CreateTableBuilder) -> bool {
    let ident = submitter_col_ident();
    table.columns.iter().any(|c: &ColumnDef| c.name == ident)
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::{format, vec};

    use on_chain_table::OnChainColumn;
    use sqlparser::ast::helpers::stmt_create_table::CreateTableBuilder;
    use sqlparser::ast::{ColumnDef, DataType, ExactNumberInfo, Ident, TimezoneInfo};
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn extract_column_name_works() {
        assert_eq!(extract_column_name("column_id_uuid"), Some("id"));
        assert_eq!(extract_column_name("column_name_uuid"), Some("name"));
        assert_eq!(extract_column_name("columnuuid"), None);
        assert_eq!(extract_column_name("column_uuid"), None);
        assert_eq!(extract_column_name("column__uuid"), None);
        assert_eq!(extract_column_name("col_id_uuid"), None);
        assert_eq!(extract_column_name("column_id_uuids"), None);
        assert_eq!(extract_column_name("column_id_uuid_extra"), None);
    }

    #[test]
    fn checking_for_submitter_column_works() {
        let false_statement = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) );";
        let false_table = create_statement_to_sqlparser(
            CreateStatement::try_from(false_statement.as_bytes().to_vec()).unwrap(),
        )
        .unwrap();

        assert!(!has_submitter_column(&false_table));

        let true_statement = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, META_SUBMITTER BINARY NOT NULL, PRIMARY KEY (ID, NAME) );";
        let true_table = create_statement_to_sqlparser(
            CreateStatement::try_from(true_statement.as_bytes().to_vec()).unwrap(),
        )
        .unwrap();

        assert!(has_submitter_column(&true_table));
    }

    #[test]
    fn we_can_inject_submitter_columns_into_table() {
        let test_input = on_chain_table::OnChainTable::try_from_iter([
            (Ident::new("FIRST"), OnChainColumn::BigInt(vec![1])),
            (
                Ident::new("SECOND"),
                OnChainColumn::VarChar(vec!["ABC123".into()]),
            ),
            (Ident::new("THIRD"), OnChainColumn::BigInt(vec![1])),
        ])
        .unwrap();

        let test_submitter = vec![4; 32]; // 32 byte 'address'

        let expected_output = on_chain_table::OnChainTable::try_from_iter([
            (Ident::new("FIRST"), OnChainColumn::BigInt(vec![1])),
            (
                Ident::new("SECOND"),
                OnChainColumn::VarChar(vec!["ABC123".into()]),
            ),
            (Ident::new("THIRD"), OnChainColumn::BigInt(vec![1])),
            (
                Ident::new("META_SUBMITTER"),
                OnChainColumn::VarBinary(vec![test_submitter.clone()]),
            ),
        ])
        .unwrap();

        assert_eq!(
            inject_submitter_data(test_input, test_submitter).unwrap(),
            expected_output
        );
    }

    #[test]
    fn we_can_inject_submitter_data_into_tables() {
        let test_val = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) );";
        let sample_statement = CreateStatement::try_from(test_val.as_bytes().to_vec()).unwrap();

        let expected_val = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, META_SUBMITTER BINARY NOT NULL, PRIMARY KEY (ID, NAME) );";
        let expected_statement =
            CreateStatement::try_from(expected_val.as_bytes().to_vec()).unwrap();
        let expected_output = create_statement_to_sqlparser(expected_statement).unwrap();

        let test_input = create_statement_to_sqlparser(sample_statement).unwrap();

        assert_eq!(inject_submitter_column(test_input), expected_output);
    }

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

        let (table_uuid, columns) = uuids_from_sqlparser(create_table).unwrap();

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

    #[test]
    fn we_can_inject_uuids_into_sql_statements() {
        let test_val = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) );";
        let create_statement: CreateStatement =
            BoundedVec::try_from(test_val.as_bytes().to_vec()).unwrap();
        let test_uuid: TableUuid = BoundedVec::try_from("TESTUUID".as_bytes().to_vec()).unwrap();
        let r = update_uuid_in_create_table_statement(
            test_uuid,
            ColumnUuidList::default(),
            create_statement,
        );
        assert!(r.is_ok());

        let result_statement = r.unwrap();
        let expected = "CREATE TABLE SOUTH.BOOK (ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME)) WITH (TABLE_UUID = TESTUUID)";
        assert_eq!(expected, from_utf8(&result_statement).unwrap());
    }

    #[test]
    fn we_can_inject_uuids_into_sql_statements_while_preserving_other_options() {
        let test_val = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) ) WITH (public_key=A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026,access_type=public_read,immutable=true);";
        let create_statement: CreateStatement =
            BoundedVec::try_from(test_val.as_bytes().to_vec()).unwrap();
        let test_uuid: TableUuid = BoundedVec::try_from("TESTUUID".as_bytes().to_vec()).unwrap();
        let r = update_uuid_in_create_table_statement(
            test_uuid,
            ColumnUuidList::default(),
            create_statement,
        );
        assert!(r.is_ok());

        let result_statement = r.unwrap();
        let expected = "CREATE TABLE SOUTH.BOOK (ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME)) WITH (access_type = public_read, immutable = true, public_key = A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026, TABLE_UUID = TESTUUID)";
        assert_eq!(expected, from_utf8(&result_statement).unwrap());
    }

    #[test]
    fn we_can_inject_uuids_into_sql_statements_overriding_existing_uuids_if_present() {
        let test_val = "CREATE TABLE SOUTH.BOOK( ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME) ) WITH (table_uuid=abc678, public_key=A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026,access_type=public_read,immutable=true);";
        let create_statement: CreateStatement =
            BoundedVec::try_from(test_val.as_bytes().to_vec()).unwrap();
        let test_uuid: TableUuid = BoundedVec::try_from("TESTUUID".as_bytes().to_vec()).unwrap();
        let r = update_uuid_in_create_table_statement(
            test_uuid,
            ColumnUuidList::default(),
            create_statement,
        );
        assert!(r.is_ok());

        let result_statement = r.unwrap();
        let expected = "CREATE TABLE SOUTH.BOOK (ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME)) WITH (access_type = public_read, immutable = true, public_key = A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026, TABLE_UUID = TESTUUID)";
        assert_eq!(expected, from_utf8(&result_statement).unwrap());
    }

    #[test]
    fn we_can_convert_between_column_def_and_scale_column_schema() {
        let expected_columns = [
            (Ident::new("VARCHAR_COl"), DataType::Varchar(None)),
            (Ident::new("TINYINT_COl"), DataType::TinyInt(None)),
            (Ident::new("SMALLINT_COl"), DataType::SmallInt(None)),
            (Ident::new("SMALLINT_COl"), DataType::SmallInt(None)),
            (Ident::new("BIGINT_COl"), DataType::BigInt(None)),
            (
                Ident::new("DECIMAL_COl"),
                DataType::Decimal(ExactNumberInfo::PrecisionAndScale(10, 2)),
            ),
            (Ident::new("BOOLEAN_COl"), DataType::Boolean),
            (
                Ident::new("TIMESTAMP_COl"),
                DataType::Timestamp(None, TimezoneInfo::None),
            ),
            (Ident::new("BINARY_COl"), DataType::Binary(None)),
        ]
        .map(|(name, data_type)| ColumnDef {
            name,
            data_type,
            options: Vec::new(),
            collation: None,
        });

        expected_columns.into_iter().for_each(|expected| {
            let actual = ColumnDef::try_from(ScaleColumnSchema::from(expected.clone())).unwrap();

            assert_eq!(actual, expected);
        });
    }

    #[test]
    fn we_cannot_create_column_def_from_invalid_scale_column_schema() {
        let column_schema = ScaleColumnSchema {
            name: "EXCESSIVE_DATA_TYPE".to_string(),
            data_type: "Binary Varchar".to_string(),
        };
        assert!(ColumnDef::try_from(column_schema).is_err());

        let column_schema = ScaleColumnSchema {
            name: "SQL_NOT_DATA_TYPE".to_string(),
            data_type: "SELECT * FROM TABLE".to_string(),
        };
        assert!(ColumnDef::try_from(column_schema).is_err());

        let column_schema = ScaleColumnSchema {
            name: "NOTHING".to_string(),
            data_type: "".to_string(),
        };
        assert!(ColumnDef::try_from(column_schema).is_err());
    }

    #[test]
    fn we_can_create_table_schema_from_create_statement() {
        let create_statement = create_statement("CREATE TABLE SOUTH.BOOK (ID INT NOT NULL, NAME VARCHAR NOT NULL, PRIMARY KEY (ID, NAME)) WITH (access_type = public_read, immutable = true, public_key = A1D9C617F01C9975117B3D605CD4F945853E263D6E52888EE6E3AF5CB0FA1026, TABLE_UUID = TESTUUID)");

        let table_schema = table_schema_from_create_statement(create_statement).unwrap();
        let expected = vec![
            ScaleColumnSchema {
                name: "ID".to_string(),
                data_type: "INT".to_string(),
            },
            ScaleColumnSchema {
                name: "NAME".to_string(),
                data_type: "VARCHAR".to_string(),
            },
        ];

        assert_eq!(table_schema, expected);
    }

    #[test]
    fn we_cannot_create_table_schema_from_invalid_create_statement() {
        let create_statement = create_statement("SELECT * FROM NOT.CREATE");

        assert!(table_schema_from_create_statement(create_statement).is_err());
    }

    #[test]
    fn uuids_from_sqlparser_returns_invalid_column_name_bytes_error() {
        let long_column_name = "a".repeat(65);
        let sql = format!(
            "CREATE TABLE test.table ({} INT) WITH (column_{}_uuid=abc)",
            long_column_name, long_column_name,
        );

        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&sql)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let result = uuids_from_sqlparser(create_table);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), UuidExtractionError::ColumnNameTooLarge);
    }

    #[test]
    fn uuids_from_sqlparser_returns_invalid_column_uuid_bytes_error() {
        let long_uuid = "a".repeat(65);
        let sql = format!(
            "CREATE TABLE test.table (id INT) WITH (column_id_uuid={})",
            long_uuid
        );

        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&sql)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let result = uuids_from_sqlparser(create_table);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), UuidExtractionError::ColumnUuidTooLarge);
    }

    #[test]
    fn uuids_from_sqlparser_returns_too_many_columns_for_table_error() {
        let mut with_options = String::new();
        let mut col_schema_text = String::new();
        for i in 0..65 {
            if !with_options.is_empty() {
                with_options.push_str(", ");
                col_schema_text.push_str(", ");
            }
            col_schema_text.push_str(&format!("col{} INT", i));
            with_options.push_str(&format!("column_col{}_uuid=uuid{}", i, i));
        }

        let sql = format!(
            "CREATE TABLE test.table ({}) WITH ({})",
            col_schema_text, with_options
        );

        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&sql)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let result = uuids_from_sqlparser(create_table);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), UuidExtractionError::TooManyColumns);
    }

    #[test]
    fn uuids_from_sqlparser_returns_invalid_table_uuid_bytes_error() {
        let long_uuid = "a".repeat(65);
        let sql = format!(
            "CREATE TABLE test.table (id INT) WITH (table_uuid={})",
            long_uuid
        );

        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(&sql)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let result = uuids_from_sqlparser(create_table);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), UuidExtractionError::TableUuidTooLarge);
    }

    #[test]
    fn uuids_from_sqlparser_returns_column_name_not_found_error() {
        let sql =
            "CREATE TABLE test.table (id INT) WITH (table_uuid=abc, column_notacolumn_uuid=def)";

        let create_table: CreateTableBuilder = Parser::new(&PostgreSqlDialect {})
            .try_with_sql(sql)
            .unwrap()
            .parse_statement()
            .unwrap()
            .try_into()
            .unwrap();

        let result = uuids_from_sqlparser(create_table);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), UuidExtractionError::ColumnNameNotFound);
    }
}
