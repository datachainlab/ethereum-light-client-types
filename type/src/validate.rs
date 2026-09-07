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

/// Validates the execution update block hash against the finalized execution
/// root of `consensus_update`.
///
/// For pre-Gloas forks the block hash is verified via a Merkle proof against
/// the execution payload root. For Gloas and later forks the execution root is
/// the block hash itself, so the update's `block_hash` must simply equal it.
///
/// Required for L2 chains like Optimism and Arbitrum; not needed for Ethereum mainnet.
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
    let trusted_execution_root = consensus_update.finalized_execution_root();
    if fork_spec.is_gloas() {
        if execution_update.block_hash != trusted_execution_root {
            return Err(Error::UnexpectedBlockHash {
                expected: trusted_execution_root,
                actual: execution_update.block_hash,
            });
        }
    } else {
        validate_block_hash(execution_update, fork_spec, trusted_execution_root)?;
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
    use crate::consensus::ConsensusUpdateInfo;
    use alloc::vec;
    use ethereum_consensus::beacon::Version;
    use ethereum_consensus::config;
    use ethereum_consensus::context::DefaultChainContext;
    use ethereum_consensus::fork::altair::ALTAIR_FORK_SPEC;
    use ethereum_consensus::fork::deneb::DENEB_FORK_SPEC;
    use ethereum_consensus::fork::{ForkParameter, ForkParameters};
    use ethereum_consensus::preset;
    use ethereum_consensus::types::U64;
    use ethereum_light_client_verifier::context::{Fraction, LightClientContext};

    fn test_context() -> DefaultChainContext {
        // genesis_slot = 0, seconds_per_slot = 6 (minimal preset),
        // genesis_time = min_genesis_time => timestamp(slot) = 1578009600 + slot * 6
        let cfg = config::Config {
            preset: preset::minimal::PRESET,
            fork_parameters: ForkParameters::new(
                ethereum_consensus::beacon::Version([0, 0, 0, 1]),
                vec![ForkParameter::new(
                    ethereum_consensus::beacon::Version([1, 0, 0, 1]),
                    U64(0),
                    ALTAIR_FORK_SPEC,
                )],
            )
            .unwrap(),
            min_genesis_time: U64(1578009600),
        };
        DefaultChainContext::new_with_config(U64(1729846322), cfg)
    }

    fn h256_from_byte(byte: u8) -> H256 {
        H256::from_slice(&[byte; 32])
    }

    fn create_test_execution_update() -> ExecutionUpdateInfo {
        ExecutionUpdateInfo {
            state_root: H256::default(),
            state_root_branch: vec![],
            block_number: U64::from(0),
            block_number_branch: vec![],
            block_hash: H256::default(),
            block_hash_branch: vec![],
            rlp: vec![],
        }
    }

    fn make_context(spec: ethereum_consensus::fork::ForkSpec) -> LightClientContext {
        LightClientContext::new(
            ForkParameters::new(
                Version([0, 0, 0, 1]),
                vec![ForkParameter::new(
                    Version([1, 0, 0, 1]),
                    U64::from(0),
                    spec,
                )],
            )
            .unwrap(),
            U64::from(6),
            U64::from(8),
            U64::from(8),
            U64::from(0),
            Default::default(),
            1,
            Fraction::new(2, 3).unwrap(),
            U64::from(0),
        )
    }

    #[test]
    fn test_validate_execution_update_gloas_block_hash_equality() {
        let mut gloas_spec = DENEB_FORK_SPEC;
        gloas_spec.execution_block_hash_gindex = 2856;
        assert!(gloas_spec.is_gloas());
        let ctx = make_context(gloas_spec);
        let consensus_update = ConsensusUpdateInfo::<32> {
            finalized_execution_root: h256_from_byte(5),
            ..Default::default()
        };

        // for Gloas, block_hash must equal the verified execution root (no merkle proof needed)
        let mut execution_update = create_test_execution_update();
        execution_update.block_hash = h256_from_byte(5);
        validate_execution_update::<32, _, _>(&ctx, &consensus_update, &execution_update).unwrap();

        // mismatched block_hash must be rejected
        execution_update.block_hash = h256_from_byte(6);
        let result =
            validate_execution_update::<32, _, _>(&ctx, &consensus_update, &execution_update);
        assert!(matches!(result, Err(Error::UnexpectedBlockHash { .. })));
    }

    #[test]
    fn test_validate_execution_update_requires_proof_pre_gloas() {
        let ctx = make_context(DENEB_FORK_SPEC);
        let consensus_update = ConsensusUpdateInfo::<32>::default();
        let execution_update = create_test_execution_update();
        let result =
            validate_execution_update::<32, _, _>(&ctx, &consensus_update, &execution_update);
        assert!(matches!(
            result,
            Err(Error::InvalidBlockHashMerkleBranch { .. })
        ));
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
}
