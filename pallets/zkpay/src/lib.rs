//! # ZKpay Pallet
//! This pallet contains types, utilities, and logic related to processing ZKpay
// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

/// Templates for parsing indexed ZKpay events
pub mod templates;

// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
    // Import various useful types required by all FRAME pallets.
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use hex::encode;
    use sp_runtime::traits::{StaticLookup, UniqueSaturatedInto};
    use sp_runtime::SaturatedConversion;
    use sxt_core::parse::SystemRequestType::ZkPay;
    use sxt_core::parse::{SystemFieldValue, SystemRequest, SystemRequestType, ZKPayRequest};
    use sxt_core::ByteString;

    use super::*;
    use crate::Error::*;
    use crate::Event::*;

    // The `Pallet` struct serves as a placeholder to implement traits, methods and dispatchables
    // (`Call`s) in this pallet.
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The pallet's configuration trait.
    ///
    /// All our types and constants a pallet depends on must be declared here.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    /// Events that functions in this pallet can emit.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// We processed a ZKpay event
        EventReceived {
            /// The event that we received
            event: ZKPayRequest,
        },
        /// An error occurred processing a ZKpay Request
        ZkPayProcessingError {
            /// The error that occurred
            error: DispatchError,
        },
        /// Compute credits were purchased for an account
        ComputeCreditsPurchased {
            /// The amount purchased
            amount: u128,
            /// The account ID that was credited
            account_id: T::AccountId,
            /// The sender of the payment
            sender: T::AccountId,
        },
        /// The smart contract address for the compute credit contract has changed
        ComputeCreditAddressUpdated {
            /// The old address
            old_address: ByteString,
            /// The new address
            new_address: ByteString,
        },
    }

    /// Errors that can be returned by this pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// There was a missing field in the record batch we tried to parse
        MissingExpectedField,
        /// The contract address supplied was not a 20 byte address
        ContractAddressError,
        /// An account id provided was invalid
        InvalidAccountId,
    }

    #[pallet::storage]
    #[pallet::getter(fn compute_credit_address)]
    pub type ComputeCreditAddress<T> = StorageValue<_, ByteString, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// A helper extrinsic to set the contract address used for Compute Credit transactions
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::zero())]
        pub fn update_compute_credit_address(
            origin: OriginFor<T>,
            address: ByteString,
        ) -> DispatchResult {
            // Check that the extrinsic was signed by root.
            ensure_root(origin)?;

            // Address must be a valid 20 byte ethereum address
            if address.to_vec().len() != 20 {
                return Err(Error::<T>::ContractAddressError.into());
            }

            // Retrieve the old address for the event we'll emit
            let old_address = ComputeCreditAddress::<T>::get();

            // Update storage.
            ComputeCreditAddress::<T>::put(address.clone());

            // Emit an event.
            Self::deposit_event(Event::<T>::ComputeCreditAddressUpdated {
                old_address,
                new_address: address,
            });

            Ok(())
        }
    }

    /// Internal functionality
    impl<T: Config> Pallet<T> {
        /// Takes in a ZKpay request and routes it to the appropriate processing function
        pub fn process_zkpay_request(request: SystemRequest) -> DispatchResult {
            // Start by emitting an event that we've parsed and received the request
            Pallet::<T>::notify_received(request.request_type);

            match request.request_type {
                ZkPay(ZKPayRequest::SendPayment) => Pallet::<T>::process_send_payment(request),
                ZkPay(ZKPayRequest::AssetAdded) => Pallet::<T>::process_asset_added(request),
                ZkPay(ZKPayRequest::AssetRemoved) => Pallet::<T>::process_asset_removed(request),
                ZkPay(ZKPayRequest::CallbackFailed) => {
                    Pallet::<T>::process_callback_failed(request)
                }
                ZkPay(ZKPayRequest::CallbackSucceeded) => {
                    Pallet::<T>::process_callback_succeeded(request)
                }
                ZkPay(ZKPayRequest::Initialized) => Pallet::<T>::process_initialized(request),
                ZkPay(ZKPayRequest::NewQueryPayment) => {
                    Pallet::<T>::process_new_query_payment(request)
                }
                ZkPay(ZKPayRequest::PaymentRefunded) => {
                    Pallet::<T>::process_payment_refunded(request)
                }
                ZkPay(ZKPayRequest::PaymentSettled) => {
                    Pallet::<T>::process_payment_settled(request)
                }
                ZkPay(ZKPayRequest::QueryCancelled) => {
                    Pallet::<T>::process_query_cancelled(request)
                }
                ZkPay(ZKPayRequest::QueryFulfilled) => {
                    Pallet::<T>::process_query_fulfilled(request)
                }
                ZkPay(ZKPayRequest::QueryReceived) => Pallet::<T>::process_query_received(request),
                ZkPay(ZKPayRequest::TreasurySet) => Pallet::<T>::process_treasury_set(request),
                _ => Ok(()),
            }
        }

        /// Helper function to deposit an EventReceived event for the supplied request type
        fn notify_received(request_type: SystemRequestType) {
            match request_type {
                ZkPay(subtype) => Pallet::<T>::deposit_event(EventReceived { event: subtype }),
                _ => {
                    // NOP
                }
            }
        }

        /// Helper function to deposit an error event if the supplied result was an error
        fn emit_for_error(result: DispatchResult) {
            if let Err(error) = result {
                // Emit an event for any errors
                Pallet::<T>::deposit_event(Event::<T>::ZkPayProcessingError { error });
            }
        }

        /// Process a request to add a new ZKpay asset
        fn process_asset_added(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("ASSET"),
                        row.get("ALLOWEDPAYMENTTYPES"),
                        row.get("PRICEFEED"),
                        row.get("TOKENDECIMALS"),
                        row.get("STALEPRICETHRESHOLDINSECONDS"),
                    ) {
                        (
                            Some(SystemFieldValue::Bytes(asset)), // The asset address
                            Some(SystemFieldValue::Bytes(allowed_payment_types)), // The allowed payment types presented as a bytes1 bitmask
                            Some(SystemFieldValue::Bytes(price_feed)), // The price feed address
                            Some(SystemFieldValue::SmallInt(token_decimals)), //The token decimals
                            Some(SystemFieldValue::Decimal(stale_price_threshold_in_seconds)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// Process a request to remove a ZKpay asset
        fn process_asset_removed(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match row.get("ASSET") {
                        Some(SystemFieldValue::Bytes(asset)) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// Process a callback_failed notification
        fn process_callback_failed(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("QUERYHASH"),
                        row.get("CALLBACKCLIENTCONTRACTADDRESS"),
                    ) {
                        (
                            Some(SystemFieldValue::Bytes(query_hash)),
                            Some(SystemFieldValue::Bytes(callback_client_contract_address)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// Process a call success notification
        fn process_callback_succeeded(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("QUERYHASH"),
                        row.get("CALLBACKCLIENTCONTRACTADDRESS"),
                    ) {
                        (
                            Some(SystemFieldValue::Bytes(query_hash)),
                            Some(SystemFieldValue::Bytes(callback_client_contract_address)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// Process smart contract initialization. This is included for completeness, but should only ever
        /// occur once when the contract is first initialized on the source chain
        fn process_initialized(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match row.get("VERSION") {
                        Some(SystemFieldValue::Decimal(asset)) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// A payment has been made for a query
        fn process_new_query_payment(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("QUERYHASH"),
                        row.get("ASSET"),
                        row.get("AMOUNT"),
                        row.get("SOURCE_"),
                        row.get("AMOUNTINUSD"),
                    ) {
                        (
                            Some(SystemFieldValue::Bytes(query_hash)),
                            Some(SystemFieldValue::Bytes(asset)),
                            Some(SystemFieldValue::Decimal(amount)),
                            Some(SystemFieldValue::Bytes(source_)),
                            Some(SystemFieldValue::Decimal(amount_in_usd)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// A payment has been refunded (likely due to a cancelled query)
        fn process_payment_refunded(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("QUERYHASH"),
                        row.get("ASSET"),
                        row.get("SOURCE_"),
                        row.get("AMOUNT"),
                    ) {
                        (
                            Some(SystemFieldValue::Bytes(query_hash)),
                            Some(SystemFieldValue::Bytes(asset)),
                            Some(SystemFieldValue::Bytes(source_)),
                            Some(SystemFieldValue::Decimal(amount)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// A payment for a query has been settled on the source chain
        fn process_payment_settled(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("QUERYHASH"),
                        row.get("USEDAMOUNT"),
                        row.get("REMAININGAMOUNT"),
                    ) {
                        (
                            Some(SystemFieldValue::Bytes(query_hash)),
                            Some(SystemFieldValue::Decimal(used_amount)),
                            Some(SystemFieldValue::Decimal(remaining_amount)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// A query was cancelled
        fn process_query_cancelled(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (row.get("QUERYHASH"), row.get("CALLER")) {
                        (
                            Some(SystemFieldValue::Bytes(query_hash)),
                            Some(SystemFieldValue::Bytes(caller)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// A query was successfully fulfilled
        fn process_query_fulfilled(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match row.get("QUERYHASH") {
                        Some(SystemFieldValue::Bytes(query_hash)) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// A query was requested and is now awaiting fulfillment
        fn process_query_received(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("QUERYNONCE"),
                        row.get("SENDER"),
                        row.get("QUERY"),
                        row.get("QUERYPARAMETERS"),
                        row.get("TIMEOUT"),
                        row.get("CALLBACKCLIENTCONTRACTADDRESS"),
                        row.get("CALLBACKGASLIMIT"),
                        row.get("CALLBACKDATA"),
                        row.get("CUSTOMLOGICCONTRACTADDRESS"),
                        row.get("QUERYHASH"),
                    ) {
                        (
                            Some(SystemFieldValue::Decimal(query_nonce)),
                            Some(SystemFieldValue::Bytes(sender)),
                            Some(SystemFieldValue::Bytes(query)),
                            Some(SystemFieldValue::Bytes(query_parameters)),
                            Some(SystemFieldValue::Decimal(timeout)),
                            Some(SystemFieldValue::Bytes(callback_client_contract_address)),
                            Some(SystemFieldValue::Decimal(callback_gas_limit)),
                            Some(SystemFieldValue::Bytes(callback_data)),
                            Some(SystemFieldValue::Bytes(custom_logic_contract_address)),
                            Some(SystemFieldValue::Bytes(query_hash)),
                        ) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// A payment has been sent via ZKpay, such as when purchasing compute credits
        pub(crate) fn process_send_payment(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match (
                        row.get("ASSET"),
                        row.get("AMOUNT"),
                        row.get("ONBEHALFOF"),
                        row.get("TARGET"),
                        row.get("MEMO"),
                        row.get("AMOUNTINUSD"),
                        row.get("SENDER"),
                    ) {
                        (
                            Some(SystemFieldValue::Bytes(asset)),
                            Some(SystemFieldValue::Decimal(amount)),
                            Some(SystemFieldValue::Bytes(on_behalf_of)),
                            Some(SystemFieldValue::Bytes(target)),
                            Some(SystemFieldValue::Bytes(memo)),
                            Some(SystemFieldValue::Decimal(amount_in_usd)),
                            Some(SystemFieldValue::Bytes(sender)),
                        ) => {
                            // For now, we only support SXT and no other assets. Because of this
                            // we will ignore the asset field.

                            // Check if the target address is the Compute Credit Address
                            let compute_credit_address = ComputeCreditAddress::<T>::get();
                            if target == compute_credit_address.as_slice() {
                                // Get the address we need to fund
                                let funded_addr = sxt_core::utils::account_id_from_str::<T>(
                                    encode(on_behalf_of).as_str(),
                                )
                                .map_err(|_| Error::<T>::InvalidAccountId)?;

                                // Get their current balance
                                let balance: u128 =
                                    pallet_balances::Pallet::<T>::free_balance(&funded_addr)
                                        .unique_saturated_into();

                                let amount: u128 = amount.as_u128().saturated_into();

                                // Add the new amount
                                let new_total_balance = balance.saturating_add(amount);

                                // Force set the balance
                                let funded_lookup = <T as frame_system::Config>::Lookup::unlookup(
                                    funded_addr.clone(),
                                );
                                pallet_balances::Pallet::<T>::force_set_balance(
                                    RawOrigin::Root.into(),
                                    funded_lookup,
                                    new_total_balance.saturated_into(),
                                )?;

                                // Emit an event that the sender sent payment, including the "on_behalf_of"
                                let sender_addr = sxt_core::utils::account_id_from_str::<T>(
                                    encode(sender).as_str(),
                                )?;
                                Self::deposit_event(Event::<T>::ComputeCreditsPurchased {
                                    amount,
                                    account_id: funded_addr.clone(),
                                    sender: sender_addr,
                                })
                            }
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }

        /// The treasury address has been set or updated.
        fn process_treasury_set(request: SystemRequest) -> DispatchResult {
            request
                .rows()
                .map(|row| -> DispatchResult {
                    match row.get("TREASURY") {
                        Some(SystemFieldValue::Bytes(treasury)) => {
                            // Not yet supported
                            Ok(())
                        }
                        _ => Err(Error::<T>::MissingExpectedField.into()),
                    }
                })
                .for_each(Pallet::<T>::emit_for_error);

            Ok(())
        }
    }
}
