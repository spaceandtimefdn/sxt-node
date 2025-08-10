use proof_of_sql::base::commitment::{ColumnBounds, ColumnCommitmentMetadata};
use proof_of_sql::base::database::ColumnType;
use sqlparser::ast::Ident;

/// Returns the same column commitment metadata, but if a column claims to be varchar, claim that
/// it is varbinary instead.
pub fn varchar_to_varbinary(
    (identifier, column_commitment_metadata): (Ident, ColumnCommitmentMetadata),
) -> (Ident, ColumnCommitmentMetadata) {
    if column_commitment_metadata.column_type() == &ColumnType::VarChar {
        (
            identifier,
            ColumnCommitmentMetadata::try_new(ColumnType::VarBinary, ColumnBounds::NoOrder)
                .expect("varbinary columns have no order"),
        )
    } else {
        (identifier, column_commitment_metadata)
    }
}

/// Returns the same column commitment metadata, but if a column claims to be varbinary, claim that
/// it is varchar instead.
pub fn varbinary_to_varchar(
    (identifier, column_commitment_metadata): (Ident, ColumnCommitmentMetadata),
) -> (Ident, ColumnCommitmentMetadata) {
    if column_commitment_metadata.column_type() == &ColumnType::VarBinary {
        (
            identifier,
            ColumnCommitmentMetadata::try_new(ColumnType::VarChar, ColumnBounds::NoOrder)
                .expect("varchar columns have no order"),
        )
    } else {
        (identifier, column_commitment_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn we_can_map_varchar_to_varbinary() {
        let ident = Ident::new("COL");
        let varchar_column_commitment_metadata =
            ColumnCommitmentMetadata::from_column_type_with_max_bounds(ColumnType::VarChar);

        let (mapped_ident, mapped_column_commitment_metadata) =
            varchar_to_varbinary((ident.clone(), varchar_column_commitment_metadata));

        assert_eq!(ident, mapped_ident);
        assert_eq!(
            mapped_column_commitment_metadata.column_type(),
            &ColumnType::VarBinary
        );
        assert_eq!(
            mapped_column_commitment_metadata.bounds(),
            &ColumnBounds::NoOrder
        );

        let int_column_commitment_metadata =
            ColumnCommitmentMetadata::from_column_type_with_max_bounds(ColumnType::Int);

        let (mapped_ident, mapped_column_commitment_metadata) =
            varchar_to_varbinary((ident.clone(), int_column_commitment_metadata));

        assert_eq!(ident, mapped_ident);
        assert_eq!(
            int_column_commitment_metadata,
            mapped_column_commitment_metadata
        );
    }

    #[test]
    fn we_can_map_varbinary_to_varchar() {
        let ident = Ident::new("COL");
        let varbinary_column_commitment_metadata =
            ColumnCommitmentMetadata::from_column_type_with_max_bounds(ColumnType::VarBinary);

        let (mapped_ident, mapped_column_commitment_metadata) =
            varbinary_to_varchar((ident.clone(), varbinary_column_commitment_metadata));

        assert_eq!(ident, mapped_ident);
        assert_eq!(
            mapped_column_commitment_metadata.column_type(),
            &ColumnType::VarChar
        );
        assert_eq!(
            mapped_column_commitment_metadata.bounds(),
            &ColumnBounds::NoOrder
        );

        let int_column_commitment_metadata =
            ColumnCommitmentMetadata::from_column_type_with_max_bounds(ColumnType::Int);

        let (mapped_ident, mapped_column_commitment_metadata) =
            varbinary_to_varchar((ident.clone(), int_column_commitment_metadata));

        assert_eq!(ident, mapped_ident);
        assert_eq!(
            int_column_commitment_metadata,
            mapped_column_commitment_metadata
        );
    }
}
