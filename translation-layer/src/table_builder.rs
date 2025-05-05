use subxt::tx::DefaultPayload;
use sxt_core::sxt_chain_runtime;
use sxt_core::sxt_chain_runtime::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use sxt_core::sxt_chain_runtime::api::runtime_types::pallet_tables::pallet::CommitmentCreationCmd;
use sxt_core::sxt_chain_runtime::api::runtime_types::proof_of_sql_commitment_map::commitment_scheme::{CommitmentSchemeFlags, PerCommitmentScheme};
use sxt_core::sxt_chain_runtime::api::runtime_types::proof_of_sql_commitment_map::commitment_storage_map::TableCommitmentBytes;
use sxt_core::sxt_chain_runtime::api::runtime_types::sxt_core::tables::{
    TableIdentifier,
    TableType,
};
use sxt_core::sxt_chain_runtime::api::tables::calls::types::CreateTables;

use crate::model::{ApiSource, CommitmentScheme};

/// A builder for constructing table configurations before adding them to a `TableCreator`.
pub struct TableBuilder<'a> {
    identifier: TableIdentifier,
    ddl_statement: BoundedVec<u8>,
    parent: &'a mut TableCreator,
    table_type: TableType,
    commitment_scheme: Option<CommitmentScheme>,
    snapshot_url: Option<BoundedVec<u8>>,
    commitment: Option<BoundedVec<u8>>,
    source: ApiSource,
}

impl<'a> TableBuilder<'a> {
    /// Creates a new `TableBuilder` linked to a `TableCreator`.
    pub fn new(parent: &'a mut TableCreator) -> Self {
        Self {
            identifier: TableIdentifier {
                name: BoundedVec(Vec::new()),
                namespace: BoundedVec(Vec::new()),
            },
            ddl_statement: BoundedVec(Vec::new()),
            parent,
            table_type: TableType::CoreBlockchain,
            commitment_scheme: None,
            snapshot_url: None,
            commitment: None,
            source: ApiSource::Ethereum,
        }
    }

    /// Sets the source chain for the table
    pub fn source(mut self, source: ApiSource) -> Self {
        self.source = source;
        self
    }

    /// Sets the identifier (name and namespace) for the table.
    pub fn identifier(mut self, name: &str, namespace: &str) -> Self {
        self.identifier = TableIdentifier {
            name: BoundedVec(name.as_bytes().to_vec()),
            namespace: BoundedVec(namespace.as_bytes().to_vec()),
        };
        self
    }

    /// Sets the Data Definition Language (DDL) statement for the table.
    pub fn ddl_statement(mut self, ddl: &str) -> Self {
        self.ddl_statement = BoundedVec(ddl.as_bytes().to_vec());
        self
    }

    /// The type of table we are creating
    pub fn table_type(mut self, table_type: TableType) -> Self {
        self.table_type = table_type;
        self
    }

    /// scheme
    pub fn commitment_scheme(mut self, scheme: CommitmentScheme) -> Self {
        self.commitment_scheme = Some(scheme);
        self
    }

    /// snapshot
    pub fn snapshot_url(mut self, snapshot_url: &str) -> Self {
        self.snapshot_url = Some(BoundedVec(snapshot_url.as_bytes().to_vec()));
        self
    }

    /// commitment as base64 hex
    pub fn commitment(mut self, commitment: &[u8]) -> Self {
        self.commitment = Some(BoundedVec(commitment.to_vec()));
        self
    }

    /// Finalizes the table configuration and adds it to the parent `TableCreator`.
    pub fn add(self) -> &'a mut TableCreator {
        let commitment = match (self.commitment_scheme, self.commitment, self.snapshot_url) {
            (Some(scheme), Some(commitment), Some(snapshot)) => {
                let bytes = TableCommitmentBytes { data: commitment };
                let scheme = match scheme {
                    CommitmentScheme::HyperKzg => PerCommitmentScheme {
                        hyper_kzg: Some(bytes),
                        dynamic_dory: None,
                        __ignore: std::marker::PhantomData,
                    },
                    CommitmentScheme::DynamicDory => PerCommitmentScheme {
                        hyper_kzg: None,
                        dynamic_dory: Some(bytes),
                        __ignore: std::marker::PhantomData,
                    },
                };

                CommitmentCreationCmd::FromSnapshot(snapshot, scheme)
            }
            _ => CommitmentCreationCmd::Empty(CommitmentSchemeFlags {
                hyper_kzg: true,
                dynamic_dory: true,
            }),
        };

        let request = sxt_chain_runtime::api::runtime_types::pallet_tables::pallet::UpdateTable {
            ident: self.identifier,
            create_statement: self.ddl_statement,
            table_type: self.table_type,
            commitment,
            source: self.source.into(),
        };
        self.parent.tables.push(request);
        self.parent
    }
}

/// A creator for defining multiple tables and building an `UpdateTables` payload.
pub struct TableCreator {
    tables: Vec<sxt_chain_runtime::api::runtime_types::pallet_tables::pallet::UpdateTable>,
}

impl TableCreator {
    /// Creates a new `TableCreator` with a specified source and indexer mode.
    pub fn new() -> Self {
        Self { tables: Vec::new() }
    }

    /// Returns a `TableBuilder` for adding a new table configuration.
    pub fn add_table(&mut self) -> TableBuilder {
        TableBuilder::new(self)
    }

    /// Constructs an `UpdateTables` payload from the configured tables.
    pub fn build(self) -> DefaultPayload<CreateTables> {
        sxt_chain_runtime::api::tx()
            .tables()
            .create_tables(BoundedVec(self.tables))
    }

    /// Get the list of tables without finishing the builder
    pub fn tables(
        self,
    ) -> Vec<sxt_chain_runtime::api::runtime_types::pallet_tables::pallet::UpdateTable> {
        self.tables
    }
}

impl Default for TableCreator {
    fn default() -> Self {
        Self::new()
    }
}
