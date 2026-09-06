//! Validation utilities for Ethereum light client updates.
//!
//! This module provides validation functions for consensus and execution updates,
//! including pre-Gloas block hash verification and the fork-dependent execution
//! header timestamp checks.

use crate::consensus::ExecutionUpdateInfo;
use crate::errors::Error;
use crate::time::secs_to_nanos;
use ethereum_consensus::beacon::Slot;
use ethereum_consensus::compute::{compute_timestamp_at_slot, hash_tree_root};
use ethereum_consensus::context::ChainContext;
use ethereum_consensus::merkle::is_valid_normalized_merkle_branch;
use ethereum_consensus::types::H256;
use ethereum_light_client_verifier::context::ChainConsensusVerificationContext;
use ethereum_light_client_verifier::updates::ConsensusUpdate;

/// Difference between block_number gindex and block_hash gindex in ExecutionPayload.
const BLOCK_NUMBER_TO_BLOCK_HASH_DIFF: u32 = 6;

/// Index of `timestamp` in the RLP-encoded execution block header.
const RLP_TIMESTAMP_INDEX: usize = 11;

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

/// Validates the header timestamp against the execution block that `execution_update`
/// describes, selecting the rule that applies to the fork at `finalized_slot`.
///
/// Callers should prefer this over calling [`validate_header_timestamp`] or
/// [`validate_header_timestamp_from_rlp`] directly, so that every light client built on this
/// crate branches the same way.
///
/// - pre-Gloas: the finalized beacon block carries the execution payload of its own slot, so
///   the timestamp must equal `compute_timestamp_at_slot(finalized_slot)`
/// - Gloas: `execution_update` describes the block that the bid's `parent_block_hash` points
///   at, so the timestamp is taken from the authenticated RLP header instead
pub fn validate_execution_header_timestamp<C: ChainConsensusVerificationContext>(
    ctx: &C,
    finalized_slot: Slot,
    execution_update: &ExecutionUpdateInfo,
    header_timestamp_nanos: u128,
) -> Result<(), Error> {
    if ctx.compute_fork_spec(finalized_slot).is_gloas() {
        validate_header_timestamp_from_rlp(&execution_update.rlp, header_timestamp_nanos)
    } else {
        validate_header_timestamp(ctx, finalized_slot, header_timestamp_nanos)
    }
}

/// Validates that the header timestamp is non-zero and equals
/// `compute_timestamp_at_slot(finalized_slot)`.
///
/// This holds pre-Gloas only, where the finalized beacon block carries the execution payload
/// of its own slot. Prefer [`crate::validate::validate_execution_header_timestamp`], which
/// picks this or the Gloas rule based on the fork.
pub fn validate_header_timestamp<C: ChainContext>(
    ctx: &C,
    finalized_slot: Slot,
    header_timestamp_nanos: u128,
) -> Result<(), Error> {
    if header_timestamp_nanos == 0 {
        return Err(Error::ZeroTimestamp);
    }
    let expected = secs_to_nanos(compute_timestamp_at_slot(ctx, finalized_slot).0);
    if header_timestamp_nanos != expected {
        return Err(Error::UnexpectedTimestamp {
            expected,
            actual: header_timestamp_nanos,
        });
    }
    Ok(())
}

