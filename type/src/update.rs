//! Sync committee state transition logic.
//!
//! This module handles the application of verified consensus updates to
//! transition between sync committee periods.

use crate::consensus::ConsensusUpdateInfo;
use crate::errors::Error;
use ethereum_consensus::bls::PublicKey;
use ethereum_consensus::sync_protocol::{SyncCommittee, SyncCommitteePeriod};
use ethereum_consensus::types::U64;
use ethereum_consensus::{compute::compute_sync_committee_period_at_slot, context::ChainContext};
use ethereum_light_client_verifier::state::LightClientStoreReader;
use ethereum_light_client_verifier::updates::ConsensusUpdate;

/// Information about the current and next sync committees.
///
/// This struct holds the aggregate public keys of both sync committees,
/// which are used to verify beacon chain signatures.
pub struct SyncCommitteeInfo {
    /// Aggregate public key of the current sync committee.
    pub current_sync_committee: PublicKey,
    /// Aggregate public key of the next sync committee.
    pub next_sync_committee: PublicKey,
}

/// Trait for accessing trusted sync committee information.
///
/// Implementors provide access to the sync committee data stored in
/// a consensus state, enabling state transition calculations.
pub trait TrustedSyncCommitteeInfo {
    /// Returns the sync committee period of the current sync committee.
    fn current_period<C: ChainContext>(&self, ctx: &C) -> SyncCommitteePeriod;
    /// Returns the aggregate public key of the current sync committee.
    fn current_sync_committee(&self) -> PublicKey;
    /// Returns the aggregate public key of the next sync committee.
    fn next_sync_committee(&self) -> PublicKey;
    /// Returns true if the update is relevant (e.g., finalized slot is newer than current).
    fn is_relevant_update(&self, update_finalized_slot: U64) -> bool;
}

/// Computes the new sync committee info based on the consensus update.
///
/// This function determines the current and next sync committees after applying
/// the given consensus update. It does NOT update other state fields like
/// `storage_root`, `timestamp`, or `latest_execution_block_number` - callers
/// must handle those separately.
///
/// # Contract
///
/// This function must be called after `SyncProtocolVerifier::validate_updates()`.
/// The update must satisfy:
/// - `finalized_period <= attested_period <= signature_period`
/// - `consensus_update`'s signature period is in `(store_period, store_period + 1)`
///
/// # State Transition
///
/// - If `store_period == update_finalized_period`: sync committee info remains unchanged
/// - If `store_period + 1 == update_finalized_period`: advances to next sync committee
///
/// # Errors
///
/// - [`Error::NoNextSyncCommitteeInConsensusUpdate`]: Update missing next sync committee
/// - [`Error::StoreNotSupportedFinalizedPeriod`]: Period gap too large
pub fn compute_sync_committees<
    const SYNC_COMMITTEE_SIZE: usize,
    CC: ChainContext,
    CS: TrustedSyncCommitteeInfo,
