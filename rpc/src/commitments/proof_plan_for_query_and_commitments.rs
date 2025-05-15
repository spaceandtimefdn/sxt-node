use datafusion::config::{ConfigOptions, SqlParserOptions};
use proof_of_sql::base::commitment::{QueryCommitments, TableCommitment};
use proof_of_sql::sql::proof_plans::DynProofPlan;
use proof_of_sql_commitment_map::generic_over_commitment::ConcreteType;
use proof_of_sql_commitment_map::{CommitmentId, GenericOverCommitmentFn, TableCommitmentBytes};
use proof_of_sql_planner::sql_to_proof_plans;

use super::error::CommitmentsApiError;
use super::statement_and_associated_table_refs::StatementAndAssociatedTableRefs;

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

fn proof_plan_for_query_and_commitments<C: CommitmentId>(
    statement: StatementAndAssociatedTableRefs,
    table_commitments: &[TableCommitmentBytes],
) -> Result<DynProofPlan, CommitmentsApiError> {
    let num_tables = statement.table_refs().len();
    let num_commitments = table_commitments.len();
    if num_tables != num_commitments {
        return Err(CommitmentsApiError::UnexpectedTableCommitmentMismap {
            num_tables,
            num_commitments,
        });
    }

    let table_commitments_typed = table_commitments
        .iter()
        .map(TableCommitment::<C>::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| CommitmentsApiError::DeserializeTableCommitment { source })?;

    let query_commitments: QueryCommitments<C> = statement
        .table_refs()
        .iter()
        .cloned()
        .zip(table_commitments_typed)
        .collect();

    let proof_plan = sql_to_proof_plans(
        std::slice::from_ref(statement.statement()),
        &query_commitments,
        &datafusion_config_no_normalization(),
    )?
    .pop()
    .expect("expected one proof plan for one statement");

    Ok(proof_plan)
}

/// `GenericOverCommitmentFn` that returns a `DynProofPlan` for the given query and commitments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofPlanForQueryAndCommitments(pub StatementAndAssociatedTableRefs);

impl GenericOverCommitmentFn for ProofPlanForQueryAndCommitments {
    type In = ConcreteType<Vec<TableCommitmentBytes>>;
    type Out = ConcreteType<Result<DynProofPlan, CommitmentsApiError>>;

