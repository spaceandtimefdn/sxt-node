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

    let single_era_payout = Balance::from(21629021218343597u128);
    assert_eq!(to_stakers, single_era_payout);
}
