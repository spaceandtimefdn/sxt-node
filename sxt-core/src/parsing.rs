use std::{fs::read_to_string, path::Path};

use sqlparser::{ast::Statement, dialect::GenericDialect, parser::Parser};

use crate::tables::{
    create_statement, table_identifier, CreateStatement, SourceAndMode, TableIdentifier,
};

/// Read a ddl file from a path and parse it into create table statements, any error will cause a panic
pub fn ddl_to_tables(
    p: &Path,
    sm: SourceAndMode,
) -> Vec<(SourceAndMode, TableIdentifier, CreateStatement)> {
    let ddl = read_to_string(p).unwrap();

    let mut parser = Parser::new(&GenericDialect {})
        .try_with_sql(ddl.as_str())
        .unwrap();
    let statements = parser.parse_statements().unwrap();

    statements
        .iter()
        .filter_map(|x| match x {
            Statement::CreateTable(c) => {
                let name = c.name.to_string();
                let pieces: Vec<&str> = name.split(".").collect();
                let namespace = pieces.first().unwrap();
                let name = pieces.get(1).unwrap();
                let s = c.to_string();
                let sm = sm.clone();

                Some((sm, table_identifier(name, namespace), create_statement(&s)))
            }
            _ => None,
        })
        .collect()
}

/// Convert a vector of ddl paths and source and modes into a vector of tables for genesis configuration
pub fn ddls_to_genesis(
    input: Vec<(String, SourceAndMode)>,
) -> Vec<(SourceAndMode, TableIdentifier, CreateStatement)> {
    input
        .iter()
        .flat_map(|(path, sm)| ddl_to_tables(Path::new(path), sm.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tables_from_ddl_works() {
        ddl_to_tables(Path::new("testing/ddl.sql"), SourceAndMode::default());
    }

    #[test]
    fn ddls_to_genesis_works() {
        ddls_to_genesis(
            vec![
                ("testing/ddl.sql".into(), SourceAndMode::default()),
                ("testing/ddl2.sql".into(), SourceAndMode::default())
            ]
        );
    }
}