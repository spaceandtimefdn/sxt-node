use std::io::Write;

use arrow::ipc::reader::StreamReader;
use arrow::util::pretty::print_batches;

/// Print a given hex encoded record batch
pub(crate) fn print_batch(hex_encoded: &str) -> anyhow::Result<()> {
    let hex_minus_prefix = hex_encoded.strip_prefix("0x").unwrap_or(hex_encoded);

    let bytes = hex::decode(hex_minus_prefix)?;

    let mut file = std::fs::File::create("./batch")?;
    let _ = file.write(&bytes)?;

    let mut reader = StreamReader::try_new(bytes.as_slice(), None)?;

    let batch = (reader.next().unwrap())?;

    print_batches(&[batch.clone()])?;
    dbg!(&batch.schema());
    Ok(())
}
