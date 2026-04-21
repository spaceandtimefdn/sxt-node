use polkadot_sdk::pallet_staking::EraPayout;
use polkadot_sdk::sp_runtime::traits::Zero;

use crate::{Balance, EraPayout as SXTPayout, DOLLARS};

#[test]
fn era_payout_calculation_works() {
    let test_staked: Balance = Balance::from(100 * DOLLARS);
    let test_issued: Balance = Balance::from(1000 * DOLLARS);

    // One day of Milliseconds
    let test_ms_per_era = 1000 * 3600 * 24;

    let (to_stakers, to_treasury) =
        SXTPayout::era_payout(test_staked, test_issued, test_ms_per_era);
    assert_eq!(to_treasury, Balance::zero());

    let single_era_payout = Balance::from(26557152635181379u128);
    assert_eq!(to_stakers, single_era_payout);
}

#[test]
fn dynamic_quorum_reached_index_resolves() {
    use polkadot_sdk::frame_support::traits::Get;
    // If `pallet_indexing::Event::QuorumReached` is ever renamed or removed,
    // this test panics at `cargo test` time instead of failing silently in
    // the block-forwarder at runtime.
    let idx = crate::DynamicQuorumReachedIndex::get();
    // The lookup must succeed (get() panics otherwise). We don't assert a
    // specific index — reordering the enum is fine, renaming it is not.
    assert!(idx < u8::MAX, "sanity-check: variant index should be well-defined");
}
