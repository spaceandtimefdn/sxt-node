use pallet_staking::EraPayout;
use sp_runtime::traits::Zero;

use crate::{
    Balance,
    EraPayout as SXTPayout,
    SessionsPerEra,
    DOLLARS,
    EPOCH_DURATION_IN_BLOCKS,
    MILLISECS_PER_BLOCK,
};

#[test]
fn era_payout_calculation_works() {
    let test_staked: Balance = Balance::from(100 * DOLLARS);
    let test_issued: Balance = Balance::from(1000 * DOLLARS);

    // Test one session in an era
    const MILLISECONDS_PER_YEAR: u64 = (1000 * 3600 * 24 * 36525) / 100;

    // One day of Milliseconds
    let test_ms_per_era = 1000 * 3600 * 24;

    let (to_stakers, to_treasury) =
        SXTPayout::era_payout(test_staked, test_issued, test_ms_per_era);
    assert_eq!(to_treasury, Balance::zero());

    let single_era_payout = Balance::from(21629021218343597u128);
    assert_eq!(to_stakers, single_era_payout);
}
