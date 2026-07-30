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
	// must exceed a mainnet sync committee period (~27.3 hours)
	defaultTrustingPeriodSecs = 7 * 86400
	defaultMaxClockDriftSecs  = 60
)

// ErrFinalityNotAdvanced is returned right after a period boundary; retry
// after more slots are finalized.
var ErrFinalityNotAdvanced = errors.New("finalized slot is not newer than the period boundary slot")

// ErrArchiveStateUnavailable is returned when the execution node cannot serve
// proofs for the requested (older) block.
var ErrArchiveStateUnavailable = errors.New("archive state unavailable")

func wrapProofErr(err error) error {
	if err == nil {
		return nil
	}
	msg := strings.ToLower(err.Error())
	if strings.Contains(msg, "archive") || strings.Contains(msg, "missing trie node") {
		return fmt.Errorf("%w: %v", ErrArchiveStateUnavailable, err)
	}
	return err
}

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

// getClientState returns the client state bytes committed at
// "clients/<clientID>/clientState" via the IBC handler's getClientState(string).
func getClientState(ctx context.Context, client *rpc.Client, ibcAddress common.Address, clientID string, blockNumber uint64) ([]byte, error) {
	// abi-encoded call: 0x76c81c42 = keccak256("getClientState(string)")[:4],
	// then per the ABI spec for a single dynamic argument: the offset to the
	// string data (32), its length, and the bytes right-padded to a 32-byte word
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
	// decode the (bytes clientState, bool found) return value
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

// encodeRLPProof re-encodes hex proof nodes into the RLP list format expected
// by the verifier.
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

// detectNetwork determines the network from the genesis fork version;
// anything other than mainnet/sepolia is treated as minimal.
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

// fetchForkSchedule fetches fork epochs from the beacon node's config spec;
// an unscheduled fork is treated as never activating.
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

// BuildVerifyUpdateRequest builds a request from live endpoints: the trusted
// state is the sync committee bootstrap at the period boundary, and the update
// to verify is the latest finality update.
//
// With isNext, the trusted state is placed in the previous period and the
// period's update snapshot is verified with the trusted next sync committee
// (a sync committee period transition).
func BuildVerifyUpdateRequest(
	ctx context.Context,
	beaconEndpoint, executionEndpoint string,
	ibcAddress common.Address,
	ibcClientID string,
	isNext bool,
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

	// the period's update snapshot provides the trusted next sync committee;
	// with isNext it is also the update to verify (it carries the next sync
	// committee required for the period transition)
	lcUpdate, err := beaconClient.GetLightClientUpdate(ctx, period)
	if err != nil {
		return nil, fmt.Errorf("failed to get light client update for period %d: %w", period, err)
	}

	var consensusUpdate *lctypes.ConsensusUpdate
	var executionUpdate *lctypes.ExecutionUpdate
	var headerTimestamp uint64
	if isNext {
		consensusUpdate = lcUpdate.Data.ToProto()
		executionUpdate, headerTimestamp, err = relay.BuildExecutionUpdateFromFinalizedHeader(&lcUpdate.Data.FinalizedHeader, false)
	} else {
		consensusUpdate = finalityUpdate.Data.ToProto()
		executionUpdate, headerTimestamp, err = relay.BuildExecutionUpdateFromFinalizedHeader(&finalityUpdate.Data.FinalizedHeader, false)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to build execution update: %w", err)
	}

	bootstrapCommittee, err := relay.GetBootstrapInPeriod(ctx, beaconClient, network, period)
	if err != nil {
		return nil, fmt.Errorf("failed to get bootstrap for period %d: %w", period, err)
	}

	// the trusted state sits in the previous period for a transition update
	trustedPeriod := period
	trustedCurrentAggregate := bootstrapCommittee.AggregatePubkey
	trustedNextAggregate := lcUpdate.Data.NextSyncCommittee.ToProto().AggregatePubkey
	if isNext {
		if period == 0 {
			return nil, errors.New("isNext requires the chain to be past its first sync committee period")
		}
		trustedPeriod = period - 1
		prevCommittee, err := relay.GetBootstrapInPeriod(ctx, beaconClient, network, trustedPeriod)
		if err != nil {
			return nil, fmt.Errorf("failed to get bootstrap for period %d: %w", trustedPeriod, err)
		}
		trustedCurrentAggregate = prevCommittee.AggregatePubkey
		trustedNextAggregate = bootstrapCommittee.AggregatePubkey
	}
	trustedSlot := relay.GetPeriodBoundarySlot(network, trustedPeriod)
	updateFinalizedSlot := uint64(lcUpdate.Data.FinalizedHeader.Beacon.Slot)
	if !isNext {
		updateFinalizedSlot = finalizedSlot
	}
	if updateFinalizedSlot <= trustedSlot {
		return nil, fmt.Errorf("%w: finalized=%d boundary=%d", ErrFinalityNotAdvanced, updateFinalizedSlot, trustedSlot)
	}
	trustedTimestamp := genesis.GenesisTimeSeconds + trustedSlot*relay.SecondsPerSlot(network)

	executionClient, err := rpc.DialContext(ctx, executionEndpoint)
	if err != nil {
		return nil, fmt.Errorf("failed to dial execution endpoint: %w", err)
	}
	defer executionClient.Close()
	accountUpdate, err := relay.BuildAccountUpdate(ctx, proofClient{executionClient}, ibcAddress, executionUpdate.BlockNumber)
	if err != nil {
		return nil, wrapProofErr(fmt.Errorf("failed to build account update: %w", err))
	}

	// optionally prove membership of the client state commitment
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
			return nil, wrapProofErr(fmt.Errorf("failed to build state proof: %w", err))
		}
	}

	// the fork schedule is only consulted for the minimal preset
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
			IsNext:        isNext,
		},
		TrustedCurrentSyncCommittee: trustedCurrentAggregate,
		TrustedNextSyncCommittee:    trustedNextAggregate,
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
