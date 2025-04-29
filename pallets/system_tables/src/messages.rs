#![doc = include_str!("../MESSAGES.md")]
extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::str::from_utf8;

use codec::{Decode, Encode};
use frame_support::dispatch::RawOrigin;
use frame_support::pallet_prelude::TypeInfo;
use hex::FromHex;
use pallet_staking::ValidatorPrefs;
use sp_runtime::{DispatchError, DispatchResult, Perbill};

use crate::{Config, Error};

/// Right now we only support one message to register session keys
pub fn handle_message<T: Config>(sender: T::AccountId, message_bytes: Vec<u8>) -> DispatchResult {
    let keys =
        T::Keys::decode(&mut &message_bytes[..]).map_err(|_| Error::<T>::InvalidSessionKeys)?;

    // Set session keys
    let prefs = ValidatorPrefs {
        commission: Perbill::from_percent(0), // 0 Commission here, because we calculate it at the end of the era
        blocked: false,
    };

    match pallet_staking::Pallet::<T>::set_controller(RawOrigin::Signed(sender.clone()).into()) {
        Ok(_) => Ok(()),
        Err(e) => {
            if e == DispatchError::from(pallet_staking::Error::<T>::AlreadyPaired) {
                // Non Action
                Ok(())
            } else {
                Err(e)
            }
        }
    }?;

    pallet_staking::Pallet::<T>::validate(RawOrigin::Signed(sender.clone()).into(), prefs)?;
    pallet_session::Pallet::<T>::set_keys(RawOrigin::Signed(sender.clone()).into(), keys, vec![])
}
