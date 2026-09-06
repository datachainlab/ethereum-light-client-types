//! Error types for Ethereum light client operations.
//!
//! This module provides the [`Error`] enum which covers all error cases
//! that can occur during light client operations, including verification
//! failures, state transition errors, and serialization issues.

use crate::height::Height;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;
use ethereum_consensus::bls::PublicKey;
use ethereum_consensus::types::{H256, U64};

/// Error type for Ethereum light client operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ========================================================================
    // State transition errors
    // ========================================================================
    #[error("consensus update doesn't have next sync committee: store_period={store_period} update_period={update_period}")]
    NoNextSyncCommitteeInConsensusUpdate {
        store_period: U64,
        update_period: U64,
    },
    #[error("store does not support the finalized_period: store_period={store_period} finalized_period={finalized_period}")]
    StoreNotSupportedFinalizedPeriod {
        store_period: U64,
        finalized_period: U64,
    },
    #[error("unexpected height revision number: expected={expected} got={got}")]
    UnexpectedHeightRevisionNumber { expected: u64, got: u64 },
    #[error("unexpected proof height: proof_height={proof_height} latest_height={latest_height}")]
    UnexpectedProofHeight {
        proof_height: Height,
        latest_height: Height,
    },
    #[error("unexpected storage root is zero: proof_height={proof_height} latest_height={latest_height}")]
    UnexpectedStorageRoot {
        proof_height: Height,
        latest_height: Height,
    },

    // ========================================================================
    // Verification errors
    // ========================================================================
    #[error("mpt verification: {error:?} state_root={state_root} address={address} account_proof={account_proof:?}")]
    MptVerification {
        error: ethereum_light_client_verifier::errors::Error,
        state_root: H256,
        address: String,
        account_proof: Vec<String>,
    },
    #[error("account storage root mismatch: expected={expected} actual={actual} state_root={state_root} address={address} account_proof={account_proof:?}")]
    AccountStorageRootMismatch {
        expected: H256,
        actual: H256,
        state_root: H256,
        address: String,
        account_proof: Vec<String>,
    },
    #[error("membership verification: path={path} root={root} value={value:?} error={error:?}")]
    MembershipVerification {
        path: String,
        root: H256,
        value: Vec<u8>,
        error: ethereum_light_client_verifier::errors::Error,
    },
    #[error("non-membership verification: path={path} root={root} error={error:?}")]
    NonMembershipVerification {
        path: String,
        root: H256,
        error: ethereum_light_client_verifier::errors::Error,
    },
    #[error("invalid block hash merkle branch: {error:?}")]
    InvalidBlockHashMerkleBranch {
        error: ethereum_consensus::errors::MerkleError,
    },
    #[error("unexpected block hash: expected={expected:?} actual={actual:?}")]
    UnexpectedBlockHash { expected: H256, actual: H256 },
    #[error("invalid current sync committee keys: expected={expected:?} actual={actual:?}")]
    InvalidCurrentSyncCommitteeKeys {
        expected: PublicKey,
        actual: PublicKey,
    },
    #[error("invalid next sync committee keys: expected={expected:?} actual={actual:?}")]
    InvalidNextSyncCommitteeKeys {
        expected: PublicKey,
        actual: PublicKey,
    },

    // ========================================================================
    // Time validation errors (timestamps are unix nanoseconds)
    // ========================================================================
    #[error("current time {current} is before trusted state time {trusted}")]
    CurrentTimeBeforeTrustedState { current: u128, trusted: u128 },
    #[error("out of trusting period: current_timestamp={current_timestamp} trusting_period_end={trusting_period_end}")]
    OutOfTrustingPeriod {
        current_timestamp: u128,
        trusting_period_end: u128,
    },
    #[error("header is coming from future: current_timestamp={current_timestamp} clock_drift={clock_drift:?} header_timestamp={header_timestamp}")]
    HeaderFromFuture {
        current_timestamp: u128,
        clock_drift: Duration,
        header_timestamp: u128,
    },
    #[error("zero timestamp")]
    ZeroTimestamp,
    #[error("unexpected header timestamp: expected={expected} actual={actual}")]
    UnexpectedTimestamp { expected: u128, actual: u128 },
    #[error("invalid execution block header rlp")]
    InvalidExecutionBlockHeaderRlp,

    // ========================================================================
    // Serialization errors
    // ========================================================================
    #[error("invalid proof format: {message}")]
    InvalidProofFormat { message: String },
    #[error("deserialize sync committee bits: {error:?} sync_committee_size={sync_committee_size} sync_committee_bits={sync_committee_bits:?}")]
    DeserializeSyncCommitteeBits {
        error: ssz_rs::DeserializeError,
        sync_committee_size: usize,
        sync_committee_bits: Vec<u8>,
    },
    #[error("serialize sync committee bits: {error:?} sync_committee_size={sync_committee_size}")]
    SerializeSyncCommitteeBits {
        error: ssz_rs::SerializeError,
        sync_committee_size: usize,
    },
    #[error("proto missing field: {field}")]
    ProtoMissingField { field: String },
    #[error("invalid H256 length for field {field}: expected 32, got {got}")]
    InvalidH256Length { field: String, got: usize },
    #[error("invalid version length for field {field}: expected 4, got {got}")]
    InvalidVersionLength { field: String, got: usize },

    // ========================================================================
    // External library errors (with impl From)
    // ========================================================================
    #[error("ethereum consensus error: {0:?}")]
    EthereumConsensus(ethereum_consensus::errors::Error),
}

impl Error {
    pub fn proto_missing(s: &str) -> Self {
        Error::ProtoMissingField {
            field: s.to_string(),
        }
    }
}

impl From<ethereum_consensus::errors::Error> for Error {
    fn from(e: ethereum_consensus::errors::Error) -> Self {
        Error::EthereumConsensus(e)
    }
}
