package relay

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"testing"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/ethereum/go-ethereum/common/hexutil"
)

// MockFetcher is a mock implementation of beacon.Fetcher for testing
type MockFetcher struct {
	responses map[string][]byte
	errors    map[string]error
}

func NewMockFetcher() *MockFetcher {
	return &MockFetcher{
		responses: make(map[string][]byte),
		errors:    make(map[string]error),
	}
}

func (m *MockFetcher) SetResponse(path string, response any) error {
	bz, err := json.Marshal(response)
	if err != nil {
		return err
	}
	m.responses[path] = bz
	return nil
}

func (m *MockFetcher) SetRawResponse(path string, response []byte) {
	m.responses[path] = response
}

func (m *MockFetcher) SetError(path string, err error) {
	m.errors[path] = err
}

func (m *MockFetcher) Get(ctx context.Context, path string, result any) error {
	if err, ok := m.errors[path]; ok {
		return err
	}
	if bz, ok := m.responses[path]; ok {
		return json.Unmarshal(bz, result)
	}
	return fmt.Errorf("no mock response for path: %s", path)
}

// makeBootstrapResponse creates a properly formatted bootstrap response JSON
func makeBootstrapResponse(pubkeys [][]byte, aggregatePubkey []byte) []byte {
	// Build pubkeys array
	pubkeysJSON := "["
	for i, pk := range pubkeys {
		if i > 0 {
			pubkeysJSON += ","
		}
		pubkeysJSON += fmt.Sprintf(`"0x%s"`, hex.EncodeToString(pk))
	}
	pubkeysJSON += "]"

	return []byte(fmt.Sprintf(`{
		"version": "deneb",
		"data": {
			"header": {
				"beacon": {
					"slot": "1",
					"proposer_index": "1",
					"parent_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"state_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"body_root": "0x0000000000000000000000000000000000000000000000000000000000000000"
				},
				"execution": {
					"parent_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"fee_recipient": "0x0000000000000000000000000000000000000000",
					"state_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"receipts_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"logs_bloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
					"prev_randao": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"block_number": "1",
					"gas_limit": "30000000",
					"gas_used": "0",
					"timestamp": "0",
					"extra_data": "0x",
					"base_fee_per_gas": "0",
					"block_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"transactions_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"withdrawals_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
					"blob_gas_used": "0",
					"excess_blob_gas": "0"
				},
				"execution_branch": []
			},
			"current_sync_committee": {
				"pubkeys": %s,
				"aggregate_pubkey": "0x%s"
			},
			"current_sync_committee_branch": []
		}
	}`, pubkeysJSON, hex.EncodeToString(aggregatePubkey)))
}

func TestGetBootstrapInPeriod_Success(t *testing.T) {
	ctx := context.Background()
	mockFetcher := NewMockFetcher()

	// Set up mock data for mainnet period 0
	// First epoch slot in period 0 is slot 32 (slots per epoch)
	slot := uint64(32)
	root := make([]byte, 32)
	root[0] = 0x01

	blockRootPath := fmt.Sprintf("/eth/v1/beacon/blocks/%d/root", slot)
	mockFetcher.SetResponse(blockRootPath, beacon.BlockRootResponse{
		Data: struct {
			Root hexutil.Bytes `json:"root"`
		}{
			Root: root,
		},
	})

	expectedPubkey := make([]byte, 48)
	expectedPubkey[0] = 0xAB
	expectedAggregate := make([]byte, 48)
	expectedAggregate[0] = 0xCD

	bootstrapPath := fmt.Sprintf("/eth/v1/beacon/light_client/bootstrap/0x%s", hex.EncodeToString(root))
	mockFetcher.SetRawResponse(bootstrapPath, makeBootstrapResponse([][]byte{expectedPubkey}, expectedAggregate))

	client := beacon.NewClientWithFetcher(mockFetcher)
	result, err := GetBootstrapInPeriod(ctx, client, Mainnet, 0)
	if err != nil {
		t.Fatalf("GetBootstrapInPeriod() error = %v", err)
	}

	if result == nil {
		t.Fatal("expected non-nil result")
	}

	if len(result.Pubkeys) != 1 {
		t.Errorf("expected 1 pubkey, got %d", len(result.Pubkeys))
	}
}

func TestGetBootstrapInPeriod_BlockRootError(t *testing.T) {
	ctx := context.Background()
	mockFetcher := NewMockFetcher()

	// Set error for all block root requests
	slot := uint64(32)
	blockRootPath := fmt.Sprintf("/eth/v1/beacon/blocks/%d/root", slot)
	mockFetcher.SetError(blockRootPath, errors.New("rpc error"))

	client := beacon.NewClientWithFetcher(mockFetcher)
	_, err := GetBootstrapInPeriod(ctx, client, Mainnet, 0)
	if err == nil {
		t.Error("expected error when GetBlockRoot fails")
	}
}

