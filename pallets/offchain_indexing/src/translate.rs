//! Translation between chain types and the protobuf wire format.
//!
//! Provides:
//! - `fq_name`: `TableIdentifier` → `"namespace.name"` string
//! - `ddl_to_arrow_ipc_schema`: DDL bytes → Arrow IPC StreamWriter schema bytes
//! - `on_chain_tables_to_arrow_ipc`: postcard `OnChainTable` blobs → Arrow IPC bytes

use alloc::string::String;
use alloc::vec::Vec;

use arrow::array::RecordBatch;
use arrow::datatypes::Schema;
use arrow::ipc::writer::StreamWriter;
use commitment_sql::ValidatedCreateTable;
use on_chain_table::OnChainTable;
use sxt_core::tables::{create_statement_to_sqlparser, CreateStatement, TableIdentifier};

/// Render a `TableIdentifier` as `"namespace.name"`.
pub fn fq_name(ident: &TableIdentifier) -> String {
    let ns = String::from_utf8_lossy(&ident.namespace);
    let name = String::from_utf8_lossy(&ident.name);
    alloc::format!("{ns}.{name}")
}

/// Derive an Arrow IPC schema blob (StreamWriter format, no record batches)
/// from the on-chain `CREATE TABLE …` DDL bytes.
pub fn ddl_to_arrow_ipc_schema(ddl: &[u8]) -> Result<Vec<u8>, &'static str> {
    let create_statement = CreateStatement::try_from(ddl.to_vec())
        .map_err(|_| "DDL exceeds maximum length")?;

    let create_table = create_statement_to_sqlparser(create_statement)
        .map_err(|_| "failed to parse DDL")?;

    let validated = ValidatedCreateTable::validate(&create_table)
        .map_err(|_| "create table failed validation")?;

    let empty_table: OnChainTable = validated.into_empty_table();
    let batch: RecordBatch = empty_table.into();
    let schema: &Schema = batch.schema_ref();

    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = StreamWriter::try_new(&mut buffer, schema)
        .map_err(|_| "failed to create StreamWriter")?;
    writer.finish().map_err(|_| "failed to finish StreamWriter")?;

    Ok(buffer)
}

/// Decode postcard-serialized `OnChainTable` blobs and re-encode them as a
/// single Arrow IPC StreamWriter blob (schema + N record batches).
pub fn on_chain_tables_to_arrow_ipc(blobs: &[Vec<u8>]) -> Result<Vec<u8>, &'static str> {
    if blobs.is_empty() {
        return Err("no blobs to convert");
    }

    let batches: Vec<RecordBatch> = blobs
        .iter()
        .map(|bytes| {
            let table: OnChainTable =
                postcard::from_bytes(bytes).map_err(|_| "postcard decode failed")?;
            Ok(RecordBatch::from(table))
        })
        .collect::<Result<Vec<_>, &'static str>>()?;

    let schema = batches[0].schema();

    let mut buffer: Vec<u8> = Vec::new();
    let mut writer = StreamWriter::try_new(&mut buffer, &schema)
        .map_err(|_| "failed to create StreamWriter")?;
    for batch in &batches {
        writer.write(batch).map_err(|_| "failed to write batch")?;
    }
    writer.finish().map_err(|_| "failed to finish StreamWriter")?;

    Ok(buffer)
}