>(
    ctx: &CC,
    trusted_sync_committee: &CS,
    consensus_update: ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE>,
) -> Result<SyncCommitteeInfo, Error> {
    let store_period = trusted_sync_committee.current_period(ctx);
    let update_finalized_slot = consensus_update.finalized_header.0.slot;
    let update_finalized_period = compute_sync_committee_period_at_slot(ctx, update_finalized_slot);

    if store_period == update_finalized_period {
        // store_period == finalized_period <= attested_period <= signature_period
        Ok(SyncCommitteeInfo {
            current_sync_committee: trusted_sync_committee.current_sync_committee(),
            next_sync_committee: trusted_sync_committee.next_sync_committee(),
        })
    } else if store_period + 1 == update_finalized_period {
        // store_period + 1 == finalized_period == attested_period == signature_period
        if let Some((update_next_sync_committee, _)) = consensus_update.next_sync_committee {
            Ok(SyncCommitteeInfo {
                current_sync_committee: trusted_sync_committee.next_sync_committee(),
                next_sync_committee: update_next_sync_committee.aggregate_pubkey,
            })
        } else {
            Err(Error::NoNextSyncCommitteeInConsensusUpdate {
                store_period,
                update_period: update_finalized_period,
            })
        }
    } else {
        Err(Error::StoreNotSupportedFinalizedPeriod {
            store_period,
            finalized_period: update_finalized_period,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedConsensusState<const SYNC_COMMITTEE_SIZE: usize, CS: TrustedSyncCommitteeInfo> {
    state: CS,
    current_sync_committee: Option<SyncCommittee<SYNC_COMMITTEE_SIZE>>,
    next_sync_committee: Option<SyncCommittee<SYNC_COMMITTEE_SIZE>>,
}

impl<const SYNC_COMMITTEE_SIZE: usize, CS: TrustedSyncCommitteeInfo>
    TrustedConsensusState<SYNC_COMMITTEE_SIZE, CS>
{
    pub fn new(
        trusted_sync_committee_info: CS,
        update_sync_committee: SyncCommittee<SYNC_COMMITTEE_SIZE>,
        is_next: bool,
    ) -> Result<Self, Error> {
        update_sync_committee.validate()?;
        if !is_next {
            return if update_sync_committee.aggregate_pubkey
                == trusted_sync_committee_info.current_sync_committee()
            {
                Ok(Self {
                    state: trusted_sync_committee_info,
                    current_sync_committee: Some(update_sync_committee),
                    next_sync_committee: None,
                })
            } else {
                Err(Error::InvalidCurrentSyncCommitteeKeys {
                    expected: trusted_sync_committee_info.current_sync_committee(),
                    actual: update_sync_committee.aggregate_pubkey,
                })
            };
        }

        if update_sync_committee.aggregate_pubkey
            == trusted_sync_committee_info.next_sync_committee()
        {
            Ok(Self {
                state: trusted_sync_committee_info,
                current_sync_committee: None,
                next_sync_committee: Some(update_sync_committee),
            })
        } else {
            Err(Error::InvalidNextSyncCommitteeKeys {
                expected: trusted_sync_committee_info.next_sync_committee(),
                actual: update_sync_committee.aggregate_pubkey,
            })
        }
    }
}

impl<const SYNC_COMMITTEE_SIZE: usize, CS: TrustedSyncCommitteeInfo>
    LightClientStoreReader<SYNC_COMMITTEE_SIZE> for TrustedConsensusState<SYNC_COMMITTEE_SIZE, CS>
{
    fn current_period<C: ChainContext>(&self, ctx: &C) -> SyncCommitteePeriod {
        self.state.current_period(ctx)
    }

    fn current_sync_committee(&self) -> Option<SyncCommittee<SYNC_COMMITTEE_SIZE>> {
        self.current_sync_committee.clone()
    }

    fn next_sync_committee(&self) -> Option<SyncCommittee<SYNC_COMMITTEE_SIZE>> {
        self.next_sync_committee.clone()
    }

    fn ensure_relevant_update<C: ChainContext, CU: ConsensusUpdate<SYNC_COMMITTEE_SIZE>>(
        &self,
        _ctx: &C,
        update: &CU,
    ) -> Result<(), ethereum_light_client_verifier::errors::Error> {
        if self
            .state
            .is_relevant_update(update.finalized_beacon_header().slot)
        {
            Ok(())
        } else {
            Err(
                ethereum_light_client_verifier::errors::Error::IrrelevantConsensusUpdates(
                    "update is not relevant".into(),
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use ethereum_consensus::beacon::BeaconBlockHeader;
    use ethereum_consensus::config::Config;
    use ethereum_consensus::context::DefaultChainContext;
    use ethereum_consensus::fork::{altair::ALTAIR_FORK_SPEC, ForkParameter, ForkParameters};
    use ethereum_consensus::preset;
    use ethereum_consensus::sync_protocol::{SyncAggregate, SyncCommittee};
    use ethereum_consensus::types::{H256, U64};

    const SYNC_COMMITTEE_SIZE: usize = 32; // Minimal preset

    fn get_minimal_config() -> Config {
        Config {
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
        }
    }

    fn create_test_context() -> DefaultChainContext {
        DefaultChainContext::new_with_config(U64(1729846322), get_minimal_config())
    }

    struct MockTrustedSyncCommitteeInfo {
        period: SyncCommitteePeriod,
        current: PublicKey,
        next: PublicKey,
    }

    impl TrustedSyncCommitteeInfo for MockTrustedSyncCommitteeInfo {
        fn current_period<C: ChainContext>(&self, _ctx: &C) -> SyncCommitteePeriod {
            self.period
        }

        fn current_sync_committee(&self) -> PublicKey {
            self.current.clone()
        }

        fn next_sync_committee(&self) -> PublicKey {
            self.next.clone()
        }

        fn is_relevant_update(&self, _update_finalized_slot: U64) -> bool {
            true
        }
    }

    fn create_consensus_update(
        finalized_slot: u64,
        next_sync_committee: Option<SyncCommittee<SYNC_COMMITTEE_SIZE>>,
    ) -> ConsensusUpdateInfo<SYNC_COMMITTEE_SIZE> {
        ConsensusUpdateInfo {
            attested_header: BeaconBlockHeader::default(),
            next_sync_committee: next_sync_committee.map(|sc| (sc, vec![])),
            finalized_header: (
                BeaconBlockHeader {
                    slot: finalized_slot.into(),
                    ..Default::default()
                },
                vec![],
            ),
            sync_aggregate: SyncAggregate::default(),
            signature_slot: (finalized_slot + 1).into(),
            finalized_execution_root: H256::default(),
            finalized_execution_branch: vec![],
        }
    }

    #[test]
    fn test_compute_sync_committees_same_period() {
        let ctx = create_test_context();
        // Minimal preset: slots_per_epoch=8, epochs_per_sync_committee_period=8
        // So period = slot / (8 * 8) = slot / 64
        // Period 0: slots 0-63
        // Period 1: slots 64-127

        let trusted = MockTrustedSyncCommitteeInfo {
            period: U64(0),
            current: PublicKey::default(),
            next: PublicKey::default(),
        };

        // Finalized slot in same period (period 0)
        let update = create_consensus_update(32, None);

        let result = compute_sync_committees::<SYNC_COMMITTEE_SIZE, _, _>(&ctx, &trusted, update);
        assert!(result.is_ok());

        let info = result.unwrap();
        // Should keep same committees since we're in the same period
        assert_eq!(
            info.current_sync_committee,
            <MockTrustedSyncCommitteeInfo as TrustedSyncCommitteeInfo>::current_sync_committee(
                &trusted
            )
        );
        assert_eq!(
            info.next_sync_committee,
            <MockTrustedSyncCommitteeInfo as TrustedSyncCommitteeInfo>::next_sync_committee(
                &trusted
            )
        );
    }

    #[test]
    fn test_compute_sync_committees_next_period_with_committee() {
        let ctx = create_test_context();

        let trusted = MockTrustedSyncCommitteeInfo {
            period: U64(0),
            current: PublicKey::default(),
            next: PublicKey::default(),
        };

        // Create a new sync committee for the update
        let new_next_committee = SyncCommittee::<SYNC_COMMITTEE_SIZE>::default();

        // Finalized slot in next period (period 1, slot 64+)
        let update = create_consensus_update(64, Some(new_next_committee.clone()));

        let result = compute_sync_committees::<SYNC_COMMITTEE_SIZE, _, _>(&ctx, &trusted, update);
        assert!(result.is_ok());

        let info = result.unwrap();
        // Current should become the old next
        assert_eq!(
            info.current_sync_committee,
            <MockTrustedSyncCommitteeInfo as TrustedSyncCommitteeInfo>::next_sync_committee(
                &trusted
            )
        );
        // Next should come from the update
        assert_eq!(
            info.next_sync_committee,
            new_next_committee.aggregate_pubkey
        );
    }

    #[test]
    fn test_compute_sync_committees_next_period_missing_committee() {
        let ctx = create_test_context();

        let trusted = MockTrustedSyncCommitteeInfo {
            period: U64(0),
            current: PublicKey::default(),
            next: PublicKey::default(),
        };

        // Finalized slot in next period but NO next sync committee in update
        let update = create_consensus_update(64, None);

        let result = compute_sync_committees::<SYNC_COMMITTEE_SIZE, _, _>(&ctx, &trusted, update);
        assert!(result.is_err());

        match result {
            Err(Error::NoNextSyncCommitteeInConsensusUpdate {
                store_period,
                update_period,
            }) => {
                assert_eq!(store_period, U64(0));
                assert_eq!(update_period, U64(1));
            }
            _ => panic!("Expected NoNextSyncCommitteeInConsensusUpdate error"),
        }
    }

    #[test]
    fn test_compute_sync_committees_period_gap_too_large() {
        let ctx = create_test_context();

        let trusted = MockTrustedSyncCommitteeInfo {
            period: U64(0),
            current: PublicKey::default(),
            next: PublicKey::default(),
        };

        // Finalized slot in period 2 (skipping period 1)
        // Period 2: slots 128+
        let update = create_consensus_update(128, None);

        let result = compute_sync_committees::<SYNC_COMMITTEE_SIZE, _, _>(&ctx, &trusted, update);
        assert!(result.is_err());

        match result {
            Err(Error::StoreNotSupportedFinalizedPeriod {
                store_period,
                finalized_period,
            }) => {
                assert_eq!(store_period, U64(0));
                assert_eq!(finalized_period, U64(2));
            }
            _ => panic!("Expected StoreNotSupportedFinalizedPeriod error"),
        }
    }

    #[test]
    fn test_sync_committee_info_fields() {
        let current = PublicKey::default();
        let next = PublicKey::default();

        let info = SyncCommitteeInfo {
            current_sync_committee: current.clone(),
            next_sync_committee: next.clone(),
        };

        assert_eq!(info.current_sync_committee, current);
        assert_eq!(info.next_sync_committee, next);
    }
}
