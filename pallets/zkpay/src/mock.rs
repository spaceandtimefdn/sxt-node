use frame_support::pallet_prelude::ConstU32;
use frame_support::traits::{ConstU128, KeyOwnerProofSystem, VariantCountOf};
use frame_support::{derive_impl, parameter_types};
use sp_core::{ConstU64, H256};
use sp_runtime::traits::{ConvertInto, IdentityLookup, MaybeConvert, OpaqueKeys, TryConvertInto};
use sp_runtime::{generic, BuildStorage, KeyTypeId};

use crate as pallet_zkpay;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        ZkPay: pallet_zkpay,
        Balances: pallet_balances,
    }
);

type AccountId = sp_core::crypto::AccountId32;
type Nonce = u32;
type Balance = u128;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Nonce = Nonce;
    type AccountId = AccountId;
    type AccountData = pallet_balances::AccountData<Balance>;
    type RuntimeCall = RuntimeCall;

    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Hash = H256;
}

impl pallet_zkpay::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    storage.into()
}
