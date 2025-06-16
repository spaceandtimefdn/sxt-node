#![cfg_attr(not(feature = "std"), no_std)]
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use on_chain_table::OnChainTable;
use sp_core::U256;
use sxt_core::tables::TableIdentifier;

use crate::parse::SystemFieldType::{Decimal, Varchar};
use crate::parse::SystemRequestType::{Message, Staking};
use crate::zkpay::{getZkPayTemplates, ZkPayRequest};

/// Supported types of system requests, typically originating from data submissions
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SystemRequestType {
    Message,
    Staking(StakingSystemRequest),
    ZkPay(ZkPayRequest),
}

#[derive(Clone, Copy, Eq, PartialEq)]
/// Types of supported staking requests
pub enum StakingSystemRequest {
    Stake,
    Nominate,
    UnstakeInitiated,
    UnstakeCancelled,
}

#[derive(Clone, Copy, Eq, PartialEq)]
/// Types of supported ZKpay requests
pub enum ZKPayRequest {
    AssetAdded,
    AssetRemoved,
    CallbackSucceeded,
    Initialized,
    NewQueryPayment,
    PaymentRefunded,
    PaymentSettled,
    QueryCancelled,
    QueryFulfilled,
    QueryReceived,
    SendPayment,
    TreasurySet,
}

#[derive(Clone)]
/// A Wrapper for a system request parsed out of a data submission
pub struct SystemRequest {
    pub request_type: SystemRequestType,
    pub fields: Vec<SystemTableField>,
    pub table_id: TableIdentifier,
}

impl SystemRequest {
    /// Retrieve the system request as discrete rows. Each row will reflect an intended modification
    /// to chain stake such as staking balance
    pub fn rows(&self) -> impl Iterator<Item = BTreeMap<String, SystemFieldValue>> + '_ {
        let min_len = self
            .fields
            .iter()
            .map(|field| field.values.len())
            .min()
            .unwrap_or(0);

        (0..min_len).map(move |i| {
            self.fields
                .iter()
                .map(|field| (field.name.clone(), field.values[i].clone()))
                .collect::<BTreeMap<String, SystemFieldValue>>()
        })
    }
}

#[derive(Clone)]
pub(crate) enum SystemFieldType {
    Varchar,
    Bytes,
    SmallInt,
    Decimal,
}

/// A wrapper for supported fields of system requests
#[derive(Clone)]
pub enum SystemFieldValue {
    Varchar(String),
    Bytes(Vec<u8>),
    Decimal(U256),
    SmallInt(i16),
}

/// A wrapper for a field/column containing multiple values from a request
#[derive(Clone)]
pub struct SystemTableField {
    pub name: String,
    pub value_type: SystemFieldType,
    pub values: Vec<SystemFieldValue>,
}

impl SystemTableField {
    /// Returns a System Table Field with the given value and name. Useful for tests
    pub fn with_value(name: String, value: SystemFieldValue) -> Self {
        let value_type = match value {
            SystemFieldValue::Varchar(_) => SystemFieldType::Varchar,
            SystemFieldValue::Bytes(_) => SystemFieldType::Bytes,
            SystemFieldValue::Decimal(_) => SystemFieldType::Decimal,
            SystemFieldValue::SmallInt(_) => SystemFieldType::SmallInt,
        };

        SystemTableField {
            name,
            value_type,
            values: vec![value],
        }
    }
}

impl From<(&str, SystemFieldType)> for SystemTableField {
    fn from((value, value_type): (&str, SystemFieldType)) -> Self {
        SystemTableField {
            name: String::from(value),
            value_type,
            values: vec![],
        }
    }
}

