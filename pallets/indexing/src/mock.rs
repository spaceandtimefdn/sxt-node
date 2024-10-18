use frame_support::derive_impl;
use frame_support::traits::OriginTrait;
use sp_core::H256;
use sp_runtime::BuildStorage;

use crate as pallet_indexing;

type Block = frame_system::mocking::MockBlock<Test>;

pub mod api_impl {
    use native::interface;
    use native_api::NativeApi;
    use sp_runtime::BoundedVec;
    use sxt_core::native::{OnChainTableBytes, RowData};

    use super::*;
    pub struct Api;

    impl NativeApi for Api {
        fn record_batch_to_onchain(
            row_data: RowData,
        ) -> Result<sxt_core::native::OnChainTableBytes, sxt_core::native::NativeError> {
            interface::record_batch_to_onchain(row_data)
        }
    }

    pub type Pallet<T> = pallet_indexing::pallet::Pallet<T, Api>;
    pub type Event<T> = pallet_indexing::pallet::Event<T, Api>;
    pub type Error<T> = pallet_indexing::pallet::Error<T, Api>;

    pub use crate::pallet::{
        __substrate_call_check,
        __substrate_event_check,
        tt_default_parts,
        tt_error_token,
    };
}

pub use api_impl::Api;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Indexing: api_impl,
        Permissions: pallet_permissions,
        Commitments: pallet_commitments,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type Hash = H256;
}

impl pallet_indexing::pallet::Config<Api> for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_permissions::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_commitments::Config for Test {}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_commitments::GenesisConfig::<Test>::default()
        .assimilate_storage(&mut storage)
        .unwrap();

    storage.into()
}
