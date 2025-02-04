use frame_support::assert_ok;

use crate::mock::*;

#[test]
fn converting_from_eth_address_string_to_substrate_account_id_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let test_address = "0x67400a78A74f8211A0E15091DB79782e08Afa677";
        let r: Result<<Test as frame_system::Config>::AccountId, _> =
            crate::<Test>::eth_address_to_substrate_account_id(test_address);
        assert_ok!(r);
    })
}
