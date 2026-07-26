package client

import (
	"context"
	"errors"
	"fmt"
	"math/big"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/datachainlab/ethereum-light-client-types/e2e/client/pb"
	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/relay"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/ethereum/go-ethereum/rlp"
	"github.com/ethereum/go-ethereum/rpc"
)

const (
	// A mainnet sync committee period is ~27.3 hours, so the trusting period
	// must comfortably exceed it: the trusted state is the period boundary.
	defaultTrustingPeriodSecs = 7 * 86400
	defaultMaxClockDriftSecs  = 60
)

// ErrFinalityNotAdvanced is returned right after a sync committee period
// boundary, while the finalized slot still equals the boundary slot.
// Callers can retry after more slots are finalized.
var ErrFinalityNotAdvanced = errors.New("finalized slot is not newer than the period boundary slot")

// proofClient implements relay.ProofClient on top of a raw execution RPC client.
type proofClient struct {
	client *rpc.Client
}

func (p proofClient) GetProof(ctx context.Context, address common.Address, storageKeys [][]byte, blockNumber *big.Int) (*relay.StateProof, error) {
	keys := make([]string, 0, len(storageKeys))
	for _, k := range storageKeys {
		keys = append(keys, string(k))
	}
	var out struct {
		StorageHash  common.Hash `json:"storageHash"`
		AccountProof []string    `json:"accountProof"`
		StorageProof []struct {
			Proof []string `json:"proof"`
		} `json:"storageProof"`
	}
	if err := p.client.CallContext(ctx, &out, "eth_getProof", address, keys, hexutil.EncodeBig(blockNumber)); err != nil {
		return nil, fmt.Errorf("eth_getProof failed: %w", err)
	}
	accountProofRLP, err := encodeRLPProof(out.AccountProof)
	if err != nil {
		return nil, err
	}
	proof := &relay.StateProof{
		StorageHash:     out.StorageHash,
		AccountProofRLP: accountProofRLP,
	}
	for _, sp := range out.StorageProof {
		bz, err := encodeRLPProof(sp.Proof)
		if err != nil {
			return nil, err
		}
		proof.StorageProofRLP = append(proof.StorageProofRLP, bz)
	}
	return proof, nil
}

// encodeRLPProof converts hex-encoded proof nodes into a single RLP-encoded
// list of nodes, the format expected by the light client verifier.
func encodeRLPProof(proof []string) ([]byte, error) {
	var target [][][]byte
	for _, p := range proof {
		bz, err := hexutil.Decode(p)
		if err != nil {
			return nil, err
		}
		var val [][]byte
		if err := rlp.DecodeBytes(bz, &val); err != nil {
			return nil, err
		}
		target = append(target, val)
	}
	return rlp.EncodeToBytes(target)
}

// detectNetwork determines the network from the beacon node's genesis fork
// version. Anything other than mainnet/sepolia is treated as a minimal-preset
// devnet.
func detectNetwork(genesisForkVersion [4]byte) string {
	switch genesisForkVersion {
	case [4]byte{0x00, 0x00, 0x00, 0x00}:
		return relay.Mainnet
	case [4]byte{0x90, 0x00, 0x00, 0x69}:
		return relay.Sepolia
	default:
		return relay.Minimal
	}
}

// forkScheduleFromEnv parses FORK_SCHEDULE ("altair=0,bellatrix=0,...").
// All forks default to epoch 0, which matches a typical local devnet.
func forkScheduleFromEnv() (map[string]uint64, error) {
	schedule := map[string]uint64{
		relay.Altair:    0,
		relay.Bellatrix: 0,
		relay.Capella:   0,
		relay.Deneb:     0,
		relay.Electra:   0,
		relay.Fulu:      0,
	}
	env := os.Getenv("FORK_SCHEDULE")
	if env == "" {
		return schedule, nil
	}
	for _, kv := range strings.Split(env, ",") {
		parts := strings.SplitN(kv, "=", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("invalid FORK_SCHEDULE entry: %q", kv)
		}
		epoch, err := strconv.ParseUint(parts[1], 10, 64)
		if err != nil {
			return nil, fmt.Errorf("invalid FORK_SCHEDULE epoch in %q: %w", kv, err)
		}
		schedule[parts[0]] = epoch
	}
	return schedule, nil
}

