package relay

import (
	"context"
	"fmt"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/execution"
)

// GetSlotAtTimestamp computes slot from timestamp
func GetSlotAtTimestamp(ctx context.Context, beaconClient beacon.Client, network string, timestamp uint64) (uint64, error) {
	genesis, err := beaconClient.GetGenesis(ctx)
	if err != nil {
		return 0, err
	}
	secondsPerSlot := SecondsPerSlot(network)
	if timestamp < genesis.GenesisTimeSeconds {
		return 0, fmt.Errorf("timestamp is smaller than genesisTime: timestamp=%v genesisTime=%v", timestamp, genesis.GenesisTimeSeconds)
	} else if (timestamp-genesis.GenesisTimeSeconds)%secondsPerSlot != 0 {
		return 0, fmt.Errorf("timestamp is not multiple of secondsPerSlot: timestamp=%v secondsPerSlot=%v genesisTime=%v", timestamp, secondsPerSlot, genesis.GenesisTimeSeconds)
	}
	slotsSinceGenesis := (timestamp - genesis.GenesisTimeSeconds) / secondsPerSlot
	return GENESIS_SLOT + slotsSinceGenesis, nil
}

// maxSkippedSlotsLookahead bounds the search for the next slot that has a block.
// A gap this long means the chain is not finalizing, in which case there is nothing
// sensible to return anyway.
const maxSkippedSlotsLookahead = 64

// GetConsensusStateSlotWithBlockNumber returns the beacon slot that a consensus state
// created for `blockNumber` records, i.e. the slot of the finalized header whose light
// client header references that execution block.
//
//   - pre-Gloas: the finalized beacon block carries the execution payload of its own slot,
//     so the slot derived from the execution block timestamp is already the answer.
//   - Gloas: the light client header exposes
//     `signed_execution_payload_bid.message.parent_block_hash`, so the execution block it
//     references was produced one slot earlier. The recorded slot is therefore the next
//     slot that actually has a block, which is not necessarily `slot+1` because slots may
//     be skipped.
//
// Deriving the period from the execution block's own slot instead would be one period off
// whenever the finalized slot is the first slot of a period, which makes the prover send
// the previous period's sync committee.
func GetConsensusStateSlotWithBlockNumber(ctx context.Context, beaconClient beacon.Client, executionClient execution.Client, network string, minimalForkSchedule map[string]uint64, blockNumber uint64) (uint64, error) {
	timestamp, err := execution.GetBlockTimestamp(ctx, executionClient, blockNumber)
	if err != nil {
		return 0, err
	}
	slot, err := GetSlotAtTimestamp(ctx, beaconClient, network, timestamp)
	if err != nil {
		return 0, err
	}
	if !IsGloasSlot(network, minimalForkSchedule, slot) {
		return slot, nil
	}
	for next := slot + 1; next <= slot+maxSkippedSlotsLookahead; next++ {
		if _, err := beaconClient.GetBlockRoot(ctx, next, true); err == nil {
			return next, nil
		}
	}
	return 0, fmt.Errorf("no block found after slot %v within %v slots: block_number=%v", slot, maxSkippedSlotsLookahead, blockNumber)
}

// GetPeriodWithBlockNumber returns sync committee period for a block number
func GetPeriodWithBlockNumber(ctx context.Context, beaconClient beacon.Client, executionClient execution.Client, network string, minimalForkSchedule map[string]uint64, blockNumber uint64) (uint64, error) {
	slot, err := GetConsensusStateSlotWithBlockNumber(ctx, beaconClient, executionClient, network, minimalForkSchedule, blockNumber)
	if err != nil {
		return 0, err
	}
	return ComputeSyncCommitteePeriod(network, ComputeEpoch(network, slot)), nil
}

// IsGloasSlot reports whether `slot` is at or after the Gloas fork.
//
// Gloas is identified by `execution_block_hash_gindex` being set, which is the same
// discriminator the Rust verifier uses (`ForkSpec::is_gloas`).
func IsGloasSlot(network string, minimalForkSchedule map[string]uint64, slot uint64) bool {
	epoch := ComputeEpoch(network, slot)
	params := GetForkParameters(network, minimalForkSchedule)
	for i := len(params.Forks) - 1; i >= 0; i-- {
		fork := params.Forks[i]
		if epoch >= fork.Epoch {
			return fork.Spec != nil && fork.Spec.ExecutionBlockHashGindex != 0
		}
	}
	return false
}

// ComputeSyncCommitteePeriod computes sync committee period from epoch
func ComputeSyncCommitteePeriod(network string, epoch uint64) uint64 {
	return epoch / EpochsPerSyncCommitteePeriod(network)
}

// ComputeEpoch computes epoch from slot
func ComputeEpoch(network string, slot uint64) uint64 {
	return slot / SlotsPerEpoch(network)
}

// GetPeriodBoundarySlot returns the first slot of the period
func GetPeriodBoundarySlot(network string, period uint64) uint64 {
	return period * EpochsPerSyncCommitteePeriod(network) * SlotsPerEpoch(network)
}
