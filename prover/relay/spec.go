package relay

import (
	"fmt"

	"github.com/datachainlab/ethereum-light-client-types/prover/types"
)

const (
	Mainnet = "mainnet"
	Minimal = "minimal"
	Sepolia = "sepolia"
	// MinimalEthpandaops is the minimal preset as produced by
	// ethpandaops/ethereum-package (kurtosis devnets). It is identical to Minimal
	// except for the fork versions, which the package hardcodes to its own scheme.
	MinimalEthpandaops = "minimal-ethpandaops"
)

const (
	MAINNET_PRESET_SYNC_COMMITTEE_SIZE = 512
	MINIMAL_PRESET_SYNC_COMMITTEE_SIZE = 32
)

const (
	Altair    = "altair"
	Bellatrix = "bellatrix"
	Capella   = "capella"
	Deneb     = "deneb"
	Electra   = "electra"
	Fulu      = "fulu"
	Gloas     = "gloas"
)

var (
	AltairSpec = types.ForkSpec{
		FinalizedRootGindex:        105,
		CurrentSyncCommitteeGindex: 54,
		NextSyncCommitteeGindex:    55,
	}
	BellatrixSpec = types.ForkSpec{
		FinalizedRootGindex:               AltairSpec.FinalizedRootGindex,
		CurrentSyncCommitteeGindex:        AltairSpec.CurrentSyncCommitteeGindex,
		NextSyncCommitteeGindex:           AltairSpec.NextSyncCommitteeGindex,
		ExecutionPayloadGindex:            25,
		ExecutionPayloadStateRootGindex:   18,
		ExecutionPayloadBlockNumberGindex: 22,
	}
	CapellaSpec = BellatrixSpec
	DenebSpec   = types.ForkSpec{
		FinalizedRootGindex:               CapellaSpec.FinalizedRootGindex,
		CurrentSyncCommitteeGindex:        CapellaSpec.CurrentSyncCommitteeGindex,
		NextSyncCommitteeGindex:           CapellaSpec.NextSyncCommitteeGindex,
		ExecutionPayloadGindex:            CapellaSpec.ExecutionPayloadGindex,
		ExecutionPayloadStateRootGindex:   34,
		ExecutionPayloadBlockNumberGindex: 38,
	}
	ElectraSpec = types.ForkSpec{
		FinalizedRootGindex:               169,
		CurrentSyncCommitteeGindex:        86,
		NextSyncCommitteeGindex:           87,
		ExecutionPayloadGindex:            DenebSpec.ExecutionPayloadGindex,
		ExecutionPayloadStateRootGindex:   DenebSpec.ExecutionPayloadStateRootGindex,
		ExecutionPayloadBlockNumberGindex: DenebSpec.ExecutionPayloadBlockNumberGindex,
	}
	FuluSpec = ElectraSpec
	// GloasSpec: Uses execution_block_hash instead of ExecutionPayloadHeader.
	// EIP-7688 turns BeaconState into a progressive container, so the state gindices
	// are not inherited from Electra; EIP-7732 moves the execution commitment into
	// signed_execution_payload_bid, which gives a new block body gindex.
	// ref: https://github.com/ethereum/consensus-specs/blob/master/specs/gloas/light-client/sync-protocol.md#new-constants
	GloasSpec = types.ForkSpec{
		FinalizedRootGindex:        735,  // FINALIZED_ROOT_GINDEX_GLOAS
		CurrentSyncCommitteeGindex: 2945, // CURRENT_SYNC_COMMITTEE_GINDEX_GLOAS
		NextSyncCommitteeGindex:    2946, // NEXT_SYNC_COMMITTEE_GINDEX_GLOAS
		ExecutionPayloadGindex:     0,    // Not used in Gloas
		// Not used in Gloas (RLP verification instead of SSZ merkle proofs)
		ExecutionPayloadStateRootGindex:   0,
		ExecutionPayloadBlockNumberGindex: 0,
		ExecutionBlockHashGindex:          2856, // EXECUTION_BLOCK_HASH_GINDEX_GLOAS
	}
)

