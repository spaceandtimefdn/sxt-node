#![cfg_attr(not(feature = "std"), no_std)]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use codec::{Decode, Encode, MaxEncodedLen};
use on_chain_table::OnChainTable;
use polkadot_sdk::frame_support::pallet_prelude::TypeInfo;
use polkadot_sdk::sp_core::U256;

use crate::tables::TableIdentifier;

/// Supported types of system requests, typically originating from data submissions
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum SystemRequestType {
    /// A message request (can be regular or funded)
    Message(MessageSystemRequest),
    /// A Staking related request
    Staking(StakingSystemRequest),
    /// A ZKpay related request
    ZkPay(ZKPayRequest),
}

#[derive(Clone, Copy, Eq, PartialEq)]
/// Types of supported message requests
pub enum MessageSystemRequest {
    /// A regular message (currently only used for session keys)
    Message,
    /// A funded message that includes target and amount fields
    FundedMessage,
}

#[derive(Clone, Copy, Eq, PartialEq)]
/// Types of supported staking requests
pub enum StakingSystemRequest {
    /// An address has staked some amount
    Stake,
    /// An already staked address is nominating one or more validators
    Nominate,
    /// An already staked address is initiating the unstaking process
    UnstakeInitiated,
    /// A user has completed an unstaking action, including the claim
    Unstaked,
    /// An existing unstaking request was cancelled
    UnstakeCancelled,
    /// An existing unstaking request that has waited the required unbonding period has been claimed
    UnstakeClaimed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, MaxEncodedLen, TypeInfo)]
/// Types of supported zkPay requests
pub enum ZKPayRequest {
    /// A new token has been added to zkPay
    AssetAdded,
    /// A token has been removed from zkPay
    AssetRemoved,
    /// A client contract's callback failed to execute
    CallbackFailed,
    /// A client contract's callback executed successfully
    CallbackSucceeded,
    /// The contract has been initialized (should only happen once)
    Initialized,
    /// A payment has been made by a client for a query
    NewQueryPayment,
    /// A query has been cancelled and a refund has been issued
    PaymentRefunded,
    /// Payment for a query has been settled
    PaymentSettled,
    /// A query has been cancelled by a client contract
    QueryCancelled,
    /// A query has been fulfilled
    QueryFulfilled,
    /// A client has requested a query
    QueryReceived,
    /// A client has sent payment, such as when buying compute credits
    SendPayment,
    /// The Treasury address has been set or updated.
    TreasurySet,
}

#[derive(Clone)]
/// A Wrapper for a system request parsed out of a data submission
pub struct SystemRequest {
    /// The type of request represented
    pub request_type: SystemRequestType,
    /// The fields of the corresponding event. In the case of templates these will have default
    /// values, but otherwise they will contain the values from the event
    pub fields: Vec<SystemTableField>,
    /// The Table Identifier of the SQL table corresponding to the event
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

/// Enumerated possible value types in System Event fields
#[derive(Clone)]
pub enum SystemFieldType {
    /// Varchar/Text
    Varchar,
    /// Variable Binary
    Bytes,
    /// SmallInt / i16
    SmallInt,
    /// Decimal of any scale or precision
    Decimal,
}

/// A wrapper for supported fields of system requests
#[derive(Clone)]
pub enum SystemFieldValue {
    /// Varchar/Text
    Varchar(String),
    /// Variable Binary
    Bytes(Vec<u8>),
    /// Decimal of any scale or precision
    Decimal(U256),
    /// SmallInt / i16
    SmallInt(i16),
}

/// A wrapper for a field/column containing multiple values from a request
#[derive(Clone)]
pub struct SystemTableField {
    /// The name of the field/Column
    pub name: String,
    /// The type of values in this field/column
    pub value_type: SystemFieldType,
    /// The values of this field/column, wrapped for easier handling
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

/// Creates a SystemRequest object with relevant fields based on the supplied template
/// and OnChainTable
pub fn parse_request_with_template(
    oc_table: OnChainTable,
    template: &SystemRequest,
) -> SystemRequest {
    let fields: Vec<SystemTableField> = template
        .fields
        .iter()
        .filter_map(|f| match f.value_type {
            SystemFieldType::Varchar => {
                oc_table
                    .get_varchars_by_column(&f.name)
                    .map(|data| SystemTableField {
                        name: f.name.clone(),
                        value_type: SystemFieldType::Varchar,
                        values: data
                            .iter()
                            .map(|v| SystemFieldValue::Varchar(v.clone()))
                            .collect(),
                    })
            }
            SystemFieldType::Decimal => {
                oc_table
                    .get_decimal_by_column(&f.name)
                    .map(|data| SystemTableField {
                        name: f.name.clone(),
                        value_type: SystemFieldType::Decimal,
                        values: data.iter().map(|v| SystemFieldValue::Decimal(*v)).collect(),
                    })
            }
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
                            .map(|v| SystemFieldValue::SmallInt(*v))
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
