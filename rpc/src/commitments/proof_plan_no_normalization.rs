use datafusion::config::{ConfigOptions, SqlParserOptions};
use proof_of_sql::base::database::SchemaAccessor;
use proof_of_sql::sql::proof_plans::DynProofPlan;
use proof_of_sql_planner::{sql_to_proof_plans, PlannerError};
use sqlparser::ast::Statement;

/// Since all of our table identifiers/column identifiers are stored and communicated in all-caps,
/// we need to disable this datafusion setting that will coerce identifiers to lowercase.
fn datafusion_config_no_normalization() -> ConfigOptions {
    let mut config = ConfigOptions::new();
    config.sql_parser = SqlParserOptions {
        enable_ident_normalization: false,
        ..Default::default()
    };
    config
}

/// Returns a proof plan for the given statement with no ident normalization.
pub fn proof_plan_no_normalization<A>(
    statement: &Statement,
    schema_accessor: &A,
) -> Result<DynProofPlan, PlannerError>
where
    A: SchemaAccessor + Clone,
{
    Ok(sql_to_proof_plans(
        std::slice::from_ref(statement),
        schema_accessor,
        &datafusion_config_no_normalization(),
    )?
    .pop()
    .expect("expected one proof plan for one statement"))
}

#[cfg(test)]
mod tests {
    use proof_of_sql::base::database::{ColumnType, TestSchemaAccessor};
    use sqlparser::ast::Ident;
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use super::*;

    fn sample_schema_accessor() -> TestSchemaAccessor {
        TestSchemaAccessor::new(
            [
                (
                    "TEST.TABLE1".parse().unwrap(),
                    [
                        (Ident::new("INT_COL"), ColumnType::Int),
                        (Ident::new("VARCHAR_COL"), ColumnType::VarChar),
                    ]
                    .into_iter()
                    .collect(),
                ),
                (
                    "TEST.TABLE2".parse().unwrap(),
                    [
                        (Ident::new("INT_COL"), ColumnType::Int),
                        (Ident::new("BOOLEAN_COL"), ColumnType::Boolean),
                    ]
                    .into_iter()
                    .collect(),
                ),
            ]
            .into_iter()
            .collect(),
        )
    }

    #[test]
    fn no_normalization_allows_querying_all_caps_identifiers() {
        let schema_accessor = sample_schema_accessor();

        let statement = Parser::new(&PostgreSqlDialect {})
            .try_with_sql("SELECT VARCHAR_COL, TABLE1.INT_COL FROM TEST.TABLE1 JOIN TEST.TABLE2 ON TABLE1.INT_COL = TABLE2.INT_COL WHERE BOOLEAN_COL = true")
            .unwrap()
            .parse_statement()
            .unwrap();

        assert!(proof_plan_no_normalization(&statement, &schema_accessor).is_ok());
    }

    #[test]
    fn we_cannot_create_proof_plan_for_nonexistent_columns() {
        let schema_accessor = sample_schema_accessor();

        let statement = Parser::new(&PostgreSqlDialect {})
            .try_with_sql("SELECT BINARY_COL FROM TEST.TABLE1")
            .unwrap()
            .parse_statement()
            .unwrap();

        assert!(proof_plan_no_normalization(&statement, &schema_accessor).is_err());
    }
}