const (
	GENESIS_SLOT = 0
)

// merkle tree's leaf index
const (
	EXECUTION_STATE_ROOT_LEAF_INDEX   = 2
	EXECUTION_BLOCK_NUMBER_LEAF_INDEX = 6
	EXECUTION_BLOCK_HASH_LEAF_INDEX   = 12
)

// minimal preset
const (
	MINIMAL_SECONDS_PER_SLOT                 uint64 = 6
	MINIMAL_SLOTS_PER_EPOCH                  uint64 = 8
	MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD uint64 = 8
)

// mainnet preset
const (
	MAINNET_SECONDS_PER_SLOT                 uint64 = 12
	MAINNET_SLOTS_PER_EPOCH                  uint64 = 32
	MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD uint64 = 256
)

func IsMainnetPreset(network string) bool {
	switch network {
	case Mainnet, Sepolia:
		return true
	case Minimal, MinimalEthpandaops:
		return false
	default:
		panic(fmt.Sprintf("unknown network: %v", network))
	}
}

func SecondsPerSlot(network string) uint64 {
	if IsMainnetPreset(network) {
		return MAINNET_SECONDS_PER_SLOT
	} else {
		return MINIMAL_SECONDS_PER_SLOT
	}
}

func SlotsPerEpoch(network string) uint64 {
	if IsMainnetPreset(network) {
		return MAINNET_SLOTS_PER_EPOCH
	} else {
		return MINIMAL_SLOTS_PER_EPOCH
	}
}

func EpochsPerSyncCommitteePeriod(network string) uint64 {
	if IsMainnetPreset(network) {
		return MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD
	} else {
		return MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD
	}
}

// convertToEthpandaopsForkVersions converts standard minimal fork versions to the
// ethpandaops/ethereum-package scheme.
// Ethpandaops reserves 0x10 for the genesis version and bumps each subsequent fork by 0x10,
// so the standard-minimal fork ordinal n (Altair=1 .. Gloas=7) maps to (n+1)*0x10.
// Standard minimal: GenesisForkVersion={0,0,0,1}, Fork.Version={n,0,0,1}
// Ethpandaops:      GenesisForkVersion={0x10,0,0,0x38}, Fork.Version={(n+1)*0x10,0,0,0x38}
// (genesis=0x10, Altair=0x20, Bellatrix=0x30, Capella=0x40, Deneb=0x50, Electra=0x60,
// Fulu=0x70, Gloas=0x80)
// ref: https://github.com/ethpandaops/ethereum-package/blob/main/src/package_io/constants.star
func convertToEthpandaopsForkVersions(params *types.ForkParameters) *types.ForkParameters {
	newGenesis := make([]byte, 4)
	copy(newGenesis, params.GenesisForkVersion)
	newGenesis[0] = 0x10
	newGenesis[3] = 0x38

	newForks := make([]*types.Fork, len(params.Forks))
	for i, fork := range params.Forks {
		newVersion := make([]byte, 4)
		copy(newVersion, fork.Version)
		newVersion[0] = (fork.Version[0] + 1) * 0x10
		newVersion[3] = 0x38

		newForks[i] = &types.Fork{
			Version: newVersion,
			Epoch:   fork.Epoch,
			Spec:    fork.Spec,
		}
	}

	return &types.ForkParameters{
		GenesisForkVersion: newGenesis,
		Forks:              newForks,
	}
}