static SYSTEM_TEMPLATES: spin::Once<Vec<SystemRequest>> = spin::Once::new();
fn get_system_templates() -> &'static Vec<SystemRequest> {
    SYSTEM_TEMPLATES.call_once(|| {
        let out = vec![
            getZkPayTemplates(),
            vec![
                SystemRequest {
                    request_type: Message,
                    fields: vec![
                        ("SENDER", SystemFieldType::Bytes).into(),
                        ("BODY", SystemFieldType::Bytes).into(),
                        ("NONCE", Decimal).into(),
                    ],
                    table_id: TableIdentifier::from_str_unchecked("MESSAGE", "SXT_SYSTEM_STAKING"),
                },
                SystemRequest {
                    request_type: Staking(StakingSystemRequest::Stake),
                    fields: vec![
                        ("STAKER", SystemFieldType::Bytes).into(),
                        ("AMOUNT", Decimal).into(),
                    ],
                    table_id: TableIdentifier::from_str_unchecked("STAKED", "SXT_SYSTEM_STAKING"),
                },
                SystemRequest {
                    request_type: Staking(StakingSystemRequest::Nominate),
                    fields: vec![
                        ("NOMINATOR", SystemFieldType::Bytes).into(),
                        ("NODESED25519PUBKEYS", Varchar).into(),
                    ],
                    table_id: TableIdentifier::from_str_unchecked(
                        "NOMINATED",
                        "SXT_SYSTEM_STAKING",
                    ),
                },
                SystemRequest {
                    request_type: Staking(StakingSystemRequest::UnstakeInitiated),
                    fields: vec![("STAKER", SystemFieldType::Bytes).into()],
                    table_id: TableIdentifier::from_str_unchecked(
                        "UNSTAKEINITIATED",
                        "SXT_SYSTEM_STAKING",
                    ),
                },
                SystemRequest {
                    request_type: Staking(StakingSystemRequest::UnstakeCancelled),
                    fields: vec![("STAKER", SystemFieldType::Bytes).into()],
                    table_id: TableIdentifier::from_str_unchecked(
                        "UNSTAKECANCELLED",
                        "SXT_SYSTEM_STAKING",
                    ),
                },
            ],
        ];
        out.into_iter().flatten().collect()
    })
}

/// Creates a SystemRequest object with relevant fields based on the supplied template
/// and OnChainTable
fn parse_request_with_template(oc_table: OnChainTable, template: &SystemRequest) -> SystemRequest {
    let fields: Vec<SystemTableField> = template
        .fields
        .iter()
        .filter_map(|f| match f.value_type {
            Varchar => oc_table
                .get_varchars_by_column(&f.name)
                .map(|data| SystemTableField {
                    name: f.name.clone(),
                    value_type: Varchar,
                    values: data
                        .iter()
                        .map(|v| SystemFieldValue::Varchar(v.clone()))
                        .collect(),
                }),
            Decimal => oc_table
                .get_decimal_by_column(&f.name)
                .map(|data| SystemTableField {
                    name: f.name.clone(),
                    value_type: Decimal,
                    values: data.iter().map(|v| SystemFieldValue::Decimal(*v)).collect(),
                }),
            SystemFieldType::Bytes => {
                oc_table
                    .get_bytes_by_column(&f.name)
                    .map(|data| SystemTableField {
                        name: f.name.clone(),
                        value_type: SystemFieldType::Bytes,
                        values: data
                            .iter()
                            .map(|v| SystemFieldValue::Bytes(v.clone()))
                            .collect(),
                    })
            }
            SystemFieldType::SmallInt => {
                oc_table
                    .get_smallints_by_column(&f.name)
                    .map(|data| SystemTableField {
                        name: f.name.clone(),
                        value_type: SystemFieldType::SmallInt,
                        values: data
                            .iter()
                            .map(|v| SystemFieldValue::SmallInt(v.clone()))
                            .collect(),
                    })
            }
        })
        .collect();

    SystemRequest {
        request_type: template.request_type,
        table_id: template.table_id.clone(),
        fields,
    }
}

/// Returns the parsing template for the provided table identifier. The template is a
/// SystemRequest that contains the corresponding SystemTableFields with no values.
pub fn template_for_identifier(table_identifier: TableIdentifier) -> Option<SystemRequest> {
    for t in get_system_templates() {
        if t.table_id == table_identifier {
            return Some(t.clone());
        }
    }
    None
}

/// Converts a given OnChainTable into a SystemRequest object
pub fn table_to_request(
    oc_table: OnChainTable,
    table_identifier: TableIdentifier,
) -> Option<SystemRequest> {
    template_for_identifier(table_identifier)
        .map(|template| parse_request_with_template(oc_table, &template))
}
