use super::ByteString;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::storage::bounded_vec::BoundedVec;
use frame_support::traits::ConstU32;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

/// A user created permission level represented by a byte string;
pub type UserCreatedPermissionLevel = ByteString;

/// AccountId's can have associated permissions that allow them to make changes within the indexing pallet.
/// These permissions can currently only be sent by the sudo key.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum PermissionLevel {
    /// This account id has permission to edit permissions for other users
    UpdatePermissions,

    /// A permission level created through a signed transaction, represented by a byte string
    UserCreated(UserCreatedPermissionLevel),

    // pallet level permissions
    /// Permissions related to the tables pallet
    TablesPallet(TablesPalletPermission),

    /// Permissions related to the governance pallet
    GovernancePallet(GovernancePalletPermission),
}


/// Permissions for pallet_tables
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum TablesPalletPermission {
    /// TODO: add docs
    EditSchema,
    /// TODO: add docs
    EditRewards,
}

/// Permissions for pallet_governance TODO
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum GovernancePalletPermission {
    /// TODO: add docs
    AddIndexer,
    /// TODO: add docs
    RemoveIndexer,
}

/// A collection of user permissions
pub type PermissionList = BoundedVec<PermissionLevel, ConstU32<32>>;