func GetForkParameters(network string, minimalForkSchedule map[string]uint64) *types.ForkParameters {
	switch network {
	case Minimal, MinimalEthpandaops:
		minimalParams := &types.ForkParameters{
			GenesisForkVersion: []byte{0, 0, 0, 1},
			Forks: []*types.Fork{
				{
					Version: []byte{1, 0, 0, 1},
					Epoch:   minimalForkSchedule[Altair],
					Spec:    &AltairSpec,
				},
				{
					Version: []byte{2, 0, 0, 1},
					Epoch:   minimalForkSchedule[Bellatrix],
					Spec:    &BellatrixSpec,
				},
				{
					Version: []byte{3, 0, 0, 1},
					Epoch:   minimalForkSchedule[Capella],
					Spec:    &CapellaSpec,
				},
				{
					Version: []byte{4, 0, 0, 1},
					Epoch:   minimalForkSchedule[Deneb],
					Spec:    &DenebSpec,
				},
				{
					Version: []byte{5, 0, 0, 1},
					Epoch:   minimalForkSchedule[Electra],
					Spec:    &ElectraSpec,
				},
				{
					Version: []byte{6, 0, 0, 1},
					Epoch:   minimalForkSchedule[Fulu],
					Spec:    &FuluSpec,
				},
				{
					Version: []byte{7, 0, 0, 1},
					Epoch:   minimalForkSchedule[Gloas],
					Spec:    &GloasSpec,
				},
			},
		}
		if network == MinimalEthpandaops {
			return convertToEthpandaopsForkVersions(minimalParams)
		}
		return minimalParams
	case Mainnet:
		return &types.ForkParameters{
			GenesisForkVersion: []byte{0, 0, 0, 0},
			Forks: []*types.Fork{
				{
					Version: []byte{1, 0, 0, 0},
					Epoch:   74240,
					Spec:    &AltairSpec,
				},
				{
					Version: []byte{2, 0, 0, 0},
					Epoch:   144896,
					Spec:    &BellatrixSpec,
				},
				{
					Version: []byte{3, 0, 0, 0},
					Epoch:   194048,
					Spec:    &CapellaSpec,
				},
				{
					Version: []byte{4, 0, 0, 0},
					Epoch:   269568,
					Spec:    &DenebSpec,
				},
				{
					Version: []byte{5, 0, 0, 0},
					Epoch:   364032,
					Spec:    &ElectraSpec,
				},
				// ref: https://github.com/ethereum/consensus-specs/blob/v1.6.0/configs/mainnet.yaml#L55-L57
				{
					Version: []byte{6, 0, 0, 0},
					Epoch:   411392,
					Spec:    &FuluSpec,
				},
				// Gloas fork epoch is TBD
				{
					Version: []byte{7, 0, 0, 0},
					Epoch:   18446744073709551615, // TBD: set to max uint64 until confirmed
					Spec:    &GloasSpec,
				},
			},
		}
	case Sepolia:
		return &types.ForkParameters{
			GenesisForkVersion: []byte{144, 0, 0, 105},
			Forks: []*types.Fork{
				{
					Version: []byte{144, 0, 0, 112},
					Epoch:   50,
					Spec:    &AltairSpec,
				},
				{
					Version: []byte{144, 0, 0, 113},
					Epoch:   100,
					Spec:    &BellatrixSpec,
				},
				{
					Version: []byte{144, 0, 0, 114},
					Epoch:   56832,
					Spec:    &CapellaSpec,
				},
				{
					Version: []byte{144, 0, 0, 115},
					Epoch:   132608,
					Spec:    &DenebSpec,
				},
				{
					Version: []byte{144, 0, 0, 116},
					Epoch:   222464,
					Spec:    &ElectraSpec,
				},
				// The metadata of Fulu Sepolia is from https://github.com/eth-clients/sepolia/blob/f9158732adb1a2a6440613ad2232eb50e7384c4f/metadata/config.yaml#L43-L45
				{
					Version: []byte{144, 0, 0, 117},
					Epoch:   272640,
					Spec:    &FuluSpec,
				},
				// Gloas Sepolia fork epoch is TBD
				{
					Version: []byte{144, 0, 0, 118},
					Epoch:   18446744073709551615, // TBD: set to max uint64 until confirmed
					Spec:    &GloasSpec,
				},
			},
		}
	default:
		panic(fmt.Sprintf("unknown network: %v", network))
	}
}
