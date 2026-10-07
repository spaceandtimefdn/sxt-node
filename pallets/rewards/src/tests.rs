use polkadot_sdk::frame_support::traits::{Currency, OnUnbalanced};

use crate::mock::{new_test_ext, Balances, Test};
use crate::{CollectedFees, DealWithFees};

#[test]
fn staking_rewards_pay_out() {
    new_test_ext().execute_with(|| {});
}

#[test]
fn unbalanced_fees_accumulate_in_collected_fees() {
    new_test_ext().execute_with(|| {
        assert_eq!(CollectedFees::<Test>::get(), 0);

        DealWithFees::<Test>::on_unbalanced(Balances::issue(100));
        DealWithFees::<Test>::on_unbalanceds([Balances::issue(20), Balances::issue(3)].into_iter());

        assert_eq!(CollectedFees::<Test>::get(), 123);
    });
}

#[test]
fn collected_fees_saturate() {
    new_test_ext().execute_with(|| {
        CollectedFees::<Test>::put(u128::MAX - 1);

        DealWithFees::<Test>::on_unbalanced(Balances::issue(5));

        assert_eq!(CollectedFees::<Test>::get(), u128::MAX);
    });
}
