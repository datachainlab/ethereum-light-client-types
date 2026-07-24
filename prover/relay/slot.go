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

// GetPeriodWithBlockNumber returns sync committee period for a block number
func GetPeriodWithBlockNumber(ctx context.Context, beaconClient beacon.Client, executionClient execution.Client, network string, blockNumber uint64) (uint64, error) {
	timestamp, err := execution.GetBlockTimestamp(ctx, executionClient, blockNumber)
	if err != nil {
		return 0, err
	}
	slot, err := GetSlotAtTimestamp(ctx, beaconClient, network, timestamp)
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
