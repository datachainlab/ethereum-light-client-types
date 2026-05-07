//! Consensus update structures and Protocol Buffer conversions.
//!
//! This module provides data structures for Ethereum beacon chain consensus
//! updates and conversion functions between domain types and Protocol Buffer
//! message types.

use crate::commitment::decode_eip1184_rlp_proof;
use crate::errors::Error;
use alloc::vec::Vec;
use ethereum_consensus::beacon::{BeaconBlockHeader, Slot};
use ethereum_consensus::bls::{PublicKey, Signature};
use ethereum_consensus::sync_protocol::{SyncAggregate, SyncCommittee};
use ethereum_consensus::types::{H256, U64};
use ethereum_light_client_proto::ibc::core::client::v1::Height as ProtoHeight;
use ethereum_light_client_proto::ibc::lightclients::ethereum::v1::{
    AccountUpdate as ProtoAccountUpdate, BeaconBlockHeader as ProtoBeaconBlockHeader,
    ConsensusUpdate as ProtoConsensusUpdate, ExecutionUpdate as ProtoExecutionUpdate,
    SyncAggregate as ProtoSyncAggregate, SyncCommittee as ProtoSyncCommittee,
    TrustedSyncCommittee as ProtoTrustedSyncCommittee,
};
use ethereum_light_client_verifier::updates::{ConsensusUpdate, ExecutionUpdate};
use light_client::types::Height;
use ssz_rs::{Bitvector, Deserialize, Vector};

/// The revision number for Ethereum client heights.
///
/// This is always 0 for Ethereum clients.
pub const ETHEREUM_CLIENT_REVISION_NUMBER: u64 = 0;

/// Information for a beacon chain consensus update.
///
/// This struct contains all the data needed to verify and apply a
/// light client update, including the attested header, finalized header,
/// sync committee data, and aggregate signature.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConsensusUpdateInfo<const SYNC_COMMITTEE_SIZE: usize> {
    /// Header attested to by the sync committee
    pub attested_header: BeaconBlockHeader,
    /// Next sync committee contained in `attested_header.state_root`
    /// 0: sync committee
    /// 1: branch indicating the next sync committee in the tree corresponding to `attested_header.state_root`
    pub next_sync_committee: Option<(SyncCommittee<SYNC_COMMITTEE_SIZE>, Vec<H256>)>,
    /// Finalized header contained in `attested_header.state_root`
    /// 0: header
    /// 1. branch indicating the header in the tree corresponding to `attested_header.state_root`
    pub finalized_header: (BeaconBlockHeader, Vec<H256>),
    /// Sync committee aggregate signature
    pub sync_aggregate: SyncAggregate<SYNC_COMMITTEE_SIZE>,
    /// Slot at which the aggregate signature was created (untrusted)
    pub signature_slot: Slot,
    /// Execution payload contained in the finalized beacon block's body
    pub finalized_execution_root: H256,
    /// Execution payload branch indicating the payload in the tree corresponding to the finalized block's body
    pub finalized_execution_branch: Vec<H256>,
}

impl<const SYNC_COMMITTEE_SIZE: usize> ConsensusUpdate<SYNC_COMMITTEE_SIZE>
    for ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE>
{
    fn attested_beacon_header(&self) -> &BeaconBlockHeader {
        &self.attested_header
    }
    fn next_sync_committee(&self) -> Option<&SyncCommittee<SYNC_COMMITTEE_SIZE>> {
        self.next_sync_committee.as_ref().map(|c| &c.0)
    }
    fn next_sync_committee_branch(&self) -> Option<Vec<H256>> {
        self.next_sync_committee.as_ref().map(|c| c.1.to_vec())
    }
    fn finalized_beacon_header(&self) -> &BeaconBlockHeader {
        &self.finalized_header.0
    }
    fn finalized_beacon_header_branch(&self) -> Vec<H256> {
        self.finalized_header.1.to_vec()
    }
    fn sync_aggregate(&self) -> &SyncAggregate<SYNC_COMMITTEE_SIZE> {
        &self.sync_aggregate
    }
    fn signature_slot(&self) -> Slot {
        self.signature_slot
    }
    fn finalized_execution_root(&self) -> H256 {
        self.finalized_execution_root
    }
    fn finalized_execution_branch(&self) -> Vec<H256> {
        self.finalized_execution_branch.to_vec()
    }
}

