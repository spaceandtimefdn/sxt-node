//! TODO: add docs
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::*;

#[allow(clippy::manual_inspect)]
#[polkadot_sdk::frame_support::pallet]
pub mod pallet {
    use alloc::boxed::Box;

    use polkadot_sdk::frame_support::dispatch::DispatchResult;
    use polkadot_sdk::frame_support::pallet_prelude::*;
    use polkadot_sdk::frame_system;
    use polkadot_sdk::frame_system::pallet_prelude::*;
    use sxt_core::permissions::{PermissionLevel, PermissionList};

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: polkadot_sdk::frame_system::Config {
        /// The events that can be emitted by this pallet.
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as polkadot_sdk::frame_system::Config>::RuntimeEvent>;
        /// The weight info for calls in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// Storage map of `AccountId`s to their permissions.
    #[pallet::storage]
    #[pallet::unbounded]
    #[pallet::getter(fn permissions)]
    pub type Permissions<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, PermissionList>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The permissions for an account were updated.
        PermissionsSet(T::AccountId, PermissionList),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The signer of this transaction has insufficient permissions
        InsufficientPermissions,

        /// set_permissions was called with an empty list of permissions, please call set_permissions
        EmptyPermissionsListError,

        /// This transaction was unsigned
        UnsignedTransaction,

        /// Bad Origin
        PermissionsBadOrigin,

        /// The proxy user already has this permission
        PermissionAlreadyExists,

        /// The proxy user's permission list is full
        PermissionListFull,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the permissions for an account.
        ///
        /// # Events
        /// Emits `Event::PermissionsSet`.
        ///
        /// # Permissions
        /// Requires `PermissionLevel::UpdatePermissions`.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_permissions())]
        pub fn set_permissions(
            origin: OriginFor<T>,
            who: T::AccountId,
            permissions: PermissionList,
        ) -> DispatchResult {
            Self::ensure_root_or_permissioned(origin.clone(), &PermissionLevel::UpdatePermissions)?;
            ensure!(
                !permissions.is_empty(),
                Error::<T>::EmptyPermissionsListError
            );

            Permissions::<T>::insert(who.clone(), permissions.clone());
            Self::deposit_event(Event::PermissionsSet(who, permissions));
            Ok(())
        }

        /// Clear the permissions for an account.
        ///
        /// # Events
        /// Emits `Event::PermissionsSet`.
        ///
        /// # Permissions
        /// Requires `PermissionLevel::UpdatePermissions`.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::clear_permissions())]
        pub fn clear_permissions(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
            Self::ensure_root_or_permissioned(origin, &PermissionLevel::UpdatePermissions)?;

            let permissions = PermissionList::default();

            Permissions::<T>::remove(who.clone());
            Self::deposit_event(Event::PermissionsSet(who, permissions));

            Ok(())
        }

        /// Adds a permission to the given account, if it doesn't already exist.
        ///
        /// # Events
        /// Emits `Event::PermissionsSet`.
        ///
        /// # Permissions
        /// Requires `PermissionLevel::EditSpecificPermission(permission)`
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::add_proxy_permission())]
        pub fn add_proxy_permission(
            origin: OriginFor<T>,
            proxy: T::AccountId,
            permission: PermissionLevel,
        ) -> DispatchResult {
            Self::ensure_root_or_permissioned(
                origin,
                &PermissionLevel::EditSpecificPermission(Box::new(permission.clone())),
            )?;

            // Retrieve the current permissions for the proxy
            Permissions::<T>::try_mutate(&proxy, |permissions_opt| -> DispatchResult {
                let permissions = permissions_opt.get_or_insert_with(PermissionList::default);

                // Add the new permission, ensuring no duplicates and bounded size
                if permissions.contains(&permission) {
                    Err(Error::<T>::PermissionAlreadyExists.into())
                } else {
                    permissions
                        .try_push(permission)
                        .map_err(|_| Error::<T>::PermissionListFull)?;

                    Self::deposit_event(Event::PermissionsSet(proxy.clone(), permissions.clone()));

                    Ok(())
                }
            })?;

            // Emit an event for successful addition of permission
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Returns `true` if the account `who` has permission `p`
        pub fn has_permissions(who: &T::AccountId, p: &PermissionLevel) -> bool {
            Permissions::<T>::get(who)
                .iter()
                .flatten()
                .any(|x| *x == *p)
        }

        /// Returns `true` if the account `who` has **at least one** of the given permissions.
        pub fn has_any_permissions(who: &T::AccountId, permissions: &[PermissionLevel]) -> bool {
            permissions.iter().any(|p| Self::has_permissions(who, p))
        }

        /// Checks whether the origin is either `Root` or a signed account with the required permission level.
        ///
        /// Returns:
        /// - `Ok(None)` if the origin is `Root` (system-level access),
        /// - `Ok(Some(account_id))` if the origin is a signed account with the required permission,
        /// - `Err(UnsignedTransaction)` if the origin is neither signed nor root,
        /// - `Err(InsufficientPermissions)` if the signed account lacks the required permission.
        ///
        /// # Parameters
        /// - `origin`: The origin of the call (can be signed, root, or other).
        /// - `permission`: The [`PermissionLevel`] required to perform the action.
        /// ```
        pub fn ensure_root_or_permissioned(
            origin: OriginFor<T>,
            permission: &PermissionLevel,
        ) -> Result<Option<T::AccountId>, DispatchError> {
            match origin.into() {
                Ok(frame_system::RawOrigin::Root) => Ok(None),
                Ok(frame_system::RawOrigin::Signed(who)) => {
                    if Self::has_permissions(&who, permission) {
                        Ok(Some(who))
                    } else {
                        Err(Error::<T>::InsufficientPermissions.into())
                    }
                }
                _ => Err(Error::<T>::UnsignedTransaction.into()),
            }
        }
    }
}
