//! Error types for Ethereum light client operations.
//!
//! This module provides the [`Error`] enum which covers all error cases
//! that can occur during light client operations, including verification
//! failures, state transition errors, and serialization issues.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::time::Duration;
use ethereum_consensus::bls::PublicKey;
use ethereum_consensus::types::{H256, U64};
use light_client::types::{ClientId, Height, Time};

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
    #[error("client is frozen: client_id={client_id}")]
    ClientFrozen { client_id: ClientId },
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
    // Time validation errors
    // ========================================================================
    #[error("current time {current} is before trusted state time {trusted}")]
    CurrentTimeBeforeTrustedState { current: Time, trusted: Time },
    #[error("out of trusting period: current_timestamp={current_timestamp} trusting_period_end={trusting_period_end}")]
    OutOfTrustingPeriod {
        current_timestamp: Time,
        trusting_period_end: Time,
    },
    #[error("header is coming from future: current_timestamp={current_timestamp} clock_drift={clock_drift:?} header_timestamp={header_timestamp}")]
    HeaderFromFuture {
        current_timestamp: Time,
        clock_drift: Duration,
        header_timestamp: Time,
    },
    #[error("zero timestamp")]
    ZeroTimestamp,
    #[error("unexpected header timestamp: expected={expected} actual={actual}")]
    UnexpectedTimestamp { expected: Time, actual: Time },

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
    #[error("proto missing field: {field}")]
    ProtoMissingField { field: String },

    // ========================================================================
    // External library errors (with impl From)
    // ========================================================================
    #[error("ethereum consensus error: {0:?}")]
    EthereumConsensus(ethereum_consensus::errors::Error),
    #[error("time error: {0:?}")]
    Time(light_client::types::TimeError),
    #[error("LCP error: {0:?}")]
    Lcp(light_client::Error),
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

impl From<light_client::types::TimeError> for Error {
    fn from(e: light_client::types::TimeError) -> Self {
        Error::Time(e)
    }
}

impl From<light_client::Error> for Error {
    fn from(e: light_client::Error) -> Self {
        Error::Lcp(e)
    }
}
