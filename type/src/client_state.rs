//! Client state trait for Ethereum light client.

use crate::height::Height;
use ethereum_consensus::types::H256;

/// Trait representing the state of an Ethereum light client.
///
/// This trait defines the minimal interface required for client state
/// operations in IBC verification workflows.
pub trait ClientState {
    /// Returns whether this client has been frozen due to misbehaviour.
    fn is_frozen(&self) -> bool;

    /// Returns the latest verified height of this client.
    fn latest_height(&self) -> Height;

    /// Returns the storage slot for IBC commitments in the IBC contract.
    ///
    /// This slot is used to calculate the storage location for commitment proofs.
    fn ibc_commitments_slot(&self) -> H256;

    /// Returns a canonicalized version of this client state.
    ///
    /// Canonicalization ensures consistent serialization for commitment calculations.
    fn canonicalize(self) -> Self;
}
