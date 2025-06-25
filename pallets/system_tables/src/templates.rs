#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use on_chain_table::OnChainTable;
use sxt_core::parse::SystemRequestType::{Message, Staking};
use sxt_core::parse::{StakingSystemRequest, SystemFieldType, SystemRequest};
use sxt_core::tables::TableIdentifier;

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
        .map(|template| sxt_core::parse::parse_request_with_template(oc_table, &template))
}

static SYSTEM_TEMPLATES: spin::Once<Vec<SystemRequest>> = spin::Once::new();
fn get_system_templates() -> &'static Vec<SystemRequest> {
    SYSTEM_TEMPLATES.call_once(|| {
        let out = vec![
            pallet_zkpay::templates::get_zkpay_templates(),
            get_staking_templates(),
        ];
        out.into_iter().flatten().collect()
    })
}

fn get_staking_templates() -> Vec<SystemRequest> {
    vec![
        SystemRequest {
            request_type: Message,
            fields: vec![
                ("SENDER", SystemFieldType::Bytes).into(),
                ("BODY", SystemFieldType::Bytes).into(),
                ("NONCE", SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("MESSAGE", "SXT_SYSTEM_STAKING"),
        },
        SystemRequest {
            request_type: Staking(StakingSystemRequest::Stake),
            fields: vec![
                ("STAKER", SystemFieldType::Bytes).into(),
                ("AMOUNT", SystemFieldType::Decimal).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("STAKED", "SXT_SYSTEM_STAKING"),
        },
        SystemRequest {
            request_type: Staking(StakingSystemRequest::Nominate),
            fields: vec![
                ("NOMINATOR", SystemFieldType::Bytes).into(),
                ("NODESED25519PUBKEYS", SystemFieldType::Varchar).into(),
            ],
            table_id: TableIdentifier::from_str_unchecked("NOMINATED", "SXT_SYSTEM_STAKING"),
        },
        SystemRequest {
            request_type: Staking(StakingSystemRequest::UnstakeInitiated),
            fields: vec![("STAKER", SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("UNSTAKEINITIATED", "SXT_SYSTEM_STAKING"),
        },
        SystemRequest {
            request_type: Staking(StakingSystemRequest::UnstakeCancelled),
            fields: vec![("STAKER", SystemFieldType::Bytes).into()],
            table_id: TableIdentifier::from_str_unchecked("UNSTAKECANCELLED", "SXT_SYSTEM_STAKING"),
        },
    ]
}
