//! The native code implementation
use sp_runtime::BoundedVec;
use sp_runtime_interface::runtime_interface;

use sxt_core::native::{NativeError, OnChainTableBytes, RowData};

#[cfg(feature = "std")]
use arrow::ipc::reader::StreamReader;
use postcard::to_allocvec;

/// Space and Time's native code interface
#[runtime_interface]
pub trait Interface {
    /// Convert a sxt_core::native::RowData into a serialized OnChainTable.
    /// RowData is a wrapper around a bounded vec that contains the table in IPC format.
    /// After the table is parsed into a record batch we convert it into an OnChainTable and then serialize it to pass back into the runtime.
    fn record_batch_to_onchain(row_data: RowData) -> Result<OnChainTableBytes, NativeError> {
        let mut reader =
            StreamReader::try_new(row_data.row_data.as_slice(), None).map_err(|_| NativeError::DeserializationError)?;

        let batch = reader
            .next()
            .ok_or(NativeError::EmptyRecordBatchError)?
            .map_err(|_| NativeError::BatchReadError)?;

        let on_chain_table = on_chain_table::OnChainTable::try_from(batch.clone()).map_err(|_| {
           NativeError::OnChainTableConversionError 
        })?;

        let table_bytes =
            to_allocvec(&on_chain_table).map_err(|e| NativeError::SerializationError)?;

        let table_bytes: BoundedVec<u8, _> =
            BoundedVec::try_from(table_bytes).map_err(|e| NativeError::BoundedVecError)?;

        Ok(OnChainTableBytes { data: table_bytes })
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, sync::Arc};

    use arrow::{array::{ArrayRef, Float64Array, Int32Array, RecordBatch}, datatypes::{DataType, Field, Schema}, ipc::writer::StreamWriter};

    use super::*;
    
    fn row_data() -> RowData {
        let schema = Arc::new(Schema::new(vec![
            Field::new("int_column", DataType::Int32, false),
        ]));
    
        let int_data = Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5])) as ArrayRef;
    
        let batch = RecordBatch::try_new(schema.clone(), vec![int_data]).unwrap();
    
        let buffer: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(buffer);
    
        let mut writer = StreamWriter::try_new(&mut cursor, &schema).unwrap();
    
        writer.write(&batch).unwrap();
        writer.finish().unwrap();
    
        let data = writer.into_inner().unwrap().clone();
        let data = data.into_inner().clone();
        
        RowData { row_data: BoundedVec::try_from(data).unwrap() }
    }
    
    #[test]
    fn conversion_works() {
       let res = interface::record_batch_to_onchain(row_data());
       assert!(res.is_ok());
    }
}
