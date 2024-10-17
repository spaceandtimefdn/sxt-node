//! TODO: add docs
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

#[allow(clippy::manual_inspect)]
#[frame_support::pallet]
pub mod pallet {
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
    }

    impl<T: Config> Pallet<T> {
        /// Returns `true` if the account `who` has permission `p`
        pub fn has_permissions(who: &T::AccountId, p: &PermissionLevel) -> bool {
            Permissions::<T>::get(who)
                .iter()
                .flatten()
                .any(|x| *x == *p)
        }

        /// Returns Ok() if the origin is root or if the origin has permissions `p`
        pub fn ensure_root_or_permissioned(
            origin: OriginFor<T>,
            permission: &PermissionLevel,
        ) -> Result<(), DispatchError> {
            ensure_root(origin.clone()).or_else(|_| {
                ensure_signed(origin.clone())
                    .map_err(|_| Error::<T>::UnsignedTransaction.into())
                    .and_then(|c| {
                        Self::has_permissions(&c, permission)
                            .then_some(())
                            .ok_or(Error::<T>::InsufficientPermissions.into())
                    })
            })
        }
    }
}
