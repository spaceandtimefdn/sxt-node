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
use tokio::net::TcpListener;

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

    /// Annualized Reward rate tracker
    pub static ref REWARD_RATE: GaugeVec = register_gauge_vec!(
        "canary_era_reward_rate",
        "Annualized staking reward rate per era",
        &["era"]
    ).unwrap();

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
}

/// Add a count for the given event name
pub(crate) fn record_event(label: &str) {
    EVENT_COUNTER.with_label_values(&[label]).inc_by(1);
}

/// Add the provided amount to the metric for the provided label
pub(crate) fn record_staking(label: &str, amount: u128) {
    use subxt::ext::sp_runtime::SaturatedConversion;
    STAKING_COUNTER
        .with_label_values(&[label])
        .inc_by(amount.saturated_into());
}

/// Add the provided amount to the metric for the provided label
pub(crate) fn record_balance(label: &str, amount: u128) {
    use subxt::ext::sp_runtime::SaturatedConversion;
    BALANCE_COUNTER
        .with_label_values(&[label])
        .inc_by(amount.saturated_into());
}
