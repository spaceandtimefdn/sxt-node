use alloc::string::{FromUtf8Error, String};
use core::convert::Infallible;

use on_chain_table::OnChainColumn;
use sqlparser::ast::Ident;

/// Returns varchar columns as utf8-encoded varbinary columns, no-op for other types.
pub fn varchar_to_varbinary(
    (identifier, column): (Ident, OnChainColumn),
) -> Result<(Ident, OnChainColumn), Infallible> {
    match column {
        OnChainColumn::VarChar(strings) => {
            let column =
                OnChainColumn::VarBinary(strings.into_iter().map(String::into_bytes).collect());

            Ok((identifier, column))
        }
        _ => Ok((identifier, column)),
    }
}

/// Returns varbinary columns as utf8-decoded varchar columns, no-op for other types.
pub fn varbinary_to_varchar(
    (identifier, column): (Ident, OnChainColumn),
) -> Result<(Ident, OnChainColumn), FromUtf8Error> {
    match column {
        OnChainColumn::VarBinary(byte_strings) => {
            let column = OnChainColumn::VarChar(
                byte_strings
                    .into_iter()
                    .map(String::from_utf8)
                    .collect::<Result<_, _>>()?,
            );

            Ok((identifier, column))
        }
        _ => Ok((identifier, column)),
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn we_can_map_varchar_to_varbinary() {
        let ident = Ident::new("COL");
        let varchar_column =
            OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec());

        let expected_binary_column =
            OnChainColumn::VarBinary([b"lorem", b"ipsum", b"dolor"].map(Vec::from).to_vec());

        let (mapped_ident, mapped_column) =
            varchar_to_varbinary((ident.clone(), varchar_column)).unwrap();
        assert_eq!(mapped_ident, ident);
        assert_eq!(mapped_column, expected_binary_column);

        let int_column = OnChainColumn::Int(vec![1, 2, 3]);
        let (mapped_ident, mapped_column) =
            varchar_to_varbinary((ident.clone(), int_column.clone())).unwrap();
        assert_eq!(mapped_ident, ident);
        assert_eq!(mapped_column, int_column);
    }

    #[test]
    fn we_can_map_varbinary_to_varchar() {
        let ident = Ident::new("COL");
        let binary_column =
            OnChainColumn::VarBinary([b"lorem", b"ipsum", b"dolor"].map(Vec::from).to_vec());

        let expected_varchar_column =
            OnChainColumn::VarChar(["lorem", "ipsum", "dolor"].map(String::from).to_vec());

        let (mapped_ident, mapped_column) =
            varbinary_to_varchar((ident.clone(), binary_column)).unwrap();
        assert_eq!(mapped_ident, ident);
        assert_eq!(mapped_column, expected_varchar_column);

        let int_column = OnChainColumn::Int(vec![1, 2, 3]);
        let (mapped_ident, mapped_column) =
            varbinary_to_varchar((ident.clone(), int_column.clone())).unwrap();
        assert_eq!(mapped_ident, ident);
        assert_eq!(mapped_column, int_column);
    }

    #[test]
    fn we_cannot_map_non_utf8_varbinary_to_varchar() {
        let ident = Ident::new("COL");
        let binary_column = OnChainColumn::VarBinary(vec![vec![128]]);

        assert!(varbinary_to_varchar((ident, binary_column)).is_err());
    }
}
