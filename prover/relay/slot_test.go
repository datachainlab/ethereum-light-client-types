package relay

import (
	"testing"
)

func TestComputeEpoch(t *testing.T) {
	tests := []struct {
		name     string
		network  string
		slot     uint64
		expected uint64
	}{
		{"mainnet slot 0", Mainnet, 0, 0},
		{"mainnet slot 31", Mainnet, 31, 0},
		{"mainnet slot 32", Mainnet, 32, 1},
		{"mainnet slot 64", Mainnet, 64, 2},
		{"minimal slot 0", Minimal, 0, 0},
		{"minimal slot 7", Minimal, 7, 0},
		{"minimal slot 8", Minimal, 8, 1},
		{"minimal slot 16", Minimal, 16, 2},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ComputeEpoch(tt.network, tt.slot)
			if result != tt.expected {
				t.Errorf("ComputeEpoch(%s, %d) = %d, want %d", tt.network, tt.slot, result, tt.expected)
			}
		})
	}
}

func TestComputeSyncCommitteePeriod(t *testing.T) {
	tests := []struct {
		name     string
		network  string
		epoch    uint64
		expected uint64
	}{
		{"mainnet epoch 0", Mainnet, 0, 0},
		{"mainnet epoch 255", Mainnet, 255, 0},
		{"mainnet epoch 256", Mainnet, 256, 1},
		{"mainnet epoch 512", Mainnet, 512, 2},
		{"minimal epoch 0", Minimal, 0, 0},
		{"minimal epoch 7", Minimal, 7, 0},
		{"minimal epoch 8", Minimal, 8, 1},
		{"minimal epoch 16", Minimal, 16, 2},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ComputeSyncCommitteePeriod(tt.network, tt.epoch)
			if result != tt.expected {
				t.Errorf("ComputeSyncCommitteePeriod(%s, %d) = %d, want %d", tt.network, tt.epoch, result, tt.expected)
			}
		})
	}
}

func TestGetPeriodBoundarySlot(t *testing.T) {
	tests := []struct {
		name     string
		network  string
		period   uint64
		expected uint64
	}{
		{"mainnet period 0", Mainnet, 0, 0},
		{"mainnet period 1", Mainnet, 1, MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD * MAINNET_SLOTS_PER_EPOCH},
		{"mainnet period 2", Mainnet, 2, 2 * MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD * MAINNET_SLOTS_PER_EPOCH},
		{"minimal period 0", Minimal, 0, 0},
		{"minimal period 1", Minimal, 1, MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD * MINIMAL_SLOTS_PER_EPOCH},
		{"minimal period 2", Minimal, 2, 2 * MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD * MINIMAL_SLOTS_PER_EPOCH},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := GetPeriodBoundarySlot(tt.network, tt.period)
			if result != tt.expected {
				t.Errorf("GetPeriodBoundarySlot(%s, %d) = %d, want %d", tt.network, tt.period, result, tt.expected)
			}
		})
	}
}