    fn call<C: CommitmentId>(
        &self,
        input: Vec<TableCommitmentBytes>,
    ) -> Result<DynProofPlan, CommitmentsApiError> {
        proof_plan_for_query_and_commitments::<C>(self.0.clone(), &input)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::marker::PhantomData;

    use commitment_sql::OnChainTableToTableCommitmentFn;
    use indexmap::IndexMap;
    use itertools::Itertools;
    use on_chain_table::{OnChainColumn, OnChainTable};
    use proof_of_sql::base::database::{ColumnType, TableRef, TestSchemaAccessor};
    use proof_of_sql_commitment_map::generic_over_commitment::{
        GenericOverCommitment,
        PairType,
        ResultOkType,
        TableCommitmentType,
    };
    use proof_of_sql_commitment_map::{AnyCommitmentScheme, PerCommitmentScheme};
    use proof_of_sql_static_setups::io::get_or_init_from_files_with_four_points_unchecked;
    use sqlparser::ast::Ident;
    use sqlparser::dialect::GenericDialect;
    use sqlparser::parser::Parser;

    use super::*;

    struct UnwrapResultFn<O, E>(PhantomData<(O, E)>);

    impl<O, E> GenericOverCommitmentFn for UnwrapResultFn<O, E>
    where
        O: GenericOverCommitment,
        E: Debug,
    {
        type In = ResultOkType<O, E>;
        type Out = O;

        fn call<C: CommitmentId>(
            &self,
            input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
        ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
            input.unwrap()
        }
    }

    struct TableCommitmentToBytesFn;

    impl GenericOverCommitmentFn for TableCommitmentToBytesFn {
        type In = TableCommitmentType;
        type Out = ConcreteType<TableCommitmentBytes>;

        fn call<C: CommitmentId>(&self, input: TableCommitment<C>) -> TableCommitmentBytes {
            TableCommitmentBytes::try_from(&input).unwrap()
        }
    }

    struct ListConcretePairFn<T>(PhantomData<T>);

    impl<T> GenericOverCommitmentFn for ListConcretePairFn<T> {
        type In = PairType<ConcreteType<T>, ConcreteType<T>>;
        type Out = ConcreteType<Vec<T>>;

        fn call<C: CommitmentId>(
            &self,
            input: <Self::In as GenericOverCommitment>::WithCommitment<C>,
        ) -> <Self::Out as GenericOverCommitment>::WithCommitment<C> {
            vec![input.0, input.1]
        }
    }

    fn valid_statement_and_commitments() -> (
        StatementAndAssociatedTableRefs,
        PerCommitmentScheme<ConcreteType<Vec<TableCommitmentBytes>>>,
        DynProofPlan,
    ) {
        let setups = get_or_init_from_files_with_four_points_unchecked();

        let table_1_id: TableRef = "NAMESPACE.TABLE1".parse().unwrap();
        let int_col_id = Ident::new("INT_COL");
        let table1_col_id: Ident = Ident::new("TABLE1_COL");
        let table_1_data = OnChainTable::try_from_iter([
            (int_col_id.clone(), OnChainColumn::Int(vec![1, 2])),
            (
                table1_col_id.clone(),
                OnChainColumn::VarChar(["lorem", "ipsum"].map(String::from).to_vec()),
            ),
        ])
        .unwrap();
        let table_1_commitments = setups
            .map(OnChainTableToTableCommitmentFn::new(&table_1_data, 0))
            .map(UnwrapResultFn(PhantomData));
        let table_1_commitment_bytes = table_1_commitments.map(TableCommitmentToBytesFn);

        let table_2_id: TableRef = "NAMESPACE.TABLE2".parse().unwrap();
        let table2_col_id: Ident = Ident::new("TABLE2_COL");
        let table_2_data = OnChainTable::try_from_iter([
            (int_col_id.clone(), OnChainColumn::Int(vec![1, 2])),
            (
                table2_col_id.clone(),
                OnChainColumn::VarChar(["dolor", "sit"].map(String::from).to_vec()),
            ),
        ])
        .unwrap();
        let table_2_commitments = setups
            .map(OnChainTableToTableCommitmentFn::new(&table_2_data, 0))
            .map(UnwrapResultFn(PhantomData));
        let table_2_commitment_bytes = table_2_commitments.map(TableCommitmentToBytesFn);

        let table_commitment_lists_per_commitment_scheme = table_1_commitment_bytes
            .zip(table_2_commitment_bytes)
            .map(ListConcretePairFn(PhantomData));

        let sql_text = format!("SELECT * FROM {table_1_id} JOIN {table_2_id} ON {table_1_id}.{int_col_id} = {table_2_id}.{int_col_id}");
        let statement = Parser::parse_sql(&GenericDialect {}, &sql_text)
            .unwrap()
            .pop()
            .unwrap();

        let statement_and_associated_table_refs =
            StatementAndAssociatedTableRefs::try_from(statement.clone()).unwrap();

        let expected_schema_accessor = TestSchemaAccessor::new(IndexMap::from_iter([
            (
                table_1_id,
                IndexMap::from_iter([
                    (int_col_id.clone(), ColumnType::Int),
                    (table1_col_id, ColumnType::VarChar),
                ]),
            ),
            (
                table_2_id,
                IndexMap::from_iter([
                    (int_col_id, ColumnType::Int),
                    (table2_col_id, ColumnType::VarChar),
                ]),
            ),
        ]));

        let expected_proof_plan = sql_to_proof_plans(
            &[statement],
            &expected_schema_accessor,
            &datafusion_config_no_normalization(),
        )
        .unwrap()
        .pop()
        .unwrap();

        (
            statement_and_associated_table_refs,
            table_commitment_lists_per_commitment_scheme,
            expected_proof_plan,
        )
    }

    #[test]
    fn we_can_construct_proof_plan_for_query_and_commitments() {
        let (
            statement_and_associated_table_refs,
            table_commitment_lists_per_commitment_scheme,
            expected_proof_plan,
        ) = valid_statement_and_commitments();

        let proof_plan = table_commitment_lists_per_commitment_scheme
            .map(ProofPlanForQueryAndCommitments(
                statement_and_associated_table_refs,
            ))
            .into_iter()
            .map(|any| any.unwrap().unwrap())
            .all_equal_value()
            .expect("all commitment schemes should have the same result");

        assert_eq!(proof_plan, expected_proof_plan);
    }

    #[test]
    fn we_cannot_construct_proof_plan_with_table_commitments_mismap() {
        let (_, table_commitment_lists_per_commitment_scheme, _) =
            valid_statement_and_commitments();

        let statement_with_only_one_table = StatementAndAssociatedTableRefs::try_from(
            Parser::parse_sql(&GenericDialect {}, "SELECT * FROM NAMESPACE.TABLE1")
                .unwrap()
                .pop()
                .unwrap(),
        )
        .unwrap();

        table_commitment_lists_per_commitment_scheme
            .map(ProofPlanForQueryAndCommitments(
                statement_with_only_one_table,
            ))
            .into_iter()
            .for_each(|any_result| {
                assert!(matches!(
                    any_result.unwrap(),
                    Err(CommitmentsApiError::UnexpectedTableCommitmentMismap { .. })
                ));
            })
    }

    #[test]
    fn we_cannot_construct_proof_plan_with_invalid_table_commitments() {
        let (statement_and_associated_table_refs, _, _) = valid_statement_and_commitments();

        let bad_commitment = TableCommitmentBytes {
            data: vec![].try_into().unwrap(),
        };

        let bad_commitments =
            AnyCommitmentScheme::HyperKzg(vec![bad_commitment.clone(), bad_commitment]);

        let result = bad_commitments
            .map(ProofPlanForQueryAndCommitments(
                statement_and_associated_table_refs,
            ))
            .unwrap();

        assert!(matches!(
            result,
            Err(CommitmentsApiError::DeserializeTableCommitment { .. })
        ));
    }

    #[test]
    fn we_cannot_construct_proof_plan_with_nonexistent_column() {
        let (_, table_commitment_lists_per_commitment_scheme, _) =
            valid_statement_and_commitments();

        let statement_with_only_one_table = StatementAndAssociatedTableRefs::try_from(
            Parser::parse_sql(
                &GenericDialect {},
                "SELECT NONEXISTENT_COLUMN FROM NAMESPACE.TABLE1 JOIN NAMESPACE.TABLE2 ON NAMESPACE.TABLE1.INT_COL = NAMESPACE.TABLE2.INT_COL",
            )
            .unwrap()
            .pop()
            .unwrap(),
        )
        .unwrap();

        table_commitment_lists_per_commitment_scheme
            .map(ProofPlanForQueryAndCommitments(
                statement_with_only_one_table,
            ))
            .into_iter()
            .for_each(|any_result| {
                assert!(matches!(
                    any_result.unwrap(),
                    Err(CommitmentsApiError::Planner { .. })
                ));
            })
    }
}