/// Validates that the header timestamp is non-zero and equals the timestamp inside the
/// RLP-encoded execution block header. This is the Gloas counterpart of
/// [`validate_header_timestamp`].
///
/// Post-Gloas the execution block referenced by a light client header is the
/// `signed_execution_payload_bid.message.parent_block_hash`, i.e. the parent of the block
/// produced at the finalized slot. Its timestamp is therefore not
/// `compute_timestamp_at_slot(finalized_slot)`, and it cannot be derived from the slot either
/// because slots may be skipped.
///
/// Binding it to the RLP header keeps the check strict rather than relaxing it: the consensus
/// verifier authenticates the RLP via `keccak256(rlp) == execution_block_hash`, so this pins
/// the timestamp the same way `state_root` and `block_number` are already pinned.
pub fn validate_header_timestamp_from_rlp(
    rlp_bytes: &[u8],
    header_timestamp_nanos: u128,
) -> Result<(), Error> {
    if header_timestamp_nanos == 0 {
        return Err(Error::ZeroTimestamp);
    }
    let rlp = rlp::Rlp::new(rlp_bytes);
    let count = rlp
        .item_count()
        .map_err(|_| Error::InvalidExecutionBlockHeaderRlp)?;
    if count <= RLP_TIMESTAMP_INDEX {
        return Err(Error::InvalidExecutionBlockHeaderRlp);
    }
    let timestamp_secs: u64 = rlp
        .val_at(RLP_TIMESTAMP_INDEX)
        .map_err(|_| Error::InvalidExecutionBlockHeaderRlp)?;
    let expected = secs_to_nanos(timestamp_secs);
    if header_timestamp_nanos != expected {
        return Err(Error::UnexpectedTimestamp {
            expected,
            actual: header_timestamp_nanos,
        });
    }
    Ok(())
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

    #[test]
    fn test_validate_header_timestamp_success() {
        let ctx = test_context();
        let slot = Slot::from(10u64);
        let expected_secs = compute_timestamp_at_slot(&ctx, slot).0;
        let header_timestamp = secs_to_nanos(expected_secs);
        assert!(validate_header_timestamp(&ctx, slot, header_timestamp).is_ok());
    }

    #[test]
    fn test_validate_header_timestamp_zero() {
        let ctx = test_context();
        assert!(matches!(
            validate_header_timestamp(&ctx, Slot::from(10u64), 0),
            Err(Error::ZeroTimestamp)
        ));
    }

    #[test]
    fn test_validate_header_timestamp_mismatch() {
        let ctx = test_context();
        let slot = Slot::from(10u64);
        // off by one second from the slot-derived timestamp
        let expected_secs = compute_timestamp_at_slot(&ctx, slot).0;
        let header_timestamp = secs_to_nanos(expected_secs + 1);
        assert!(matches!(
            validate_header_timestamp(&ctx, slot, header_timestamp),
            Err(Error::UnexpectedTimestamp { .. })
        ));
    }
}

#[cfg(test)]
mod rlp_timestamp_tests {
    use super::*;

    /// Builds a minimal RLP list whose item at index 11 is `timestamp`.
    fn rlp_header_with_timestamp(timestamp: u64) -> Vec<u8> {
        let mut stream = rlp::RlpStream::new_list(12);
        for _ in 0..11 {
            stream.append(&0u64);
        }
        stream.append(&timestamp);
        stream.out().to_vec()
    }

    #[test]
    fn accepts_matching_timestamp() {
        let rlp = rlp_header_with_timestamp(1788508911);
        assert!(validate_header_timestamp_from_rlp(&rlp, secs_to_nanos(1788508911)).is_ok());
    }

    #[test]
    fn rejects_mismatching_timestamp() {
        let rlp = rlp_header_with_timestamp(1788508911);
        // the value the strict slot-based check would have expected
        match validate_header_timestamp_from_rlp(&rlp, secs_to_nanos(1788508917)) {
            Err(Error::UnexpectedTimestamp { expected, actual }) => {
                assert_eq!(expected, secs_to_nanos(1788508911));
                assert_eq!(actual, secs_to_nanos(1788508917));
            }
            other => panic!("expected UnexpectedTimestamp, got {other:?}"),
        }
    }

    #[test]
    fn rejects_zero_timestamp() {
        let rlp = rlp_header_with_timestamp(1788508911);
        assert!(matches!(
            validate_header_timestamp_from_rlp(&rlp, 0),
            Err(Error::ZeroTimestamp)
        ));
    }

    #[test]
    fn rejects_too_short_rlp() {
        let mut stream = rlp::RlpStream::new_list(5);
        for _ in 0..5 {
            stream.append(&0u64);
        }
        assert!(matches!(
            validate_header_timestamp_from_rlp(&stream.out(), secs_to_nanos(1)),
            Err(Error::InvalidExecutionBlockHeaderRlp)
        ));
    }
}