// BuildVerifyUpdateRequest builds a VerifyUpdateRequest from live beacon and
// execution endpoints, mirroring how the relay prover assembles an update
// header: the trusted state is the sync committee bootstrap at the current
// period boundary, and the update is the latest light client update of the
// same period.
func BuildVerifyUpdateRequest(
	ctx context.Context,
	beaconEndpoint, executionEndpoint string,
	ibcAddress common.Address,
) (*pb.VerifyUpdateRequest, error) {
	beaconClient := beacon.NewClient(beaconEndpoint)

	genesis, err := beaconClient.GetGenesis(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get genesis: %w", err)
	}
	network := detectNetwork(genesis.GenesisForkVersion)

	finalityUpdate, err := beaconClient.GetLightClientFinalityUpdate(ctx)
	if err != nil {
		return nil, fmt.Errorf("failed to get finality update: %w", err)
	}
	finalizedSlot := uint64(finalityUpdate.Data.FinalizedHeader.Beacon.Slot)
	period := relay.ComputeSyncCommitteePeriod(network, relay.ComputeEpoch(network, finalizedSlot))

	// The consensus update to verify is the latest finality update; the
	// execution update and timestamp are derived from its finalized header.
	consensusUpdate := finalityUpdate.Data.ToProto()
	executionUpdate, headerTimestamp, err := relay.BuildExecutionUpdateFromFinalizedHeader(&finalityUpdate.Data.FinalizedHeader, true)
	if err != nil {
		return nil, fmt.Errorf("failed to build execution update: %w", err)
	}

	// The period's light client update snapshot provides the next sync
	// committee of the trusted period.
	lcUpdate, err := beaconClient.GetLightClientUpdate(ctx, period)
	if err != nil {
		return nil, fmt.Errorf("failed to get light client update for period %d: %w", period, err)
	}

	// Trusted state: the bootstrap sync committee at the period boundary.
	bootstrapCommittee, err := relay.GetBootstrapInPeriod(ctx, beaconClient, network, period)
	if err != nil {
		return nil, fmt.Errorf("failed to get bootstrap for period %d: %w", period, err)
	}
	trustedSlot := relay.GetPeriodBoundarySlot(network, period)
	if finalizedSlot <= trustedSlot {
		return nil, fmt.Errorf("%w: finalized=%d boundary=%d", ErrFinalityNotAdvanced, finalizedSlot, trustedSlot)
	}
	trustedTimestamp := genesis.GenesisTimeSeconds + trustedSlot*relay.SecondsPerSlot(network)

	// Account update at the finalized execution block.
	executionClient, err := rpc.DialContext(ctx, executionEndpoint)
	if err != nil {
		return nil, fmt.Errorf("failed to dial execution endpoint: %w", err)
	}
	defer executionClient.Close()
	accountUpdate, err := relay.BuildAccountUpdate(ctx, proofClient{executionClient}, ibcAddress, executionUpdate.BlockNumber)
	if err != nil {
		return nil, fmt.Errorf("failed to build account update: %w", err)
	}

	schedule, err := forkScheduleFromEnv()
	if err != nil {
		return nil, err
	}
	forkParameters := relay.GetForkParameters(network, schedule)

	syncCommitteeSize := uint32(32)
	if relay.IsMainnetPreset(network) {
		syncCommitteeSize = 512
	}

	return &pb.VerifyUpdateRequest{
		SyncCommitteeSize:            syncCommitteeSize,
		GenesisValidatorsRoot:        genesis.GenesisValidatorsRoot[:],
		MinSyncCommitteeParticipants: 1,
		GenesisTime:                  genesis.GenesisTimeSeconds,
		ForkParameters:               forkParameters,
		SecondsPerSlot:               relay.SecondsPerSlot(network),
		SlotsPerEpoch:                relay.SlotsPerEpoch(network),
		EpochsPerSyncCommitteePeriod: relay.EpochsPerSyncCommitteePeriod(network),
		TrustLevelNumerator:          2,
		TrustLevelDenominator:        3,
		TrustingPeriodSecs:           defaultTrustingPeriodSecs,
		MaxClockDriftSecs:            defaultMaxClockDriftSecs,
		IbcAddress:                   ibcAddress.Bytes(),

		TrustedSlot:                 trustedSlot,
		TrustedCurrentSyncCommittee: bootstrapCommittee.AggregatePubkey,
		TrustedNextSyncCommittee:    lcUpdate.Data.NextSyncCommittee.ToProto().AggregatePubkey,
		TrustedTimestampSecs:        trustedTimestamp,

		SyncCommittee:       bootstrapCommittee,
		IsNext:              false,
		ConsensusUpdate:     consensusUpdate,
		ExecutionUpdate:     executionUpdate,
		AccountUpdate:       accountUpdate,
		HeaderTimestampSecs: headerTimestamp,

		NowSecs: uint64(time.Now().Unix()),
	}, nil
}
