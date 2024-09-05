use super::ByteString;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::storage::bounded_vec::BoundedVec;
use frame_support::traits::ConstU32;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

const MAX_COLS_PER_TABLE: u32 = 64;
const MAX_TABLES_PER_SCHEMA: u32 = 1024;

pub type MaxColsPerTable = ConstU32<MAX_COLS_PER_TABLE>;
pub type MaxTablesPerSchema = ConstU32<MAX_TABLES_PER_SCHEMA>;

/// List of possible chains that the transaction node supports.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
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
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub enum IndexerMode {
    #[default]
    Core,
    Full,
    PriceFeeds,
    SmartContract(ByteString),
    UserCreated(ByteString),
}

/// A request for work from an indexer
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct SourceAndMode {
    pub source: Source,
    pub mode: IndexerMode,
}

/// Two megabytes
pub const FIVE_HUNDRED_KB: u32 = 500_000;

/// Arrow schema represented by an ipc buffer https://arrow.apache.org/rust/arrow_ipc/convert/fn.try_schema_from_ipc_buffer.html
/// This is what is stored in substrate.
pub type IPCSchema = BoundedVec<u8, ConstU32<FIVE_HUNDRED_KB>>;

pub type TableName = ByteString;
pub type TableNamespace = ByteString;

/// A unique identifier for a work assignment, a key that maps to the 'TableSchema'
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default)]
pub struct TableIdentifier {
    pub name: TableName,
    pub namespace: TableNamespace,
}

/// Maximum primary keys for a table
// TODO find suitable values for both of these
pub const MAX_PRIMARY_KEYS: u32 = 32;
pub type PrimaryKey = ByteString;
pub type PrimaryKeys = BoundedVec<PrimaryKey, ConstU32<MAX_PRIMARY_KEYS>>;

/// Maximum foreign keys for a table
pub const MAX_FOREIGN_KEYS: u32 = 32;
pub type ForeignKey = ByteString;
pub type ForeignKeys = BoundedVec<ForeignKey, ConstU32<MAX_FOREIGN_KEYS>>;

pub const CREATE_STMNT_LENGTH: u32 = 8192;
pub type CreateStatement = BoundedVec<u8, ConstU32<CREATE_STMNT_LENGTH>>;

pub type CreateStatements = BoundedVec<CreateStatement, ConstU32<MAX_TABLES_PER_SCHEMA>>;

pub type UpdateTableCmd = (TableIdentifier, CreateStatement);
pub type UpdateTableList = BoundedVec<UpdateTableCmd, ConstU32<MAX_TABLES_PER_SCHEMA>>;
