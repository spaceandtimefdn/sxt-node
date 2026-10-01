use codec::Encode;
use pallet_rewards::CollectedFees;
use polkadot_sdk::frame_support::traits::fungible::Mutate;
use polkadot_sdk::frame_support::traits::Get;
use polkadot_sdk::frame_support::{assert_noop, assert_ok};
use polkadot_sdk::pallet_staking::EraPayout;
use polkadot_sdk::pallet_transaction_payment::{Config, OnChargeTransaction};
use polkadot_sdk::sp_core::{sr25519, Pair};
use polkadot_sdk::sp_io::TestExternalities;
use polkadot_sdk::sp_runtime::generic::Era;
use polkadot_sdk::sp_runtime::traits::Zero;
use polkadot_sdk::sp_runtime::{DispatchError, DispatchResult, Perbill};
use polkadot_sdk::{frame_system, pallet_transaction_payment};

use crate::{
    dynamic_params,
    AccountId,
    Balance,
    Balances,
    BuildStorage,
    EraPayout as SXTPayout,
    Executive,
    Parameters,
    Runtime,
    RuntimeCall,
    RuntimeOrigin,
    RuntimeParameters,
    SignedPayload,
    UncheckedExtrinsic,
    DOLLARS,
    MILLISECONDS_PER_DAY,
};

fn set_per_diem_rate(origin: RuntimeOrigin, rate: Perbill) -> DispatchResult {
    Parameters::set_parameter(
        origin,
        RuntimeParameters::Rewards(dynamic_params::rewards::Parameters::PerDiemRate(
            dynamic_params::rewards::PerDiemRate,
            Some(rate),
        )),
    )
}

fn set_per_diem_rate_as_root(rate: Perbill) {
    assert_ok!(set_per_diem_rate(RuntimeOrigin::root(), rate));
}

#[test]
fn era_payout_defaults_to_yearly_rate_of_9_7_percent() {
    TestExternalities::default().execute_with(|| {
        let (payout, rest) =
            SXTPayout::era_payout(365_250 * DOLLARS, 1000 * DOLLARS, MILLISECONDS_PER_DAY);
        assert_eq!(rest, Balance::zero());
        assert!(payout.abs_diff(97 * DOLLARS) <= 97 * DOLLARS / 100_000);
    });
}

#[test]
fn era_payout_pays_per_diem_rate_of_total_stake() {
    TestExternalities::default().execute_with(|| {
        set_per_diem_rate_as_root(Perbill::from_percent(1));

        assert_eq!(
            SXTPayout::era_payout(100 * DOLLARS, 1000 * DOLLARS, MILLISECONDS_PER_DAY),
            (DOLLARS, Balance::zero())
        );
        assert_eq!(
            SXTPayout::era_payout(100 * DOLLARS, 1000 * DOLLARS, MILLISECONDS_PER_DAY / 2),
            (DOLLARS / 2, Balance::zero())
        );
        assert_eq!(
            SXTPayout::era_payout(100 * DOLLARS, 1000 * DOLLARS, 7 * MILLISECONDS_PER_DAY),
            (7 * DOLLARS, Balance::zero())
        );
    });
}

#[test]
fn era_payout_includes_and_drains_collected_fees() {
    TestExternalities::default().execute_with(|| {
        set_per_diem_rate_as_root(Perbill::from_percent(1));
        CollectedFees::<Runtime>::put(5 * DOLLARS);

        assert_eq!(
            SXTPayout::era_payout(100 * DOLLARS, 1000 * DOLLARS, MILLISECONDS_PER_DAY),
            (6 * DOLLARS, Balance::zero())
        );
        assert_eq!(CollectedFees::<Runtime>::get(), Balance::zero());
        assert_eq!(
            SXTPayout::era_payout(100 * DOLLARS, 1000 * DOLLARS, MILLISECONDS_PER_DAY),
            (DOLLARS, Balance::zero())
        );
    });
}

#[test]
fn transaction_fees_increase_collected_fees() {
    let storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .unwrap();
    TestExternalities::new(storage).execute_with(|| {
        let pair = sr25519::Pair::from_seed(&[1; 32]);
        let who = AccountId::from(pair.public());
        Balances::mint_into(&who, DOLLARS).unwrap();

        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
        let extra = (
            frame_system::CheckNonZeroSender::new(),
            frame_system::CheckSpecVersion::new(),
            frame_system::CheckTxVersion::new(),
            frame_system::CheckGenesis::new(),
            frame_system::CheckEra::from(Era::Immortal),
            frame_system::CheckNonce::from(0),
            frame_system::CheckWeight::new(),
            pallet_transaction_payment::ChargeTransactionPayment::from(0),
        );
        let payload = SignedPayload::new(call.clone(), extra.clone()).unwrap();
        let signature = payload.using_encoded(|m| pair.sign(m));
        let extrinsic = UncheckedExtrinsic::new_signed(call, who.into(), signature.into(), extra);
        Executive::apply_extrinsic(extrinsic).unwrap().unwrap();
        assert!(CollectedFees::<Runtime>::get() > 0);
    });
}

#[test]
fn fee_handler_deposits_into_collected_fees() {
    TestExternalities::default().execute_with(|| {
        let who = AccountId::from([1; 32]);
        Balances::mint_into(&who, 10 * DOLLARS).unwrap();
        let fee = <<Runtime as Config>::OnChargeTransaction as OnChargeTransaction<Runtime>>::withdraw_fee(
            &who,
            &RuntimeCall::System(frame_system::Call::remark { remark: vec![] }),
            &Default::default(),
            2 * DOLLARS,
            0,
        )
        .unwrap();
        <<Runtime as Config>::OnChargeTransaction as OnChargeTransaction<Runtime>>::correct_and_deposit_fee(
            &who,
            &Default::default(),
            &Default::default(),
            2 * DOLLARS,
            0,
            fee,
        )
        .unwrap();
        assert_eq!(CollectedFees::<Runtime>::get(), 2 * DOLLARS);
    });
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
        set_per_diem_rate_as_root(Perbill::from_percent(1));

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
            set_per_diem_rate(
                RuntimeOrigin::signed(AccountId::from([1; 32])),
                Perbill::from_percent(1)
            ),
            DispatchError::BadOrigin
        );
    });
}
