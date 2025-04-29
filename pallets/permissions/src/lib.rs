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
#[frame_support::pallet]
pub mod pallet {
    use alloc::boxed::Box;

    use frame_support::dispatch::DispatchResult;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sxt_core::permissions::{PermissionLevel, PermissionList};

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// TODO: add docs
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// TODO: add docs
        type WeightInfo: WeightInfo;
    }

    /// A map of which actions AccountIds have permission for
    #[pallet::storage]
    #[pallet::unbounded]
    #[pallet::getter(fn permissions)]
    pub type Permissions<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, PermissionList>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The permissions for this account id were updated
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
        /// Set the permissions for an account id
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_permissions())]
        /// TODO: add docs
        pub fn set_permissions(
            origin: OriginFor<T>,
            who: T::AccountId,
            permissions: PermissionList,
        ) -> DispatchResult {
            Self::ensure_root_or_permissioned(origin.clone(), &PermissionLevel::UpdatePermissions)?;
            ensure!(permissions.len() > 0, Error::<T>::EmptyPermissionsListError);

            Permissions::<T>::insert(who.clone(), permissions.clone());
            Self::deposit_event(Event::PermissionsSet(who, permissions));
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::clear_permissions())]
        /// TODO: add docs
        pub fn clear_permissions(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
            Self::ensure_root_or_permissioned(origin, &PermissionLevel::UpdatePermissions)?;

            let permissions = PermissionList::default();

            Permissions::<T>::remove(who.clone());
            Self::deposit_event(Event::PermissionsSet(who, permissions));

            Ok(())
        }

        /// Adds a specified permission level to the permissions list of a proxy account.
        ///
        /// This extrinsic allows a user with the `EditSpecificPermission(Permission)` level
        /// to assign the specified `PermissionLevel` to a given proxy account (`proxy`).
        ///
        /// The permissions list is managed as a bounded vector to ensure storage limits are respected.
        /// Duplicate permissions are not allowed, and an error is returned if the permission already exists
        /// or if the permissions list is full.
        ///
        /// Emits:
        /// - `Event::PermissionsSet` on successful addition of the permission.
        ///
        /// Errors:
        /// - `Error::PermissionAlreadyExists` if the permission is already assigned to the proxy.
        /// - `Error::PermissionListFull` if the proxy's permissions list has reached its capacity.
        ///
        /// Requirements:
        /// - The caller must be authorized by being either the root origin or having the
        ///   `EditSpecificPermission` level for the specified permission.        #[pallet::call_index(2)]
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