func TestGetBootstrapInPeriod_BootstrapNotFound(t *testing.T) {
	ctx := context.Background()
	mockFetcher := NewMockFetcher()

	// Set up block roots for all epochs in the period, but no bootstraps
	slotsPerEpoch := SlotsPerEpoch(Minimal)
	startSlot := GetPeriodBoundarySlot(Minimal, 0)
	lastSlotInPeriod := GetPeriodBoundarySlot(Minimal, 1) - 1

	for slot := startSlot + slotsPerEpoch; slot <= lastSlotInPeriod; slot += slotsPerEpoch {
		root := make([]byte, 32)
		root[0] = byte(slot % 256)

		blockRootPath := fmt.Sprintf("/eth/v1/beacon/blocks/%d/root", slot)
		mockFetcher.SetResponse(blockRootPath, beacon.BlockRootResponse{
			Data: struct {
				Root hexutil.Bytes `json:"root"`
			}{
				Root: root,
			},
		})
		// No bootstrap response set - will return error
	}

	client := beacon.NewClientWithFetcher(mockFetcher)
	_, err := GetBootstrapInPeriod(ctx, client, Minimal, 0)
	if err == nil {
		t.Error("expected error when no bootstrap found")
	}
}

func TestGetBootstrapInPeriod_MinimalNetwork(t *testing.T) {
	ctx := context.Background()
	mockFetcher := NewMockFetcher()

	// Minimal network: 8 slots per epoch, 8 epochs per sync committee period
	// First epoch slot in period 1 is slot 64 + 8 = 72
	slot := uint64(72)
	root := make([]byte, 32)
	root[0] = 0x02

	blockRootPath := fmt.Sprintf("/eth/v1/beacon/blocks/%d/root", slot)
	mockFetcher.SetResponse(blockRootPath, beacon.BlockRootResponse{
		Data: struct {
			Root hexutil.Bytes `json:"root"`
		}{
			Root: root,
		},
	})

	bootstrapPath := fmt.Sprintf("/eth/v1/beacon/light_client/bootstrap/0x%s", hex.EncodeToString(root))
	mockFetcher.SetRawResponse(bootstrapPath, makeBootstrapResponse([][]byte{make([]byte, 48)}, make([]byte, 48)))

	client := beacon.NewClientWithFetcher(mockFetcher)
	result, err := GetBootstrapInPeriod(ctx, client, Minimal, 1)
	if err != nil {
		t.Fatalf("GetBootstrapInPeriod() error = %v", err)
	}

	if result == nil {
		t.Fatal("expected non-nil result")
	}
}

func TestGetBootstrapInPeriod_RetryOnBootstrapError(t *testing.T) {
	ctx := context.Background()
	mockFetcher := NewMockFetcher()

	slotsPerEpoch := SlotsPerEpoch(Minimal)
	startSlot := GetPeriodBoundarySlot(Minimal, 0)

	// First slot returns block root but bootstrap fails
	firstSlot := startSlot + slotsPerEpoch
	firstRoot := make([]byte, 32)
	firstRoot[0] = 0x01

	firstBlockRootPath := fmt.Sprintf("/eth/v1/beacon/blocks/%d/root", firstSlot)
	mockFetcher.SetResponse(firstBlockRootPath, beacon.BlockRootResponse{
		Data: struct {
			Root hexutil.Bytes `json:"root"`
		}{
			Root: firstRoot,
		},
	})
	// No bootstrap for first root - will fail

	// Second slot succeeds
	secondSlot := startSlot + 2*slotsPerEpoch
	secondRoot := make([]byte, 32)
	secondRoot[0] = 0x02

	secondBlockRootPath := fmt.Sprintf("/eth/v1/beacon/blocks/%d/root", secondSlot)
	mockFetcher.SetResponse(secondBlockRootPath, beacon.BlockRootResponse{
		Data: struct {
			Root hexutil.Bytes `json:"root"`
		}{
			Root: secondRoot,
		},
	})

	secondBootstrapPath := fmt.Sprintf("/eth/v1/beacon/light_client/bootstrap/0x%s", hex.EncodeToString(secondRoot))
	mockFetcher.SetRawResponse(secondBootstrapPath, makeBootstrapResponse([][]byte{make([]byte, 48)}, make([]byte, 48)))

	client := beacon.NewClientWithFetcher(mockFetcher)
	result, err := GetBootstrapInPeriod(ctx, client, Minimal, 0)
	if err != nil {
		t.Fatalf("GetBootstrapInPeriod() error = %v", err)
	}

	if result == nil {
		t.Fatal("expected non-nil result after retry")
	}
}
