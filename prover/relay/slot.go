package relay

import (
	"bytes"
	"context"
	"fmt"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/execution"
	"github.com/datachainlab/ethereum-light-client-types/prover/types"
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

const maxSkippedSlotsLookahead = 64

// GetConsensusStateSlotWithBlockNumber returns the slot of the finalized header whose light
// client header references the execution block `blockNumber`.
//
//   - pre-Gloas: the beacon block carries the execution payload of its own slot, so the slot
//     derived from the execution block timestamp is the answer.
//   - Gloas: the light client header exposes
//     `signed_execution_payload_bid.message.parent_block_hash`, so the referencing slot is a
//     later one. It is found by matching the bid, not by taking the next slot that has a
//     block, since slots may be skipped.
func GetConsensusStateSlotWithBlockNumber(ctx context.Context, beaconClient beacon.Client, executionClient execution.RPCClient, network string, forkParameters *types.ForkParameters, blockNumber uint64) (uint64, error) {
	block, err := execution.GetBlockHeaderFields(ctx, executionClient, blockNumber)
	if err != nil {
		return 0, err
	}
	slot, err := GetSlotAtTimestamp(ctx, beaconClient, network, block.Timestamp)
	if err != nil {
		return 0, err
	}
	if !forkParameters.IsGloas(ComputeEpoch(network, slot)) {
		return slot, nil
	}
	for next := slot + 1; next <= slot+maxSkippedSlotsLookahead; next++ {
		// Any failure is treated as "not this slot": a missing block, and also a
		// transport error, which may skip past the slot we are looking for.
		parentBlockHash, err := beaconClient.GetExecutionPayloadBidParentBlockHash(ctx, next)
		if err == nil && bytes.Equal(parentBlockHash, block.Hash.Bytes()) {
			return next, nil
		}
	}
	return 0, fmt.Errorf("no beacon block bidding on execution block %v (%v) found within %v slots after slot %v", blockNumber, block.Hash, maxSkippedSlotsLookahead, slot)
}

// GetPeriodWithBlockNumber returns sync committee period for a block number
func GetPeriodWithBlockNumber(ctx context.Context, beaconClient beacon.Client, executionClient execution.RPCClient, network string, forkParameters *types.ForkParameters, blockNumber uint64) (uint64, error) {
	slot, err := GetConsensusStateSlotWithBlockNumber(ctx, beaconClient, executionClient, network, forkParameters, blockNumber)
	if err != nil {
		return 0, err
	}
	return ComputeSyncCommitteePeriod(network, ComputeEpoch(network, slot)), nil
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
