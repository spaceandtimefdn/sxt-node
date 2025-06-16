use frame_support::pallet_prelude::DispatchResult;
use sxt_core::tables::TableIdentifier;

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::parse::SystemRequest;
use crate::parse::SystemRequestType::ZkPay;

#[derive(Clone, Copy, Eq, PartialEq)]
/// Types of supported zkPay requests
pub enum ZkPayRequest {
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

/// Returns the system templates for zkPay messages
pub fn getZkPayTemplates() -> Vec<SystemRequest> {
    vec![
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::AssetAdded),
            fields: vec![
                ("ASSET", crate::parse::SystemFieldType::Bytes).into(),
                ("ALLOWEDPAYMENTTYPES", crate::parse::SystemFieldType::Bytes).into(),
                ("PRICEFEED", crate::parse::SystemFieldType::Bytes).into(),
                ("TOKENDECIMALS", crate::parse::SystemFieldType::SmallInt).into(),
                (
                    "STALEPRICETHRESHOLDINSECONDS",
                    crate::parse::SystemFieldType::Decimal,
                )
                    .into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("ASSETADDED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::AssetRemoved),
            fields: vec![("ASSET", crate::parse::SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("ASSETREMOVED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::CallbackFailed),
            fields: vec![
                ("QUERYHASH", crate::parse::SystemFieldType::Bytes).into(),
                (
                    "CALLBACKCLIENTCONTRACTADDRESS",
                    crate::parse::SystemFieldType::Bytes,
                )
                    .into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("CALLBACKFAILED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::Initialized),
            fields: vec![("VERSION", crate::parse::SystemFieldType::Decimal).into()],
            table_id: TableIdentifier::from_str_unchecked("INITIALIZED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::NewQueryPayment),
            fields: vec![
                ("QUERYHASH", crate::parse::SystemFieldType::Bytes).into(),
                ("ASSET", crate::parse::SystemFieldType::Bytes).into(),
                ("AMOUNT", crate::parse::SystemFieldType::Decimal).into(),
                ("SOURCE_", crate::parse::SystemFieldType::Bytes).into(),
                ("AMOUNTINUSD", crate::parse::SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("NEWQUERYPAYMENT", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::PaymentRefunded),
            fields: vec![
                ("QUERYHASH", crate::parse::SystemFieldType::Bytes).into(),
                ("ASSET", crate::parse::SystemFieldType::Bytes).into(),
                ("SOURCE_", crate::parse::SystemFieldType::Bytes).into(),
                ("AMOUNT", crate::parse::SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("PAYMENTREFUNDED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::PaymentSettled),
            fields: vec![
                ("QUERYHASH", crate::parse::SystemFieldType::Bytes).into(),
                ("USEDAMOUNT", crate::parse::SystemFieldType::Decimal).into(),
                ("REMAININGAMOUNT", crate::parse::SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("PAYMENTSETTLED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::QueryCancelled),
            fields: vec![
                ("QUERYHASH", crate::parse::SystemFieldType::Bytes).into(),
                ("CALLER", crate::parse::SystemFieldType::Bytes).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("QUERYCANCELLED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::QueryFulfilled),
            fields: vec![("QUERYHASH", crate::parse::SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("QUERYFULFILLED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::QueryReceived),
            fields: vec![
                ("QUERYNONCE", crate::parse::SystemFieldType::Decimal).into(),
                ("SENDER", crate::parse::SystemFieldType::Bytes).into(),
                ("QUERY", crate::parse::SystemFieldType::Bytes).into(),
                ("QUERYPARAMETERS", crate::parse::SystemFieldType::Bytes).into(),
                ("TIMEOUT", crate::parse::SystemFieldType::Decimal).into(),
                (
                    "CALLBACKCLIENTCONTRACTADDRESS",
                    crate::parse::SystemFieldType::Bytes,
                )
                    .into(),
                ("CALLBACKGASLIMIT", crate::parse::SystemFieldType::Decimal).into(),
                ("CALLBACKDATA", crate::parse::SystemFieldType::Bytes).into(),
                (
                    "CUSTOMLOGICCONTRACTADDRESS",
                    crate::parse::SystemFieldType::Bytes,
                )
                    .into(),
                ("QUERYHASH", crate::parse::SystemFieldType::Bytes).into(),
                ("VERSION", crate::parse::SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("QUERYRECEIVED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::SendPayment),
            fields: vec![
                ("ASSET", crate::parse::SystemFieldType::Bytes).into(),
                ("AMOUNT", crate::parse::SystemFieldType::Decimal).into(),
                ("ONBEHALFOF", crate::parse::SystemFieldType::Bytes).into(),
                ("TARGET", crate::parse::SystemFieldType::Bytes).into(),
                ("MEMO", crate::parse::SystemFieldType::Bytes).into(),
                ("AMOUNTINUSD", crate::parse::SystemFieldType::Decimal).into(),
                ("SENDER", crate::parse::SystemFieldType::Bytes).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("SENDPAYMENT", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZkPayRequest::TreasurySet),
            fields: vec![("TREASURY", crate::parse::SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("TREASURYSET", "SXT_SYSTEM_ZKPAY"),
        },
    ]
}

pub fn process_zkpay_request<T: crate::Config>(request: SystemRequest) -> DispatchResult {
    match request.request_type {
        ZkPay(ZkPayRequest::AssetAdded) => {}
        ZkPay(ZkPayRequest::AssetRemoved) => {}
        ZkPay(ZkPayRequest::CallbackFailed) => {}
        ZkPay(ZkPayRequest::CallbackSucceeded) => {}
        ZkPay(ZkPayRequest::Initialized) => {}
        ZkPay(ZkPayRequest::NewQueryPayment) => {}
        ZkPay(ZkPayRequest::PaymentRefunded) => {}
        ZkPay(ZkPayRequest::PaymentSettled) => {}
        ZkPay(ZkPayRequest::QueryCancelled) => {}
        ZkPay(ZkPayRequest::QueryFulfilled) => {}
        ZkPay(ZkPayRequest::QueryReceived) => {}
        ZkPay(ZkPayRequest::SendPayment) => {}
        ZkPay(ZkPayRequest::TreasurySet) => {}
        _ => {}
    }
    Ok(())
}