/// Information for an execution layer update.
///
/// This struct contains the execution layer state root and block number
/// with their Merkle branches for verification.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionUpdateInfo {
    /// State root of the execution payload
    pub state_root: H256,
    /// Branch indicating the state root in the tree corresponding to the execution payload
    pub state_root_branch: Vec<H256>,
    /// Block number of the execution payload
    pub block_number: U64,
    /// Branch indicating the block number in the tree corresponding to the execution payload
    pub block_number_branch: Vec<H256>,
    /// RLP-encoded execution block header (for Gloas+)
    pub rlp: Vec<u8>,
    /// Block hash of the execution payload
    pub block_hash: H256,
    /// Branch indicating the block hash in the tree corresponding to the execution payload
    pub block_hash_branch: Vec<H256>,
}

impl ExecutionUpdate for ExecutionUpdateInfo {
    fn state_root(&self) -> H256 {
        self.state_root
    }

    fn state_root_branch(&self) -> Vec<H256> {
        self.state_root_branch.clone()
    }

    fn block_number(&self) -> U64 {
        self.block_number
    }

    fn block_number_branch(&self) -> Vec<H256> {
        self.block_number_branch.clone()
    }

    fn rlp(&self) -> Vec<u8> {
        self.rlp.clone()
    }
}

/// A trusted sync committee with its associated height.
///
/// This struct represents the sync committee that has been verified and
/// is trusted for validating future updates.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TrustedSyncCommittee<const SYNC_COMMITTEE_SIZE: usize> {
    /// Height (execution block number) at which this sync committee is stored
    pub height: Height,
    /// trusted sync committee
    pub sync_committee: SyncCommittee<SYNC_COMMITTEE_SIZE>,
    /// since the consensus state contains a current and next sync committee, this flag determines which one to refer to
    pub is_next: bool,
}

