//! Strategies for generating on-chain-tables for use in tests.

use alloc::vec::Vec;

use arrow::datatypes::i256;
use proof_of_sql::base::database::ColumnType;
use proof_of_sql::base::math::decimal::Precision;
use proof_of_sql::base::posql_time::{PoSQLTimeUnit, PoSQLTimeZone};
use proptest::prelude::*;
use proptest::sample::SizeRange;
use proptest::string::string_regex;
use sqlparser::ast::Ident;

use crate::i256_conversion::arrow_i256_to_u256;
use crate::{OnChainColumn, OnChainTable};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofOfSqlSchema(Vec<(Ident, ColumnType)>);

#[derive(Debug)]
pub struct NoColumns;

impl ProofOfSqlSchema {
    pub fn try_from_iter(
        iter: impl IntoIterator<Item = (Ident, ColumnType)>,
    ) -> Result<ProofOfSqlSchema, NoColumns> {
        let schema = iter.into_iter().collect::<Vec<_>>();

        if schema.is_empty() {
            Err(NoColumns)
        } else {
            Ok(ProofOfSqlSchema(schema))
        }
    }

    fn as_slice(&self) -> &[(Ident, ColumnType)] {
        &self.0
    }

    fn into_vec(self) -> Vec<(Ident, ColumnType)> {
        self.0
    }
}

prop_compose! {
    pub fn ident()(value in "[a-zA-Z_][a-zA-Z0-9_]{0,63}") -> Ident {
        Ident::new(value)
    }
}

prop_compose! {
    pub fn decimal_75_column_type()(scale in 0u8..=74)
        (precision in (scale + 1)..=75, scale in Just(scale)) -> ColumnType {
        ColumnType::Decimal75(Precision::new(precision).expect("precision is < 75"), scale as i8)
    }
}

fn supported_column_type() -> impl Strategy<Value = ColumnType> {
    prop_oneof![
        Just(ColumnType::Boolean),
        Just(ColumnType::TinyInt),
        Just(ColumnType::SmallInt),
        Just(ColumnType::Int),
        Just(ColumnType::BigInt),
        Just(ColumnType::VarChar),
        Just(ColumnType::VarBinary),
        Just(ColumnType::TimestampTZ(
            PoSQLTimeUnit::Millisecond,
            PoSQLTimeZone::utc()
        )),
        decimal_75_column_type(),
    ]
}

pub fn proof_of_sql_schema<NC>(num_columns: NC) -> impl Strategy<Value = ProofOfSqlSchema>
where
    NC: Strategy<Value = usize>,
{
    num_columns
        .prop_flat_map(|num_columns| {
            proptest::collection::vec((ident(), supported_column_type()), num_columns.max(1))
        })
        .prop_map(|schema| {
            ProofOfSqlSchema::try_from_iter(schema)
                .expect("previous strategy guarantees schema has at least one column")
        })
}

fn i256_with_max_num_digits(max_num_digits: u8) -> impl Strategy<Value = i256> {
    let regex = format!("[+-]?[0-9]{{1,{max_num_digits}}}");

    string_regex(&regex)
        .expect("regex should be valid")
        .prop_map(|string| i256::from_string(&string).expect("regex should produce a valid i256"))
}

prop_compose! {
    pub fn i256()
        (low in any::<u128>(), high in any::<i128>()) -> i256 {
        i256::from_parts(low, high)
    }
}

pub fn decimal_75_column<E>(
    precision: Precision,
    scale: i8,
    element: E,
    num_rows: impl Into<SizeRange>,
) -> impl Strategy<Value = OnChainColumn>
where
    E: Strategy<Value = i256>,
{
    proptest::collection::vec(element, num_rows).prop_map(move |i256_col| {
        let u256_col = i256_col.into_iter().map(arrow_i256_to_u256).collect();
        OnChainColumn::Decimal75(precision, scale, u256_col)
    })
}

fn on_chain_column<CT, NR>(column_type: CT, num_rows: NR) -> impl Strategy<Value = OnChainColumn>
where
    CT: Strategy<Value = ColumnType>,
    NR: Into<SizeRange> + Clone + 'static,
{
    column_type.prop_flat_map(move |column_type| match column_type {
        ColumnType::Boolean => proptest::collection::vec(any::<bool>(), num_rows.clone())
            .prop_map(OnChainColumn::Boolean)
            .boxed(),
        ColumnType::Uint8 => proptest::collection::vec(any::<u8>(), num_rows.clone())
            .prop_map(OnChainColumn::UnsignedTinyInt)
            .boxed(),
        ColumnType::TinyInt => proptest::collection::vec(any::<i8>(), num_rows.clone())
            .prop_map(OnChainColumn::TinyInt)
            .boxed(),
        ColumnType::SmallInt => proptest::collection::vec(any::<i16>(), num_rows.clone())
            .prop_map(OnChainColumn::SmallInt)
            .boxed(),
        ColumnType::Int => proptest::collection::vec(any::<i32>(), num_rows.clone())
            .prop_map(OnChainColumn::Int)
            .boxed(),
        ColumnType::BigInt => proptest::collection::vec(any::<i64>(), num_rows.clone())
            .prop_map(OnChainColumn::BigInt)
            .boxed(),
        ColumnType::Int128 => proptest::collection::vec(any::<i128>(), num_rows.clone())
            .prop_map(OnChainColumn::Int128)
            .boxed(),
        ColumnType::VarChar => proptest::collection::vec(".{0,256}", num_rows.clone())
            .prop_map(OnChainColumn::VarChar)
            .boxed(),
        ColumnType::VarBinary => proptest::collection::vec(
            proptest::collection::vec(any::<u8>(), 0..256),
            num_rows.clone(),
        )
        .prop_map(OnChainColumn::VarBinary)
        .boxed(),
        ColumnType::TimestampTZ(time_unit, time_zone) => {
            proptest::collection::vec(any::<i64>(), num_rows.clone())
                .prop_map(move |values| {
                    OnChainColumn::TimestampTZ(time_unit, Some(time_zone), values)
                })
                .boxed()
        }
        ColumnType::Decimal75(precision, scale) => decimal_75_column(
            precision,
            scale,
            i256_with_max_num_digits(precision.value()),
            num_rows.clone(),
        )
        .boxed(),
        _ => unimplemented!(),
    })
}

pub fn on_chain_table<S, NR>(schema: S, num_rows: NR) -> impl Strategy<Value = OnChainTable>
where
    S: Strategy<Value = ProofOfSqlSchema>,
    NR: Strategy<Value = usize>,
{
    (schema, num_rows)
        .prop_flat_map(|(schema, num_rows)| {
            schema
                .into_vec()
                .into_iter()
                .map(|(ident, column_type)| {
                    (
                        Just(ident),
                        on_chain_column(Just(column_type), num_rows..=num_rows),
                    )
                })
                .collect::<Vec<_>>()
        })
        .prop_map(|columns| {
            OnChainTable::try_from_iter(columns).expect(
                "type guarantees are held by ProofOfSqlSchema and the on chain column strategy",
            )
        })
}
