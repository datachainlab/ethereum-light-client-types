package relay

import (
	"testing"
)

func TestIsMainnetPreset(t *testing.T) {
	tests := []struct {
		name     string
		network  string
		expected bool
	}{
		{"mainnet", Mainnet, true},
		{"sepolia", Sepolia, true},
		{"minimal", Minimal, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := IsMainnetPreset(tt.network)
			if result != tt.expected {
				t.Errorf("IsMainnetPreset(%s) = %v, want %v", tt.network, result, tt.expected)
			}
		})
	}
}

func TestSecondsPerSlot(t *testing.T) {
	tests := []struct {
		name     string
		network  string
		expected uint64
	}{
		{"mainnet", Mainnet, MAINNET_SECONDS_PER_SLOT},
		{"sepolia", Sepolia, MAINNET_SECONDS_PER_SLOT},
		{"minimal", Minimal, MINIMAL_SECONDS_PER_SLOT},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SecondsPerSlot(tt.network)
			if result != tt.expected {
				t.Errorf("SecondsPerSlot(%s) = %v, want %v", tt.network, result, tt.expected)
			}
		})
	}
}

func TestSlotsPerEpoch(t *testing.T) {
	tests := []struct {
		name     string
		network  string
		expected uint64
	}{
		{"mainnet", Mainnet, MAINNET_SLOTS_PER_EPOCH},
		{"sepolia", Sepolia, MAINNET_SLOTS_PER_EPOCH},
		{"minimal", Minimal, MINIMAL_SLOTS_PER_EPOCH},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SlotsPerEpoch(tt.network)
			if result != tt.expected {
				t.Errorf("SlotsPerEpoch(%s) = %v, want %v", tt.network, result, tt.expected)
			}
		})
	}
}

func TestEpochsPerSyncCommitteePeriod(t *testing.T) {
	tests := []struct {
		name     string
		network  string
		expected uint64
	}{
		{"mainnet", Mainnet, MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD},
		{"sepolia", Sepolia, MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD},
		{"minimal", Minimal, MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := EpochsPerSyncCommitteePeriod(tt.network)
			if result != tt.expected {
				t.Errorf("EpochsPerSyncCommitteePeriod(%s) = %v, want %v", tt.network, result, tt.expected)
			}
		})
	}
}

func TestGetForkParameters(t *testing.T) {
	tests := []struct {
		name            string
		network         string
		minimalSchedule map[string]uint64
		expectForkCount int
	}{
		{
			name:            "mainnet",
			network:         Mainnet,
			minimalSchedule: nil,
			expectForkCount: 7, // Altair, Bellatrix, Capella, Deneb, Electra, Fulu, Gloas
		},
		{
			name:            "sepolia",
			network:         Sepolia,
			minimalSchedule: nil,
			expectForkCount: 7,
		},
		{
			name:    "minimal",
			network: Minimal,
			minimalSchedule: map[string]uint64{
				Altair:    0,
				Bellatrix: 0,
				Capella:   0,
				Deneb:     0,
				Electra:   0,
				Fulu:      0,
				Gloas:     0,
			},
			expectForkCount: 7,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := GetForkParameters(tt.network, tt.minimalSchedule)
			if result == nil {
				t.Fatalf("GetForkParameters(%s) returned nil", tt.network)
			}
			if len(result.Forks) != tt.expectForkCount {
				t.Errorf("GetForkParameters(%s) fork count = %d, want %d", tt.network, len(result.Forks), tt.expectForkCount)
			}
			if len(result.GenesisForkVersion) != 4 {
				t.Errorf("GenesisForkVersion length = %d, want 4", len(result.GenesisForkVersion))
			}
		})
	}
}

func TestForkSpecs(t *testing.T) {
	// Test that Gloas spec has ExecutionBlockHashGindex set
	if GloasSpec.ExecutionBlockHashGindex == 0 {
		t.Error("GloasSpec.ExecutionBlockHashGindex should not be 0")
	}
	if GloasSpec.ExecutionPayloadGindex != 0 {
		t.Error("GloasSpec.ExecutionPayloadGindex should be 0")
	}

	// Test that pre-Gloas specs have ExecutionPayloadGindex set
	if DenebSpec.ExecutionPayloadGindex == 0 {
		t.Error("DenebSpec.ExecutionPayloadGindex should not be 0")
	}
	if DenebSpec.ExecutionBlockHashGindex != 0 {
		t.Error("DenebSpec.ExecutionBlockHashGindex should be 0")
	}
}
