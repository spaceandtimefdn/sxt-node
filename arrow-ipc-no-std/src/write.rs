use arrow::array::RecordBatch;
use arrow::error::ArrowError;
use arrow::ipc::writer::StreamWriter;

/// Encode a record batch as a single-batch IPC stream.
pub fn single_batch_stream_bytes(record_batch: &RecordBatch) -> Result<Vec<u8>, ArrowError> {
    let mut buffer = Vec::new();

    let mut writer = StreamWriter::try_new(&mut buffer, record_batch.schema().as_ref())?;
    writer.write(record_batch)?;
    writer.finish()?;

    Ok(buffer)
}
