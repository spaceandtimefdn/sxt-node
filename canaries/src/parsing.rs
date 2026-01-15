use event_forwarder::block_processing::filter_events;
use subxt::events::EventDetails;
use subxt::PolkadotConfig;

/// Returns a list of names for the provided events, with one name entry per provided event
pub(crate) fn parse_event_names(events: &[EventDetails<PolkadotConfig>]) -> Vec<String> {
    events
        .iter()
        .map(|e| format!("{:?}-{:?}", e.pallet_name(), e.variant_name()))
        .collect::<Vec<_>>()
}

/// A staking event with its label and amount.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) struct StakingEvent<'a> {
    /// The event variant name.
    pub label: &'a str,
    /// The staking amount.
    pub amount: u128,
}

impl<'a> From<(&'a str, u128)> for StakingEvent<'a> {
    fn from((label, amount): (&'a str, u128)) -> Self {
        StakingEvent { label, amount }
    }
}

/// Parses a given Vec of events, returning a tuple of variant name and amount for staking events
pub(crate) fn parse_staking_stats(
    events: &[EventDetails<PolkadotConfig>],
) -> Vec<StakingEvent<'_>> {
    use sxt_core::sxt_chain_runtime::api::staking::events::{
        Bonded,
        Rewarded,
        Slashed,
        Unbonded,
        Withdrawn,
    };

    events
        .iter()
        .filter_map(|event| {
            let name = event.variant_name();
            if event.pallet_name().contains("Staking") {
                if let Ok(Some(e)) = event.as_event::<Bonded>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Unbonded>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Withdrawn>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Rewarded>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Slashed>() {
                    return Some((name, e.amount).into());
                }
            }
            None
        })
        .collect::<Vec<_>>()
}

/// A balance event with its label and amount.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) struct BalanceEvent<'a> {
    /// The event variant name.
    pub label: &'a str,
    /// The balance amount.
    pub amount: u128,
}

impl<'a> From<(&'a str, u128)> for BalanceEvent<'a> {
    fn from((label, amount): (&'a str, u128)) -> Self {
        BalanceEvent { label, amount }
    }
}

/// Parses a given Vec of events, returning a tuple of variant name and amount for balance events
pub(crate) fn parse_balance_stats(
    events: &[EventDetails<PolkadotConfig>],
) -> Vec<BalanceEvent<'_>> {
    use sxt_core::sxt_chain_runtime::api::balances::events::{
        Burned,
        Frozen,
        Issued,
        Locked,
        Minted,
        Rescinded,
        Reserved,
        Thawed,
        Transfer,
        Unlocked,
        Unreserved,
    };

    events
        .iter()
        .filter_map(|event| {
            if event.pallet_name().contains("Balances") {
                let name = event.variant_name();
                if let Ok(Some(e)) = event.as_event::<Issued>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Burned>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Unreserved>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Frozen>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Thawed>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Locked>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Minted>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Reserved>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Transfer>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Unlocked>() {
                    return Some((name, e.amount).into());
                } else if let Ok(Some(e)) = event.as_event::<Rescinded>() {
                    return Some((name, e.amount).into());
                }
            }
            None
        })
        .collect::<Vec<_>>()
}

/// Returns true if the provided event list has a NewSession event in it
pub(crate) fn has_new_session(events: &[EventDetails<PolkadotConfig>]) -> bool {
    use sxt_core::sxt_chain_runtime::api::session::events::NewSession;
    !filter_events::<NewSession>(events).is_empty()
}
