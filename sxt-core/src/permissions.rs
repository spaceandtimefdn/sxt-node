use alloc::boxed::Box;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::storage::bounded_vec::BoundedVec;
use frame_support::traits::ConstU32;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_core::RuntimeDebug;

use super::ByteString;
use crate::tables::TableIdentifier;

/// A user created permission level represented by a byte string;
pub type UserCreatedPermissionLevel = ByteString;

/// AccountId's can have associated permissions that allow them to make changes within the indexing pallet.
/// These permissions can currently only be sent by the sudo key.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, Serialize, Deserialize)]
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

    /// Premisions related to the governance pallet
    IndexingPallet(IndexingPalletPermission),

    /// Permissions related to attestations
    AttestationPallet(AttestationPalletPermission),

    /// Permissions related to the smart contracts pallet.
    SmartContractsPallet(SmartContractsPalletPermission),

    /// The ability to proxy a permission level on behalf of other users
    EditSpecificPermission(Box<PermissionLevel>),
}

/// Permissions for pallet_tables
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
pub enum TablesPalletPermission {
    /// Permission related to editing table schemas
    EditSchema,
    /// TODO: add docs
    EditRewards,
    /// Permission related to updating the UUIDs for tables or namespaces
    EditUuid,
}

/// Permissions for pallet_governance TODO
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
pub enum GovernancePalletPermission {
    /// TODO: add docs
    AddIndexer,
    /// TODO: add docs
    RemoveIndexer,
}

/// Permissions used by the indexing pallet
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
pub enum IndexingPalletPermission {
    /// Represents the permission needed to submit data as an indexer for public quorum.
    SubmitDataForPublicQuorum,
    /// Represents the permission needed to submit data as an indexer for privileged quorum.
    ///
    /// This permission is table-specific.
    SubmitDataForPrivilegedQuorum(TableIdentifier),
}

/// Permissions used by the indexing pallet
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
pub enum AttestationPalletPermission {
    /// The permission to have attestations accepted from your signed account id
    AttestBlock,

    /// The permission to mark an attested block as forwarded to the EVM layer
    ForwardAttestedBlock,
}

/// PErmissions for the pallet-smartcontracts
#[derive(
    Clone,
    Encode,
    Decode,
    Eq,
    PartialEq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
pub enum SmartContractsPalletPermission {
    /// Permission to update the ABI of a smart contract.
    UpdateABI,
}

/// A collection of user permissions
pub type PermissionList = BoundedVec<PermissionLevel, ConstU32<32>>;