impl<const SYNC_COMMITTEE_SIZE: usize> TrustedSyncCommittee<SYNC_COMMITTEE_SIZE> {
    /// Validates the trusted sync committee.
    ///
    /// Checks that the height has the correct revision number and that
    /// the sync committee is valid.
    ///
    /// # Errors
    ///
    /// - [`Error::UnexpectedHeightRevisionNumber`]: Wrong revision number
    /// - Sync committee validation errors
    pub fn validate(&self) -> Result<(), Error> {
        if self.height.revision_number() != ETHEREUM_CLIENT_REVISION_NUMBER {
            return Err(Error::UnexpectedHeightRevisionNumber {
                expected: ETHEREUM_CLIENT_REVISION_NUMBER,
                got: self.height.revision_number(),
            });
        }
        self.sync_committee.validate()?;
        Ok(())
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> TryFrom<ProtoTrustedSyncCommittee>
    for TrustedSyncCommittee<SYNC_COMMITTEE_SIZE>
{
    type Error = Error;

    fn try_from(value: ProtoTrustedSyncCommittee) -> Result<Self, Error> {
        let trusted_height = value
            .trusted_height
            .as_ref()
            .ok_or(Error::proto_missing("trusted_height"))?;
        Ok(TrustedSyncCommittee {
            height: Height::new(
                trusted_height.revision_number,
                trusted_height.revision_height,
            ),
            sync_committee: SyncCommittee {
                pubkeys: Vector::<PublicKey, SYNC_COMMITTEE_SIZE>::from_iter(
                    value
                        .sync_committee
                        .as_ref()
                        .ok_or(Error::proto_missing("sync_committee"))?
                        .pubkeys
                        .clone()
                        .into_iter()
                        .map(|pk| pk.try_into())
                        .collect::<Result<Vec<PublicKey>, _>>()?,
                ),
                aggregate_pubkey: PublicKey::try_from(
                    value
                        .sync_committee
                        .ok_or(Error::proto_missing("sync_committee"))?
                        .aggregate_pubkey,
                )?,
            },
            is_next: value.is_next,
        })
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize> From<TrustedSyncCommittee<SYNC_COMMITTEE_SIZE>>
    for ProtoTrustedSyncCommittee
{
    fn from(value: TrustedSyncCommittee<SYNC_COMMITTEE_SIZE>) -> Self {
        Self {
            trusted_height: Some(ProtoHeight {
                revision_number: value.height.revision_number(),
                revision_height: value.height.revision_height(),
            }),
            sync_committee: Some(ProtoSyncCommittee {
                pubkeys: value
                    .sync_committee
                    .pubkeys
                    .iter()
                    .map(|pk| pk.to_vec())
                    .collect(),
                aggregate_pubkey: value.sync_committee.aggregate_pubkey.to_vec(),
            }),
            is_next: value.is_next,
        }
    }
}

/// Information for an account state update proof.
///
/// This struct contains the Merkle-Patricia Trie proof for an account
/// and its expected storage root.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccountUpdateInfo {
    /// Merkle-Patricia Trie proof for the account.
    pub account_proof: Vec<Vec<u8>>,
    /// Expected storage root of the account.
    pub account_storage_root: H256,
}

impl From<AccountUpdateInfo> for ProtoAccountUpdate {
    fn from(value: AccountUpdateInfo) -> Self {
        Self {
            account_proof: encode_account_proof(value.account_proof),
            account_storage_root: value.account_storage_root.as_bytes().to_vec(),
        }
    }
}

impl TryFrom<ProtoAccountUpdate> for AccountUpdateInfo {
    type Error = Error;
    fn try_from(value: ProtoAccountUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            account_proof: decode_eip1184_rlp_proof(value.account_proof)?,
            account_storage_root: H256::from_slice(&value.account_storage_root),
        })
    }
}

fn encode_account_proof(bz: Vec<Vec<u8>>) -> Vec<u8> {
    let proof: Vec<Vec<u8>> = bz.into_iter().map(|b| b.to_vec()).collect();
    let mut stream = rlp::RlpStream::new();
    stream.begin_list(proof.len());
    for p in proof.iter() {
        stream.append_raw(p, 1);
    }
    stream.out().freeze().into()
}

pub(crate) fn convert_proto_to_header(
    header: &ProtoBeaconBlockHeader,
) -> Result<BeaconBlockHeader, Error> {
    Ok(BeaconBlockHeader {
        slot: header.slot.into(),
        proposer_index: header.proposer_index.into(),
        parent_root: H256::from_slice(&header.parent_root),
        state_root: H256::from_slice(&header.state_root),
        body_root: H256::from_slice(&header.body_root),
    })
}

pub(crate) fn convert_header_to_proto(header: &BeaconBlockHeader) -> ProtoBeaconBlockHeader {
    ProtoBeaconBlockHeader {
        slot: header.slot.into(),
        proposer_index: header.proposer_index.into(),
        parent_root: header.parent_root.as_bytes().to_vec(),
        state_root: header.state_root.as_bytes().to_vec(),
        body_root: header.body_root.as_bytes().to_vec(),
    }
}

/// Converts a Protocol Buffer execution update to the domain type.
pub fn convert_proto_to_execution_update(
    execution_update: ProtoExecutionUpdate,
) -> ExecutionUpdateInfo {
    ExecutionUpdateInfo {
        state_root: H256::from_slice(&execution_update.state_root),
        state_root_branch: execution_update
            .state_root_branch
            .into_iter()
            .map(|n| H256::from_slice(&n))
            .collect(),
        block_number: execution_update.block_number.into(),
        block_number_branch: execution_update
            .block_number_branch
            .into_iter()
            .map(|n| H256::from_slice(&n))
            .collect(),
        rlp: execution_update.rlp,
        block_hash: H256::from_slice(&execution_update.block_hash),
        block_hash_branch: execution_update
            .block_hash_branch
            .into_iter()
            .map(|n| H256::from_slice(&n))
            .collect(),
    }
}

/// Converts an execution update to the Protocol Buffer type.
pub fn convert_execution_update_to_proto(
    execution_update: ExecutionUpdateInfo,
) -> ProtoExecutionUpdate {
    ProtoExecutionUpdate {
        state_root: execution_update.state_root.as_bytes().into(),
        state_root_branch: execution_update
            .state_root_branch
            .into_iter()
            .map(|n| n.as_bytes().to_vec())
            .collect(),
        block_number: execution_update.block_number.into(),
        block_number_branch: execution_update
            .block_number_branch
            .into_iter()
            .map(|n| n.as_bytes().to_vec())
            .collect(),
        rlp: execution_update.rlp,
        block_hash: execution_update.block_hash.as_bytes().into(),
        block_hash_branch: execution_update
            .block_hash_branch
            .into_iter()
            .map(|n| n.as_bytes().to_vec())
            .collect(),
    }
}

/// Converts a sync aggregate to the Protocol Buffer type.
///
/// # Panics
///
/// Panics if `SYNC_COMMITTEE_SIZE` is 0.
pub fn convert_sync_aggregate_to_proto<const SYNC_COMMITTEE_SIZE: usize>(
    sync_aggregate: SyncAggregate<SYNC_COMMITTEE_SIZE>,
) -> ProtoSyncAggregate {
    let sync_committee_bits = ssz_rs::serialize(&sync_aggregate.sync_committee_bits)
        .expect("failed to serialize sync_committee_bits: this should never happen unless `SYNC_COMMITTEE_SIZE` is 0");
    ProtoSyncAggregate {
        sync_committee_bits,
        sync_committee_signature: sync_aggregate.sync_committee_signature.0.to_vec(),
    }
}

pub(crate) fn convert_proto_sync_aggregate<const SYNC_COMMITTEE_SIZE: usize>(
    sync_aggregate: ProtoSyncAggregate,
) -> Result<SyncAggregate<SYNC_COMMITTEE_SIZE>, Error> {
    Ok(SyncAggregate {
        sync_committee_bits: Bitvector::<SYNC_COMMITTEE_SIZE>::deserialize(
            sync_aggregate.sync_committee_bits.as_slice(),
        )
        .map_err(|e| Error::DeserializeSyncCommitteeBits {
            error: e,
            sync_committee_size: SYNC_COMMITTEE_SIZE,
            sync_committee_bits: sync_aggregate.sync_committee_bits,
        })?,
        sync_committee_signature: Signature::try_from(sync_aggregate.sync_committee_signature)?,
    })
}

/// Converts a consensus update to the Protocol Buffer type.
pub fn convert_consensus_update_to_proto<const SYNC_COMMITTEE_SIZE: usize>(
    consensus_update: ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE>,
) -> ProtoConsensusUpdate {
    let finalized_beacon_header_branch = consensus_update.finalized_beacon_header_branch();
    let sync_aggregate = consensus_update.sync_aggregate.clone();

    ProtoConsensusUpdate {
        attested_header: Some(convert_header_to_proto(&consensus_update.attested_header)),
        next_sync_committee: consensus_update.next_sync_committee.clone().map(|c| {
            ProtoSyncCommittee {
                pubkeys: c.0.pubkeys.iter().map(|pk| pk.to_vec()).collect(),
                aggregate_pubkey: c.0.aggregate_pubkey.to_vec(),
            }
        }),
        next_sync_committee_branch: consensus_update
            .next_sync_committee
            .map_or(Vec::new(), |(_, branch)| {
                branch.into_iter().map(|n| n.as_bytes().to_vec()).collect()
            }),
        finalized_header: Some(convert_header_to_proto(
            &consensus_update.finalized_header.0,
        )),
        finalized_header_branch: finalized_beacon_header_branch
            .into_iter()
            .map(|n| n.as_bytes().to_vec())
            .collect(),
        finalized_execution_root: consensus_update.finalized_execution_root.as_bytes().into(),
        finalized_execution_branch: consensus_update
            .finalized_execution_branch
            .into_iter()
            .map(|n| n.as_bytes().to_vec())
            .collect(),
        sync_aggregate: Some(convert_sync_aggregate_to_proto(sync_aggregate)),
        signature_slot: consensus_update.signature_slot.into(),
    }
}

/// Converts a Protocol Buffer consensus update to the domain type.
///
/// # Errors
///
/// Returns an error if required fields are missing or invalid.
pub fn convert_proto_to_consensus_update<const SYNC_COMMITTEE_SIZE: usize>(
    consensus_update: ProtoConsensusUpdate,
) -> Result<ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE>, Error> {
    let attested_header = convert_proto_to_header(
        consensus_update
            .attested_header
            .as_ref()
            .ok_or(Error::proto_missing("attested_header"))?,
    )?;
    let finalized_header = convert_proto_to_header(
        consensus_update
            .finalized_header
            .as_ref()
            .ok_or(Error::proto_missing("finalized_header"))?,
    )?;

    let finalized_execution_branch = consensus_update
        .finalized_execution_branch
        .into_iter()
        .map(|b| H256::from_slice(&b))
        .collect::<Vec<H256>>();
    let consensus_update = ConsensusUpdateInfo {
        attested_header,
        next_sync_committee: if consensus_update.next_sync_committee.is_none()
            || consensus_update
                .next_sync_committee
                .as_ref()
                .ok_or(Error::proto_missing("next_sync_committee"))?
                .pubkeys
                .is_empty()
            || consensus_update.next_sync_committee_branch.is_empty()
        {
            None
        } else {
            Some((
                SyncCommittee {
                    pubkeys: Vector::<PublicKey, SYNC_COMMITTEE_SIZE>::from_iter(
                        consensus_update
                            .next_sync_committee
                            .clone()
                            .ok_or(Error::proto_missing("next_sync_committee"))?
                            .pubkeys
                            .into_iter()
                            .map(|pk| pk.try_into())
                            .collect::<Result<Vec<PublicKey>, _>>()?,
                    ),
                    aggregate_pubkey: PublicKey::try_from(
                        consensus_update
                            .next_sync_committee
                            .ok_or(Error::proto_missing("next_sync_committee"))?
                            .aggregate_pubkey,
                    )?,
                },
                decode_branch(consensus_update.next_sync_committee_branch),
            ))
        },
        finalized_header: (
            finalized_header,
            decode_branch(consensus_update.finalized_header_branch),
        ),
        sync_aggregate: convert_proto_sync_aggregate(
            consensus_update
                .sync_aggregate
                .ok_or(Error::proto_missing("sync_aggregate"))?,
        )?,
        signature_slot: consensus_update.signature_slot.into(),
        finalized_execution_root: H256::from_slice(&consensus_update.finalized_execution_root),
        finalized_execution_branch,
    };
    Ok(consensus_update)
}

pub(crate) fn decode_branch(bz: Vec<Vec<u8>>) -> Vec<H256> {
    bz.into_iter().map(|b| H256::from_slice(&b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    // Common test constants
    const TEST_SYNC_COMMITTEE_SIZE: usize = 32;

    // Helper function to create H256 from a single byte value
    fn h256_from_byte(byte: u8) -> H256 {
        H256::from_slice(&[byte; 32])
    }

    #[test]
    fn test_execution_update_info_default() {
        let update = ExecutionUpdateInfo::default();
        assert!(update.state_root.is_zero());
        assert!(update.state_root_branch.is_empty());
        assert_eq!(update.block_number, U64::from(0));
        assert!(update.block_number_branch.is_empty());
        assert!(update.rlp.is_empty());
    }

    #[test]
    fn test_execution_update_trait_impl() {
        let update = ExecutionUpdateInfo {
            state_root: h256_from_byte(1),
            state_root_branch: vec![h256_from_byte(2)],
            block_number: U64::from(12345),
            block_number_branch: vec![h256_from_byte(3)],
            rlp: vec![0xde, 0xad, 0xbe, 0xef],
            block_hash: h256_from_byte(4),
            block_hash_branch: vec![h256_from_byte(5)],
        };

        assert_eq!(update.state_root(), h256_from_byte(1));
        assert_eq!(update.state_root_branch().len(), 1);
        assert_eq!(update.block_number(), U64::from(12345));
        assert_eq!(update.block_number_branch().len(), 1);
        assert_eq!(update.rlp(), vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn test_execution_update_proto_roundtrip() {
        let original = ExecutionUpdateInfo {
            state_root: h256_from_byte(1),
            state_root_branch: vec![h256_from_byte(2), h256_from_byte(3)],
            block_number: U64::from(99999),
            block_number_branch: vec![h256_from_byte(4)],
            rlp: vec![1, 2, 3, 4, 5],
            block_hash: h256_from_byte(5),
            block_hash_branch: vec![h256_from_byte(6), h256_from_byte(7)],
        };

        let proto = convert_execution_update_to_proto(original.clone());
        let converted = convert_proto_to_execution_update(proto);

        assert_eq!(original.state_root, converted.state_root);
        assert_eq!(original.state_root_branch, converted.state_root_branch);
        assert_eq!(original.block_number, converted.block_number);
        assert_eq!(original.block_number_branch, converted.block_number_branch);
        assert_eq!(original.rlp, converted.rlp);
        assert_eq!(original.block_hash, converted.block_hash);
        assert_eq!(original.block_hash_branch, converted.block_hash_branch);
    }

    #[test]
    fn test_account_update_info_default() {
        let update = AccountUpdateInfo::default();
        assert!(update.account_proof.is_empty());
        assert!(update.account_storage_root.is_zero());
    }

    #[test]
    fn test_decode_branch() {
        let input = vec![vec![1u8; 32], vec![2u8; 32]];
        let result = decode_branch(input);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], h256_from_byte(1));
        assert_eq!(result[1], h256_from_byte(2));
    }

    #[test]
    fn test_decode_branch_empty() {
        let input: Vec<Vec<u8>> = vec![];
        let result = decode_branch(input);
        assert!(result.is_empty());
    }

    #[test]
    fn test_convert_header_roundtrip() {
        let original = BeaconBlockHeader {
            slot: 12345.into(),
            proposer_index: 42.into(),
            parent_root: h256_from_byte(1),
            state_root: h256_from_byte(2),
            body_root: h256_from_byte(3),
        };

        let proto = convert_header_to_proto(&original);
        let converted = convert_proto_to_header(&proto).unwrap();

        assert_eq!(original.slot, converted.slot);
        assert_eq!(original.proposer_index, converted.proposer_index);
        assert_eq!(original.parent_root, converted.parent_root);
        assert_eq!(original.state_root, converted.state_root);
        assert_eq!(original.body_root, converted.body_root);
    }

    #[test]
    fn test_trusted_sync_committee_validate_success() {
        use ethereum_light_client_verifier::consensus::test_utils::MockSyncCommittee;

        let mock = MockSyncCommittee::<TEST_SYNC_COMMITTEE_SIZE>::new();
        let valid_sync_committee = mock.to_committee();

        let tsc = TrustedSyncCommittee::<TEST_SYNC_COMMITTEE_SIZE> {
            height: Height::new(0, 100),
            sync_committee: valid_sync_committee,
            is_next: false,
        };

        let result = tsc.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_trusted_sync_committee_validate_wrong_revision() {
        let tsc = TrustedSyncCommittee::<TEST_SYNC_COMMITTEE_SIZE> {
            height: Height::new(1, 100), // revision_number = 1, should be 0
            sync_committee: SyncCommittee::default(),
            is_next: false,
        };

        let result = tsc.validate();
        assert!(result.is_err());
        match result {
            Err(Error::UnexpectedHeightRevisionNumber { expected, got }) => {
                assert_eq!(expected, 0);
                assert_eq!(got, 1);
            }
            _ => panic!("Expected UnexpectedHeightRevisionNumber error"),
        }
    }

    #[test]
    fn test_ethereum_client_revision_number_constant() {
        assert_eq!(ETHEREUM_CLIENT_REVISION_NUMBER, 0);
    }

    #[test]
    fn test_consensus_update_info_default() {
        let update = ConsensusUpdateInfo::<TEST_SYNC_COMMITTEE_SIZE>::default();

        assert_eq!(update.attested_header.slot, 0.into());
        assert!(update.next_sync_committee.is_none());
        assert!(update.finalized_header.1.is_empty());
        assert!(update.finalized_execution_root.is_zero());
        assert!(update.finalized_execution_branch.is_empty());
    }

    #[test]
    fn test_consensus_update_trait_impl() {
        let update = ConsensusUpdateInfo::<TEST_SYNC_COMMITTEE_SIZE> {
            attested_header: BeaconBlockHeader {
                slot: 100.into(),
                ..Default::default()
            },
            next_sync_committee: None,
            finalized_header: (
                BeaconBlockHeader {
                    slot: 50.into(),
                    ..Default::default()
                },
                vec![h256_from_byte(1)],
            ),
            sync_aggregate: SyncAggregate::default(),
            signature_slot: 101.into(),
            finalized_execution_root: h256_from_byte(2),
            finalized_execution_branch: vec![h256_from_byte(3)],
        };

        assert_eq!(update.attested_beacon_header().slot, 100.into());
        assert!(update.next_sync_committee().is_none());
        assert!(update.next_sync_committee_branch().is_none());
        assert_eq!(update.finalized_beacon_header().slot, 50.into());
        assert_eq!(update.finalized_beacon_header_branch().len(), 1);
        assert_eq!(update.signature_slot(), 101.into());
        assert_eq!(update.finalized_execution_root(), h256_from_byte(2));
        assert_eq!(update.finalized_execution_branch().len(), 1);
    }

    #[test]
    fn test_sync_aggregate_proto_roundtrip() {
        let original = SyncAggregate::<TEST_SYNC_COMMITTEE_SIZE>::default();
        let proto = convert_sync_aggregate_to_proto(original.clone());
        let converted = convert_proto_sync_aggregate::<TEST_SYNC_COMMITTEE_SIZE>(proto).unwrap();

        assert_eq!(original.sync_committee_bits, converted.sync_committee_bits);
        assert_eq!(
            original.sync_committee_signature,
            converted.sync_committee_signature
        );
    }

    #[test]
    fn test_consensus_update_proto_roundtrip() {
        let original = ConsensusUpdateInfo::<TEST_SYNC_COMMITTEE_SIZE> {
            attested_header: BeaconBlockHeader {
                slot: 100.into(),
                proposer_index: 5.into(),
                parent_root: h256_from_byte(1),
                state_root: h256_from_byte(2),
                body_root: h256_from_byte(3),
            },
            next_sync_committee: None,
            finalized_header: (
                BeaconBlockHeader {
                    slot: 64.into(),
                    proposer_index: 3.into(),
                    parent_root: h256_from_byte(4),
                    state_root: h256_from_byte(5),
                    body_root: h256_from_byte(6),
                },
                vec![h256_from_byte(7), h256_from_byte(8)],
            ),
            sync_aggregate: SyncAggregate::default(),
            signature_slot: 101.into(),
            finalized_execution_root: h256_from_byte(9),
            finalized_execution_branch: vec![h256_from_byte(10)],
        };

        let proto = convert_consensus_update_to_proto(original.clone());
        let converted =
            convert_proto_to_consensus_update::<TEST_SYNC_COMMITTEE_SIZE>(proto).unwrap();

        assert_eq!(
            original.attested_header.slot,
            converted.attested_header.slot
        );
        assert_eq!(
            original.attested_header.proposer_index,
            converted.attested_header.proposer_index
        );
        assert_eq!(
            original.attested_header.parent_root,
            converted.attested_header.parent_root
        );
        assert!(converted.next_sync_committee.is_none());
        assert_eq!(
            original.finalized_header.0.slot,
            converted.finalized_header.0.slot
        );
        assert_eq!(
            original.finalized_header.1.len(),
            converted.finalized_header.1.len()
        );
        assert_eq!(original.signature_slot, converted.signature_slot);
        assert_eq!(
            original.finalized_execution_root,
            converted.finalized_execution_root
        );
        assert_eq!(
            original.finalized_execution_branch.len(),
            converted.finalized_execution_branch.len()
        );
    }

    #[test]
    fn test_trusted_sync_committee_proto_roundtrip() {
        let original = TrustedSyncCommittee::<TEST_SYNC_COMMITTEE_SIZE> {
            height: Height::new(0, 12345),
            sync_committee: SyncCommittee::default(),
            is_next: true,
        };

        let proto: ProtoTrustedSyncCommittee = original.clone().into();
        let converted = TrustedSyncCommittee::<TEST_SYNC_COMMITTEE_SIZE>::try_from(proto).unwrap();

        assert_eq!(original.height, converted.height);
        assert_eq!(original.is_next, converted.is_next);
        assert_eq!(
            original.sync_committee.aggregate_pubkey,
            converted.sync_committee.aggregate_pubkey
        );
    }

    #[test]
    fn test_trusted_sync_committee_proto_missing_height() {
        let proto = ProtoTrustedSyncCommittee {
            trusted_height: None,
            sync_committee: Some(ProtoSyncCommittee {
                pubkeys: vec![],
                aggregate_pubkey: vec![0u8; 48],
            }),
            is_next: false,
        };

        let result = TrustedSyncCommittee::<TEST_SYNC_COMMITTEE_SIZE>::try_from(proto);
        assert!(result.is_err());
    }

    #[test]
    fn test_trusted_sync_committee_proto_missing_sync_committee() {
        let proto = ProtoTrustedSyncCommittee {
            trusted_height: Some(ProtoHeight {
                revision_number: 0,
                revision_height: 100,
            }),
            sync_committee: None,
            is_next: false,
        };

        let result = TrustedSyncCommittee::<TEST_SYNC_COMMITTEE_SIZE>::try_from(proto);
        assert!(result.is_err());
    }

    fn create_test_rlp_proof() -> Vec<u8> {
        let mut stream = rlp::RlpStream::new_list(1);
        stream.begin_list(2);
        stream.append(&vec![1u8, 2, 3]);
        stream.append(&vec![4u8, 5, 6]);
        stream.out().to_vec()
    }

    #[test]
    fn test_account_update_proto_roundtrip() {
        let original_proof =
            crate::commitment::decode_eip1184_rlp_proof(create_test_rlp_proof()).unwrap();

        let original = AccountUpdateInfo {
            account_proof: original_proof,
            account_storage_root: h256_from_byte(1),
        };

        let proto: ProtoAccountUpdate = original.clone().into();
        let converted = AccountUpdateInfo::try_from(proto).unwrap();

        assert_eq!(
            original.account_storage_root,
            converted.account_storage_root
        );
        assert_eq!(original.account_proof.len(), converted.account_proof.len());
    }

    #[test]
    fn test_consensus_update_with_next_sync_committee() {
        let sync_committee = SyncCommittee::<TEST_SYNC_COMMITTEE_SIZE>::default();
        let branch = vec![h256_from_byte(11), h256_from_byte(12)];

        let original = ConsensusUpdateInfo::<TEST_SYNC_COMMITTEE_SIZE> {
            attested_header: BeaconBlockHeader::default(),
            next_sync_committee: Some((sync_committee.clone(), branch.clone())),
            finalized_header: (BeaconBlockHeader::default(), vec![]),
            sync_aggregate: SyncAggregate::default(),
            signature_slot: 100.into(),
            finalized_execution_root: H256::default(),
            finalized_execution_branch: vec![],
        };

        let proto = convert_consensus_update_to_proto(original.clone());

        // Verify proto has next_sync_committee
        assert!(proto.next_sync_committee.is_some());
        assert!(!proto.next_sync_committee_branch.is_empty());

        let converted =
            convert_proto_to_consensus_update::<TEST_SYNC_COMMITTEE_SIZE>(proto).unwrap();

        assert!(converted.next_sync_committee.is_some());
        let (converted_committee, converted_branch) = converted.next_sync_committee.unwrap();
        assert_eq!(
            sync_committee.aggregate_pubkey,
            converted_committee.aggregate_pubkey
        );
        assert_eq!(branch.len(), converted_branch.len());
    }

    fn create_valid_proto_beacon_header() -> ProtoBeaconBlockHeader {
        ProtoBeaconBlockHeader {
            slot: 100,
            proposer_index: 5,
            parent_root: vec![0u8; 32],
            state_root: vec![0u8; 32],
            body_root: vec![0u8; 32],
        }
    }

    #[test]
    fn test_convert_proto_to_consensus_update_missing_attested_header() {
        let proto = ProtoConsensusUpdate {
            attested_header: None,
            next_sync_committee: None,
            next_sync_committee_branch: vec![],
            finalized_header: Some(create_valid_proto_beacon_header()),
            finalized_header_branch: vec![],
            finalized_execution_root: vec![0u8; 32],
            finalized_execution_branch: vec![],
            sync_aggregate: Some(ProtoSyncAggregate {
                sync_committee_bits: vec![0u8; TEST_SYNC_COMMITTEE_SIZE / 8],
                sync_committee_signature: vec![0u8; 96],
            }),
            signature_slot: 100,
        };

        let result = convert_proto_to_consensus_update::<TEST_SYNC_COMMITTEE_SIZE>(proto);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_proto_to_consensus_update_missing_finalized_header() {
        let proto = ProtoConsensusUpdate {
            attested_header: Some(create_valid_proto_beacon_header()),
            next_sync_committee: None,
            next_sync_committee_branch: vec![],
            finalized_header: None,
            finalized_header_branch: vec![],
            finalized_execution_root: vec![0u8; 32],
            finalized_execution_branch: vec![],
            sync_aggregate: Some(ProtoSyncAggregate {
                sync_committee_bits: vec![0u8; TEST_SYNC_COMMITTEE_SIZE / 8],
                sync_committee_signature: vec![0u8; 96],
            }),
            signature_slot: 100,
        };

        let result = convert_proto_to_consensus_update::<TEST_SYNC_COMMITTEE_SIZE>(proto);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_proto_to_consensus_update_missing_sync_aggregate() {
        let proto = ProtoConsensusUpdate {
            attested_header: Some(create_valid_proto_beacon_header()),
            next_sync_committee: None,
            next_sync_committee_branch: vec![],
            finalized_header: Some(create_valid_proto_beacon_header()),
            finalized_header_branch: vec![],
            finalized_execution_root: vec![0u8; 32],
            finalized_execution_branch: vec![],
            sync_aggregate: None,
            signature_slot: 100,
        };

        let result = convert_proto_to_consensus_update::<TEST_SYNC_COMMITTEE_SIZE>(proto);
        assert!(result.is_err());
    }
}
