package relay

import (
	"context"
	"errors"
	"fmt"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	lctypes "github.com/datachainlab/ethereum-light-client-types/prover/types"
)

// GetBootstrapInPeriod retrieves the current sync committee from a bootstrap within the given period
func GetBootstrapInPeriod(ctx context.Context, beaconClient beacon.Client, network string, period uint64) (*lctypes.SyncCommittee, error) {
	slotsPerEpoch := SlotsPerEpoch(network)
	startSlot := GetPeriodBoundarySlot(network, period)
	lastSlotInPeriod := GetPeriodBoundarySlot(network, period+1) - 1

	var errs []error
	for i := startSlot + slotsPerEpoch; i <= lastSlotInPeriod; i += slotsPerEpoch {
		res, err := beaconClient.GetBlockRoot(ctx, i, false)
		if err != nil {
			errs = append(errs, err)
			return nil, fmt.Errorf("there is no available bootstrap in period: period=%v err=%v", period, errors.Join(errs...))
		}
		bootstrap, err := beaconClient.GetBootstrap(ctx, res.Data.Root[:])
		if err != nil {
			errs = append(errs, err)
			continue
		}
		return bootstrap.Data.CurrentSyncCommittee.ToProto(), nil
	}
	return nil, fmt.Errorf("failed to get bootstrap in period: period=%v err=%v", period, errors.Join(errs...))
}
