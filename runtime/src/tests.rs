use polkadot_sdk::frame_support::assert_noop;
use polkadot_sdk::frame_support::traits::Get;
use polkadot_sdk::pallet_staking::EraPayout;
use polkadot_sdk::sp_io::TestExternalities;
use polkadot_sdk::sp_runtime::traits::Zero;
use polkadot_sdk::sp_runtime::{DispatchError, Perbill};

use crate::{
    dynamic_params,
    AccountId,
    Balance,
    EraPayout as SXTPayout,
    Parameters,
    RuntimeOrigin,
    RuntimeParameters,
    DOLLARS,
};

fn set_per_diem_rate(rate: Perbill) {
    Parameters::set_parameter(
        RuntimeOrigin::root(),
        RuntimeParameters::Rewards(dynamic_params::rewards::Parameters::PerDiemRate(
            dynamic_params::rewards::PerDiemRate,
            Some(rate),
        )),
    )
    .unwrap();
}

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
fn per_diem_rate_defaults_to_9_7_percent_per_year() {
    TestExternalities::default().execute_with(|| {
        assert_eq!(
            dynamic_params::rewards::PerDiemRate::get(),
            Perbill::from_rational(97u64, 365_250u64)
        );
    });
}

#[test]
fn root_can_set_per_diem_rate() {
    TestExternalities::default().execute_with(|| {
        set_per_diem_rate(Perbill::from_percent(1));

        assert_eq!(
            dynamic_params::rewards::PerDiemRate::get(),
            Perbill::from_percent(1)
        );
    });
}

#[test]
fn non_root_cannot_set_per_diem_rate() {
    TestExternalities::default().execute_with(|| {
        assert_noop!(
            Parameters::set_parameter(
                RuntimeOrigin::signed(AccountId::from([1; 32])),
                RuntimeParameters::Rewards(dynamic_params::rewards::Parameters::PerDiemRate(
                    dynamic_params::rewards::PerDiemRate,
                    Some(Perbill::from_percent(1)),
                )),
            ),
            DispatchError::BadOrigin
        );
    });
}
