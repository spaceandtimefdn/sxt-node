#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use sxt_core::parse::SystemRequestType::ZkPay;
use sxt_core::parse::{SystemFieldType, SystemRequest, ZKPayRequest};
use sxt_core::tables::TableIdentifier;

/// Returns the system templates for zkPay messages
pub fn get_zkpay_templates() -> Vec<SystemRequest> {
    vec![
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::AssetAdded),
            fields: vec![
                ("ASSET", SystemFieldType::Bytes).into(),
                ("ALLOWEDPAYMENTTYPES", SystemFieldType::Bytes).into(),
                ("PRICEFEED", SystemFieldType::Bytes).into(),
                ("TOKENDECIMALS", SystemFieldType::SmallInt).into(),
                ("STALEPRICETHRESHOLDINSECONDS", SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("ASSETADDED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::AssetRemoved),
            fields: vec![("ASSET", SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("ASSETREMOVED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::CallbackFailed),
            fields: vec![
                ("QUERYHASH", SystemFieldType::Bytes).into(),
                ("CALLBACKCLIENTCONTRACTADDRESS", SystemFieldType::Bytes).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("CALLBACKFAILED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::CallbackFailed),
            fields: vec![
                ("QUERYHASH", SystemFieldType::Bytes).into(),
                ("CALLBACKCLIENTCONTRACTADDRESS", SystemFieldType::Bytes).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("CALLBACKSUCCEEDED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::Initialized),
            fields: vec![("VERSION", SystemFieldType::Decimal).into()],
            table_id: TableIdentifier::from_str_unchecked("INITIALIZED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::NewQueryPayment),
            fields: vec![
                ("QUERYHASH", SystemFieldType::Bytes).into(),
                ("ASSET", SystemFieldType::Bytes).into(),
                ("AMOUNT", SystemFieldType::Decimal).into(),
                ("SOURCE_", SystemFieldType::Bytes).into(),
                ("AMOUNTINUSD", SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("NEWQUERYPAYMENT", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::PaymentRefunded),
            fields: vec![
                ("QUERYHASH", SystemFieldType::Bytes).into(),
                ("ASSET", SystemFieldType::Bytes).into(),
                ("SOURCE_", SystemFieldType::Bytes).into(),
                ("AMOUNT", SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("PAYMENTREFUNDED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::PaymentSettled),
            fields: vec![
                ("QUERYHASH", SystemFieldType::Bytes).into(),
                ("USEDAMOUNT", SystemFieldType::Decimal).into(),
                ("REMAININGAMOUNT", SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("PAYMENTSETTLED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::QueryCancelled),
            fields: vec![
                ("QUERYHASH", SystemFieldType::Bytes).into(),
                ("CALLER", SystemFieldType::Bytes).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("QUERYCANCELLED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::QueryFulfilled),
            fields: vec![("QUERYHASH", SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("QUERYFULFILLED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::QueryReceived),
            fields: vec![
                ("QUERYNONCE", SystemFieldType::Decimal).into(),
                ("SENDER", SystemFieldType::Bytes).into(),
                ("QUERY", SystemFieldType::Bytes).into(),
                ("QUERYPARAMETERS", SystemFieldType::Bytes).into(),
                ("TIMEOUT", SystemFieldType::Decimal).into(),
                ("CALLBACKCLIENTCONTRACTADDRESS", SystemFieldType::Bytes).into(),
                ("CALLBACKGASLIMIT", SystemFieldType::Decimal).into(),
                ("CALLBACKDATA", SystemFieldType::Bytes).into(),
                ("CUSTOMLOGICCONTRACTADDRESS", SystemFieldType::Bytes).into(),
                ("QUERYHASH", SystemFieldType::Bytes).into(),
                ("VERSION", SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("QUERYRECEIVED", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::SendPayment),
            fields: vec![
                ("ASSET", SystemFieldType::Bytes).into(),
                ("AMOUNT", SystemFieldType::Decimal).into(),
                ("ONBEHALFOF", SystemFieldType::Bytes).into(),
                ("TARGET", SystemFieldType::Bytes).into(),
                ("MEMO", SystemFieldType::Bytes).into(),
                ("AMOUNTINUSD", SystemFieldType::Decimal).into(),
                ("SENDER", SystemFieldType::Bytes).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("SENDPAYMENT", "SXT_SYSTEM_ZKPAY"),
        },
        SystemRequest {
            request_type: ZkPay(ZKPayRequest::TreasurySet),
            fields: vec![("TREASURY", SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("TREASURYSET", "SXT_SYSTEM_ZKPAY"),
        },
    ]
}
