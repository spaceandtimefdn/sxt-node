use indexmap::IndexSet;
use proof_of_sql::base::database::{ParseError, TableRef};
use proof_of_sql_planner::get_table_refs_from_statement;
use sqlparser::ast::Statement;

#[derive(Clone, Debug, PartialEq, Eq)]
/// A sqlparser `Statement` and all of its relations as proof-of-sql `TableRef`s.
pub struct StatementAndAssociatedTableRefs {
    statement: Statement,
    table_refs: IndexSet<TableRef>,
}

impl TryFrom<Statement> for StatementAndAssociatedTableRefs {
    type Error = ParseError;

    fn try_from(statement: Statement) -> Result<Self, Self::Error> {
        let table_refs = get_table_refs_from_statement(&statement)?;

        Ok(StatementAndAssociatedTableRefs {
            statement,
            table_refs,
        })
    }
}

impl StatementAndAssociatedTableRefs {
    /// Returns the stored sqlparser statement.
    pub fn statement(&self) -> &Statement {
        &self.statement
    }

    /// Returns the stored table refs.
    pub fn table_refs(&self) -> &IndexSet<TableRef> {
        &self.table_refs
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use sqlparser::dialect::GenericDialect;
    use sqlparser::parser::Parser;

    use super::*;

    #[test]
    fn we_can_construct_statement_and_associated_table_refs_from_statement() {
        let sql_text =
            "SELECT * FROM NAMESPACE.TABLE1 JOIN NAMESPACE.TABLE2 ON TABLE1.COL = TABLE2.COL";
        let statement = Parser::parse_sql(&GenericDialect {}, sql_text)
            .unwrap()
            .pop()
            .unwrap();

        let statement_and_associated_table_refs =
            StatementAndAssociatedTableRefs::try_from(statement.clone()).unwrap();

        assert_eq!(statement_and_associated_table_refs.statement(), &statement);

        let expected_table_refs = ["NAMESPACE.TABLE1", "NAMESPACE.TABLE2"]
            .map(TableRef::from_str)
            .map(Result::unwrap)
            .into_iter()
            .collect::<IndexSet<_>>();
        assert_eq!(
            statement_and_associated_table_refs.table_refs(),
            &expected_table_refs
        );
    }
}
