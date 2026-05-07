//! Validation utilities for Ethereum light client updates.
//!
//! This module provides validation functions for consensus and execution updates,
//! including pre-Gloas block hash verification.

use crate::consensus::ExecutionUpdateInfo;
use crate::errors::Error;
use ethereum_consensus::compute::hash_tree_root;
use ethereum_consensus::merkle::is_valid_normalized_merkle_branch;
use ethereum_consensus::types::H256;
use ethereum_light_client_verifier::context::ChainConsensusVerificationContext;
use ethereum_light_client_verifier::updates::ConsensusUpdate;

/// Difference between block_number gindex and block_hash gindex in ExecutionPayload.
const BLOCK_NUMBER_TO_BLOCK_HASH_DIFF: u32 = 6;

/// Validates the execution update block hash for pre-Gloas forks.
///
/// For hard forks before Gloas, the execution payload root is a Merkle root
/// and the block hash must be verified via a Merkle proof. For Gloas and later,
/// the execution root is the block hash directly, so this validation is skipped.
///
/// This validation is required for L2 chains like Optimism and Arbitrum.
/// It is not needed for Ethereum mainnet.
///
/// # Arguments
///
/// * `ctx` - Chain context for computing fork specification
/// * `consensus_update` - The consensus update containing the finalized beacon header
/// * `execution_update` - The execution update containing the block hash proof
///
/// # Errors
///
/// Returns an error if the block hash Merkle proof verification fails.
pub fn validate_execution_update<const SYNC_COMMITTEE_SIZE: usize, CC, CU>(
    ctx: &CC,
    consensus_update: &CU,
    execution_update: &ExecutionUpdateInfo,
) -> Result<(), Error>
where
    CC: ChainConsensusVerificationContext,
    CU: ConsensusUpdate<SYNC_COMMITTEE_SIZE>,
{
    let fork_spec = ctx.compute_fork_spec(consensus_update.finalized_beacon_header().slot);
    if fork_spec.execution_block_hash_gindex == 0 {
        let trusted_execution_root = consensus_update.finalized_execution_root();
        validate_block_hash(execution_update, fork_spec, trusted_execution_root)?;
    }
    Ok(())
}

/// Validates the execution update block hash for pre-Gloas forks using a raw execution root.
///
/// This is a lower-level variant that takes the execution root directly rather than
/// extracting it from a consensus update.
///
/// # Arguments
///
/// * `ctx` - Chain context for computing fork specification
/// * `slot` - The beacon slot to determine the fork specification
/// * `execution_root` - The trusted execution payload root
/// * `execution_update` - The execution update containing the block hash proof
///
/// # Errors
///
/// Returns an error if the block hash Merkle proof verification fails.
pub fn validate_execution_update_with_root<CC>(
    ctx: &CC,
    slot: u64,
    execution_root: H256,
    execution_update: &ExecutionUpdateInfo,
) -> Result<(), Error>
where
    CC: ChainConsensusVerificationContext,
{
    let fork_spec = ctx.compute_fork_spec(slot.into());
    if fork_spec.execution_block_hash_gindex == 0 {
        validate_block_hash(execution_update, fork_spec, execution_root)?;
    }
    Ok(())
}

/// Validates the block hash merkle proof against the execution root.
///
/// This is an internal helper function used by the public validation functions.
fn validate_block_hash(
    execution_update: &ExecutionUpdateInfo,
    fork_spec: ethereum_consensus::fork::ForkSpec,
    trusted_execution_root: H256,
) -> Result<(), Error> {
    let execution_payload_block_hash_gindex =
        fork_spec.execution_payload_block_number_gindex + BLOCK_NUMBER_TO_BLOCK_HASH_DIFF;

    is_valid_normalized_merkle_branch(
        hash_tree_root(execution_update.block_hash)
            .map_err(Error::EthereumConsensus)?
            .0
            .into(),
        &execution_update.block_hash_branch,
        execution_payload_block_hash_gindex,
        trusted_execution_root,
    )
    .map_err(|e| Error::InvalidBlockHashMerkleBranch { error: e })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use ethereum_consensus::fork::deneb::DENEB_FORK_SPEC;
    use ethereum_consensus::types::U64;

    fn h256_from_byte(byte: u8) -> H256 {
        H256::from_slice(&[byte; 32])
    }

    fn create_test_execution_update() -> ExecutionUpdateInfo {
        ExecutionUpdateInfo {
            state_root: H256::default(),
            state_root_branch: vec![],
            block_number: U64::from(0),
            block_number_branch: vec![],
            rlp: vec![],
            block_hash: H256::default(),
            block_hash_branch: vec![],
        }
    }

    #[test]
    fn test_validate_block_hash_empty_branch_fails() {
        let execution_update = create_test_execution_update();
        let fork_spec = DENEB_FORK_SPEC;
        let trusted_execution_root = h256_from_byte(1);

        let result = validate_block_hash(&execution_update, fork_spec, trusted_execution_root);

        // Should fail because block_hash_branch is empty
        assert!(result.is_err());
        match result {
            Err(Error::InvalidBlockHashMerkleBranch { .. }) => {}
            _ => panic!("Expected InvalidBlockHashMerkleBranch error"),
        }
    }

    #[test]
    fn test_block_number_to_block_hash_diff_constant() {
        // Block hash is 6 positions after block number in ExecutionPayload
        assert_eq!(BLOCK_NUMBER_TO_BLOCK_HASH_DIFF, 6);
    }
}
