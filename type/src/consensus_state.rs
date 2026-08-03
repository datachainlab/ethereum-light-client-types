//! Consensus state trait for Ethereum light client.

use ethereum_consensus::types::H256;

/// Trait representing the consensus state of an Ethereum light client.
///
/// The consensus state contains the minimum information needed to verify
/// proofs against a specific block height.
pub trait ConsensusState {
    /// Returns the storage root of the execution layer state.
    ///
    /// This root is used to verify Merkle-Patricia Trie proofs for
    /// IBC commitment verification.
    fn storage_root(&self) -> H256;
}
