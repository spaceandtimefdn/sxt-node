use std::sync::Arc;

use arrow::array::{ArrayRef, StringArray};
use arrow::datatypes::DataType;

/// Returns the provided column with null bytes removed from values if it is Utf8.
pub fn column_remove_null_bytes(column: ArrayRef) -> ArrayRef {
    match column.data_type() {
        DataType::Utf8 => Arc::new(StringArray::from_iter(
            column
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap()
                .iter()
                .map(|maybe_string| maybe_string.map(|string| string.replace("\0", ""))),
        )),
        _ => column,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn we_can_remove_null_bytes_from_strings() {
        let column: ArrayRef = Arc::new(StringArray::from_iter([
            Some("lorem"),
            Some("i\0ps\0um"),
            None,
            Some("\0"),
        ]));

        let result = column_remove_null_bytes(column);
        let expected = Arc::new(StringArray::from_iter([
            Some("lorem"),
            Some("ipsum"),
            None,
            Some(""),
        ]));

        assert_eq!(result.as_ref(), expected.as_ref());
    }
}
