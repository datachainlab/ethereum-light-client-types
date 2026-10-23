package relay

import (
	"context"
	"encoding/json"
	"fmt"
	"testing"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/types"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
)

const (
	testGenesisTime    = 1000
	testGenesisRoot    = "0x0000000000000000000000000000000000000000000000000000000000000001"
	testGenesisVersion = "0x00000001"
)

// gloasFromGenesis / preGloas are the minimal preset's fork parameters with Gloas
// either already active or still far in the future.
var (
	gloasFromGenesis = GetForkParameters(Minimal, map[string]uint64{})
	preGloas         = GetForkParameters(Minimal, map[string]uint64{Gloas: 1_000_000})
)

type mockExecutionBlock struct {
	hash      common.Hash
	timestamp uint64
}

type mockExecutionRPCClient struct {
	blocks map[uint64]mockExecutionBlock
}

func (m *mockExecutionRPCClient) CallContext(_ context.Context, result any, method string, args ...any) error {
	if method != "eth_getBlockByNumber" {
		return fmt.Errorf("unexpected method: %v", method)
	}
	number, err := hexutil.DecodeUint64(args[0].(string))
	if err != nil {
		return err
	}
	block, ok := m.blocks[number]
	if !ok {
		// geth answers with a JSON null, which leaves `result` untouched.
		return nil
	}
	bz, err := json.Marshal(map[string]any{
		"hash":      block.hash,
		"timestamp": hexutil.Uint64(block.timestamp),
	})
	if err != nil {
		return err
	}
	return json.Unmarshal(bz, result)
}

// slotTimestamp is the execution timestamp of the payload built for `slot` on the
// minimal preset.
func slotTimestamp(slot uint64) uint64 {
	return testGenesisTime + MINIMAL_SECONDS_PER_SLOT*slot
}

func blockHash(n byte) common.Hash {
	return common.Hash{31: n}
}

func setGenesis(t *testing.T, m *MockFetcher) {
	t.Helper()
	if err := m.SetResponse("/eth/v1/beacon/genesis", map[string]any{
		"data": map[string]any{
			"genesis_time":            fmt.Sprint(testGenesisTime),
			"genesis_validators_root": testGenesisRoot,
			"genesis_fork_version":    testGenesisVersion,
		},
	}); err != nil {
		t.Fatalf("failed to set genesis response: %v", err)
	}
}

// setBid makes the beacon block at `slot` bid on `parentBlockHash`.
func setBid(t *testing.T, m *MockFetcher, slot uint64, parentBlockHash common.Hash) {
	t.Helper()
	if err := m.SetResponse(fmt.Sprintf("/eth/v2/beacon/blocks/%v", slot), map[string]any{
		"version": "gloas",
		"data": map[string]any{
			"message": map[string]any{
				"body": map[string]any{
					"signed_execution_payload_bid": map[string]any{
						"message": map[string]any{
							"parent_block_hash": parentBlockHash.Hex(),
						},
					},
				},
			},
		},
	}); err != nil {
		t.Fatalf("failed to set bid response for slot %v: %v", slot, err)
	}
}

func TestGetConsensusStateSlotWithBlockNumber(t *testing.T) {
	tests := []struct {
		name           string
		forkParameters *types.ForkParameters
		// blockSlot is the slot the target execution block was built for.
		blockSlot uint64
		// bids maps a beacon slot to the execution block hash its proposer bid on.
		// A slot missing from the map has no block.
		bids     map[uint64]common.Hash
		expected uint64
		wantErr  bool
	}{
		{
			name:           "pre-Gloas returns the block's own slot",
			forkParameters: preGloas,
			blockSlot:      100,
			expected:       100,
		},
		{
			name:           "Gloas returns the next slot that bids on the block",
			forkParameters: gloasFromGenesis,
			blockSlot:      100,
			bids:           map[uint64]common.Hash{101: blockHash(1)},
			expected:       101,
		},
		{
			name:           "Gloas skips slots without a block",
			forkParameters: gloasFromGenesis,
			blockSlot:      100,
			bids:           map[uint64]common.Hash{103: blockHash(1)},
			expected:       103,
		},
		{
			// The bug this function exists for: the block is built in the last slot of
			// period 0 but the header referencing it lands in period 1.
			name:           "Gloas crosses a sync committee period boundary",
			forkParameters: gloasFromGenesis,
			blockSlot:      MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD*MINIMAL_SLOTS_PER_EPOCH - 1,
			bids: map[uint64]common.Hash{
				MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD * MINIMAL_SLOTS_PER_EPOCH: blockHash(1),
			},
			expected: MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD * MINIMAL_SLOTS_PER_EPOCH,
		},
		{
			// A block exists but bids on a different parent, so it does not reference
			// our block. Returning it would send the wrong sync committee.
			name:           "Gloas rejects a slot bidding on another block",
			forkParameters: gloasFromGenesis,
			blockSlot:      100,
			bids:           map[uint64]common.Hash{101: blockHash(9)},
			wantErr:        true,
		},
		{
			name:           "Gloas fails when no slot references the block",
			forkParameters: gloasFromGenesis,
			blockSlot:      100,
			wantErr:        true,
		},
	}

	const targetBlockNumber = 42

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fetcher := NewMockFetcher()
			setGenesis(t, fetcher)
			for slot, parent := range tt.bids {
				setBid(t, fetcher, slot, parent)
			}
			executionClient := &mockExecutionRPCClient{blocks: map[uint64]mockExecutionBlock{
				targetBlockNumber: {hash: blockHash(1), timestamp: slotTimestamp(tt.blockSlot)},
			}}

			slot, err := GetConsensusStateSlotWithBlockNumber(
				context.Background(),
				beacon.NewClientWithFetcher(fetcher),
				executionClient,
				Minimal,
				tt.forkParameters,
				targetBlockNumber,
			)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected an error, got slot=%v", slot)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if slot != tt.expected {
				t.Errorf("slot = %v, want %v", slot, tt.expected)
			}
		})
	}
}

func TestGetConsensusStateSlotWithBlockNumberMissingBlock(t *testing.T) {
	fetcher := NewMockFetcher()
	setGenesis(t, fetcher)

	_, err := GetConsensusStateSlotWithBlockNumber(
		context.Background(),
		beacon.NewClientWithFetcher(fetcher),
		&mockExecutionRPCClient{blocks: map[uint64]mockExecutionBlock{}},
		Minimal,
		gloasFromGenesis,
		42,
	)
	if err == nil {
		t.Fatal("expected an error for an unknown execution block")
	}
}

// The period is what the caller actually consumes, so pin the boundary case end to end.
func TestGetPeriodWithBlockNumberAtPeriodBoundary(t *testing.T) {
	const (
		targetBlockNumber = 42
		periodLength      = MINIMAL_EPOCHS_PER_SYNC_COMMITTEE_PERIOD * MINIMAL_SLOTS_PER_EPOCH
	)

	fetcher := NewMockFetcher()
	setGenesis(t, fetcher)
	setBid(t, fetcher, periodLength, blockHash(1))

	period, err := GetPeriodWithBlockNumber(
		context.Background(),
		beacon.NewClientWithFetcher(fetcher),
		&mockExecutionRPCClient{blocks: map[uint64]mockExecutionBlock{
			targetBlockNumber: {hash: blockHash(1), timestamp: slotTimestamp(periodLength - 1)},
		}},
		Minimal,
		gloasFromGenesis,
		targetBlockNumber,
	)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if period != 1 {
		t.Errorf("period = %v, want 1 (the block sits in period 0 but its header is in period 1)", period)
	}
}
