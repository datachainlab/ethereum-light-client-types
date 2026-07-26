package client

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"net/http"
	"strconv"
	"strings"
	"time"

	clienttypes "github.com/cosmos/ibc-go/v8/modules/core/02-client/types"
	"github.com/datachainlab/ethereum-light-client-types/e2e/client/pb"
	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/relay"
	lctypes "github.com/datachainlab/ethereum-light-client-types/prover/types"
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

// getClientState calls the IBC handler's getClientState(string) at the given
// block and returns the client state bytes committed at
// "clients/<clientID>/clientState".
func getClientState(ctx context.Context, client *rpc.Client, ibcAddress common.Address, clientID string, blockNumber uint64) ([]byte, error) {
	// abi-encode getClientState(string)
	id := []byte(clientID)
	idHex := fmt.Sprintf("%x", id)
	data := fmt.Sprintf("0x76c81c42%064x%064x%s%s", 0x20, len(id), idHex, strings.Repeat("0", (64-len(idHex)%64)%64))
	var out hexutil.Bytes
	if err := client.CallContext(ctx, &out, "eth_call", map[string]any{
		"to":   ibcAddress,
		"data": data,
	}, hexutil.EncodeUint64(blockNumber)); err != nil {
		return nil, fmt.Errorf("getClientState(%s) failed: %w", clientID, err)
	}
	// returns (bytes clientState, bool found)
	if len(out) < 96 {
		return nil, fmt.Errorf("unexpected getClientState result length: %d", len(out))
	}
	if new(big.Int).SetBytes(out[32:64]).Sign() == 0 {
		return nil, fmt.Errorf("client %s not found on %s", clientID, ibcAddress)
	}
	offset := new(big.Int).SetBytes(out[:32]).Uint64()
	length := new(big.Int).SetBytes(out[offset : offset+32]).Uint64()
	return out[offset+32 : offset+32+length], nil
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

// fetchForkSchedule fetches the fork epochs from the beacon node's config
// spec. A fork the node does not schedule is treated as never activating.
func fetchForkSchedule(ctx context.Context, beaconEndpoint string) (map[string]uint64, error) {
	const farFutureEpoch = ^uint64(0)

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, beaconEndpoint+"/eth/v1/config/spec", nil)
	if err != nil {
		return nil, err
	}
	res, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to get config spec: %w", err)
	}
	defer res.Body.Close()
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("config spec returned status code %d", res.StatusCode)
	}
	var spec struct {
		Data map[string]json.RawMessage `json:"data"`
	}
	if err := json.NewDecoder(res.Body).Decode(&spec); err != nil {
		return nil, fmt.Errorf("failed to decode config spec: %w", err)
	}

	epoch := func(key string) (uint64, error) {
		raw, ok := spec.Data[key+"_FORK_EPOCH"]
		if !ok {
			return farFutureEpoch, nil
		}
		var v string
		if err := json.Unmarshal(raw, &v); err != nil {
			return 0, fmt.Errorf("invalid %s_FORK_EPOCH: %w", key, err)
		}
		return strconv.ParseUint(v, 10, 64)
	}

	schedule := map[string]uint64{}
	for fork, key := range map[string]string{
		relay.Altair:    "ALTAIR",
		relay.Bellatrix: "BELLATRIX",
		relay.Capella:   "CAPELLA",
		relay.Deneb:     "DENEB",
		relay.Electra:   "ELECTRA",
		relay.Fulu:      "FULU",
	} {
		e, err := epoch(key)
		if err != nil {
			return nil, err
		}
		schedule[fork] = e
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
	ibcClientID string,
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

	// Optionally prove membership of the client state commitment at the same block.
	var membershipPath string
	var membershipValue, membershipProof []byte
	if ibcClientID != "" {
		membershipPath = fmt.Sprintf("clients/%s/clientState", ibcClientID)
		membershipValue, err = getClientState(ctx, executionClient, ibcAddress, ibcClientID, executionUpdate.BlockNumber)
		if err != nil {
			return nil, err
		}
		membershipProof, err = relay.BuildStateProof(ctx, proofClient{executionClient}, ibcAddress, []byte(membershipPath), int64(executionUpdate.BlockNumber))
		if err != nil {
			return nil, fmt.Errorf("failed to build state proof: %w", err)
		}
	}

	// The fork schedule is only consulted for the minimal preset.
	var schedule map[string]uint64
	if network == relay.Minimal {
		schedule, err = fetchForkSchedule(ctx, beaconEndpoint)
		if err != nil {
			return nil, err
		}
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

		// trusted_height.revision_height carries the trusted beacon slot
		TrustedSyncCommittee: &lctypes.TrustedSyncCommittee{
			TrustedHeight: &clienttypes.Height{RevisionNumber: 0, RevisionHeight: trustedSlot},
			SyncCommittee: bootstrapCommittee,
			IsNext:        false,
		},
		TrustedCurrentSyncCommittee: bootstrapCommittee.AggregatePubkey,
		TrustedNextSyncCommittee:    lcUpdate.Data.NextSyncCommittee.ToProto().AggregatePubkey,
		TrustedTimestampSecs:        trustedTimestamp,

		ConsensusUpdate:     consensusUpdate,
		ExecutionUpdate:     executionUpdate,
		AccountUpdate:       accountUpdate,
		HeaderTimestampSecs: headerTimestamp,

		NowSecs: uint64(time.Now().Unix()),

		IbcCommitmentsSlot: relay.IBCCommitmentsSlot(),
		MembershipPath:     membershipPath,
		MembershipValue:    membershipValue,
		MembershipProof:    membershipProof,
	}, nil
}
