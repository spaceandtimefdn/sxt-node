use std::net::SocketAddr;

use axum::routing::get;
use axum::Router;
use lazy_static::lazy_static;
use prometheus::{
    register_gauge_vec,
    register_int_counter_vec,
    Encoder,
    GaugeVec,
    IntCounterVec,
    TextEncoder,
};
use subxt::utils::AccountId32;
use sxt_runtime::DOLLARS;
use tokio::net::TcpListener;

use crate::parsing::{BalanceEvent, StakingEvent};
use crate::rpc::AttestationInfo;

/// Serve prometheus metrics
pub async fn serve_metrics(bind_addr: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new().route("/metrics", get(metrics_handler));

    let listener = TcpListener::bind(bind_addr).await?;
    log::info!(
        "📊 Prometheus metrics server running on http://{}",
        bind_addr
    );

    axum::serve(listener, app).await?;
    Ok(())
}
async fn metrics_handler() -> String {
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

lazy_static! {
    /// Prometheus event counter
    pub static ref EVENT_COUNTER: IntCounterVec = register_int_counter_vec!(
        "canary_event_total",
        "Total count of specific events observed in finalized blocks",
        &["type"]
    )
    .unwrap();

    /// Total staked tracker
    pub static ref TOTAL_STAKED: GaugeVec = register_gauge_vec!(
        "canary_era_total_staked",
        "Total stake observed for a given era",
        &["era"]
    ).unwrap();
    /// Validator reward tracker
    pub static ref VALIDATOR_REWARD: GaugeVec = register_gauge_vec!(
        "canary_era_validator_reward",
        "Total validator rewards distributed in an era",
        &["era"]
    ).unwrap();

    /// Annaulizer tracker, used for debugging
    pub static ref ANNUALIZER: GaugeVec = register_gauge_vec!(
        "canary_era_annualizer_multiplier",
        "Annualizer multiplier used in APR calculation",
        &["era"]
    ).unwrap();

    pub static ref BALANCE_COUNTER: IntCounterVec = register_int_counter_vec!(
        "canary_balance_events",
        "Total of amounts of Balance pallet events",
        &["type"]
    ).unwrap();
    pub static ref STAKING_COUNTER: IntCounterVec = register_int_counter_vec!(
        "canary_staking_events",
        "Total of amounts of Balance pallet events",
        &["type"]
    ).unwrap();
    pub static ref UNSTAKE_CLAIMED_COUNTER: IntCounterVec = register_int_counter_vec!(
        "canary_unstake_claimed_counts",
        "Total of unstake claims",
        &["block_number"]
    ).unwrap();
    pub static ref BEST_ATTESTATION_COUNTER: IntCounterVec = register_int_counter_vec!(
        "canary_unstake_best_attestation_count",
        "Total of attestations for the 'bestAttestations' RPC",
        &["block_number"]
    ).unwrap();

    pub static ref ATTESTATION_COUNT_BY_ID: IntCounterVec = register_int_counter_vec!(
        "canary_attestations_per_id",
        "A count of attestations by Attestor public address",
        &["account_id"]
    ).unwrap();

    pub static ref FREE_BALANCE_BY_ID: GaugeVec = register_gauge_vec!(
        "canary_free_balance_per_id",
        "The free balance of a given account",
        &["account_id"]
    ).unwrap();
}

/// Add a count for the given event name
pub(crate) fn record_event(label: &str) {
    EVENT_COUNTER.with_label_values(&[label]).inc_by(1);
}

/// Add the provided amount to the metric for the provided label
pub(crate) fn record_staking(e: &StakingEvent) {
    use subxt::ext::sp_runtime::SaturatedConversion;
    STAKING_COUNTER
        .with_label_values(&[e.label])
        .inc_by(e.amount.saturated_into());
}

/// Records attestation metrics for a given block.
pub(crate) fn record_attestations(block_number: u32, attestations: Vec<AttestationInfo>) {
    // Count the total attestations for this block
    BEST_ATTESTATION_COUNTER
        .with_label_values(&[block_number.to_string()])
        .inc_by(attestations.len() as u64);

    // Count the attestations for this block by signer for better granularity
    attestations.iter().for_each(|a: &AttestationInfo| {
        ATTESTATION_COUNT_BY_ID
            .with_label_values(&[&a.address20])
            .inc_by(1);
    });
}

/// Add the provided amount to the metric for the provided label
pub(crate) fn record_balance(e: &BalanceEvent) {
    use subxt::ext::sp_runtime::SaturatedConversion;
    BALANCE_COUNTER
        .with_label_values(&[e.label])
        .inc_by(e.amount.saturated_into());
}

/// Records the count of claimed unstakes for a given block.
pub(crate) fn record_claimed_unstake_count(block_number: u32, count: u64) {
    UNSTAKE_CLAIMED_COUNTER
        .with_label_values(&[block_number.to_string()])
        .inc_by(count);
}

pub(crate) fn record_watchlist(watchlist_balances: Vec<(AccountId32, u128)>) {
    for (acct, balance) in watchlist_balances {
        // Prometheus only supports 64-bit floats for metrics; convert to tokens as f64
        // by dividing the raw balance (u128) by DOLLARS as f64. This preserves fractional tokens.
        let balance_in_tokens: f64 = (balance as f64) / (DOLLARS as f64);
        FREE_BALANCE_BY_ID
            .with_label_values(&[acct.to_string().as_str()])
            .set(balance_in_tokens);
    }
}

/// Records the total validator rewards for a given era.
pub(crate) fn record_era_rewards(era: u32, amount: u128) {
    VALIDATOR_REWARD
        .with_label_values(&[era.to_string()])
        .set(amount as f64);
}

/// Records the total staked amount for a given era.
pub(crate) fn record_era_total_stake(era: u32, amount: u128) {
    TOTAL_STAKED
        .with_label_values(&[era.to_string()])
        .set(amount as f64);
}
