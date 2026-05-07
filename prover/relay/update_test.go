package relay

import (
	"bytes"
	"context"
	"reflect"
	"testing"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/rlp"
)

// MockRPCClient is a mock implementation of execution.RPCClient
type MockRPCClient struct {
	RawHeader []byte
	Err       error
}

func (m *MockRPCClient) CallContext(ctx context.Context, result interface{}, method string, args ...interface{}) error {
	if m.Err != nil {
		return m.Err
	}
	if method == "debug_getRawHeader" {
		// result is *hexutil.Bytes (which is *[]byte under the hood but different type)
		// Use reflect to set the value
		rv := reflect.ValueOf(result)
		if rv.Kind() == reflect.Ptr && rv.Elem().Kind() == reflect.Slice {
			rv.Elem().SetBytes(m.RawHeader)
		}
	}
	return nil
}

func TestBuildExecutionUpdate(t *testing.T) {
	header := &beacon.ExecutionPayloadHeader{
		ParentHash:       bytes.Repeat([]byte{1}, 32),
		FeeRecipient:     bytes.Repeat([]byte{2}, 20),
		StateRoot:        bytes.Repeat([]byte{3}, 32),
		ReceiptsRoot:     bytes.Repeat([]byte{4}, 32),
		LogsBloom:        make([]byte, 256),
		PrevRandao:       bytes.Repeat([]byte{5}, 32),
		BlockNumber:      12345,
		GasLimit:         30000000,
		GasUsed:          21000,
		Timestamp:        1234567890,
		ExtraData:        []byte("test"),
		BaseFeePerGas:    bytes.Repeat([]byte{6}, 32),
		BlockHash:        bytes.Repeat([]byte{7}, 32),
		TransactionsRoot: bytes.Repeat([]byte{8}, 32),
		WithdrawalsRoot:  bytes.Repeat([]byte{9}, 32),
		BlobGasUsed:      0,
		ExcessBlobGas:    0,
	}

	t.Run("without block hash", func(t *testing.T) {
		update, err := BuildExecutionUpdate(header, false)
		if err != nil {
			t.Fatalf("BuildExecutionUpdate() error = %v", err)
		}

		if !bytes.Equal(update.StateRoot, header.StateRoot) {
			t.Error("StateRoot mismatch")
		}
		if update.BlockNumber != header.BlockNumber {
			t.Errorf("BlockNumber = %d, want %d", update.BlockNumber, header.BlockNumber)
		}
		if len(update.StateRootBranch) == 0 {
			t.Error("StateRootBranch is empty")
		}
		if len(update.BlockNumberBranch) == 0 {
			t.Error("BlockNumberBranch is empty")
		}
		if update.BlockHash != nil {
			t.Error("BlockHash should be nil when includeBlockHash is false")
		}
		if update.BlockHashBranch != nil {
			t.Error("BlockHashBranch should be nil when includeBlockHash is false")
		}
	})

	t.Run("with block hash", func(t *testing.T) {
		update, err := BuildExecutionUpdate(header, true)
		if err != nil {
			t.Fatalf("BuildExecutionUpdate() error = %v", err)
		}

		if !bytes.Equal(update.BlockHash, header.BlockHash) {
			t.Error("BlockHash mismatch")
		}
		if len(update.BlockHashBranch) == 0 {
			t.Error("BlockHashBranch is empty")
		}
	})
}

func TestBuildExecutionUpdateFromFinalizedHeader_PreGloas(t *testing.T) {
	header := &beacon.LightClientHeader{
		Execution: &beacon.ExecutionPayloadHeader{
			ParentHash:       bytes.Repeat([]byte{1}, 32),
			FeeRecipient:     bytes.Repeat([]byte{2}, 20),
			StateRoot:        bytes.Repeat([]byte{3}, 32),
			ReceiptsRoot:     bytes.Repeat([]byte{4}, 32),
			LogsBloom:        make([]byte, 256),
			PrevRandao:       bytes.Repeat([]byte{5}, 32),
			BlockNumber:      12345,
			GasLimit:         30000000,
			GasUsed:          21000,
			Timestamp:        1234567890,
			ExtraData:        []byte("test"),
			BaseFeePerGas:    bytes.Repeat([]byte{6}, 32),
			BlockHash:        bytes.Repeat([]byte{7}, 32),
			TransactionsRoot: bytes.Repeat([]byte{8}, 32),
			WithdrawalsRoot:  bytes.Repeat([]byte{9}, 32),
			BlobGasUsed:      0,
			ExcessBlobGas:    0,
		},
		ExecutionBranch:    make([]hexutil.Bytes, 4),
		ExecutionBlockHash: nil, // pre-Gloas
	}

	ctx := context.Background()
	mockClient := &MockRPCClient{}

	update, timestamp, err := BuildExecutionUpdateFromFinalizedHeader(ctx, mockClient, header, false)
	if err != nil {
		t.Fatalf("BuildExecutionUpdateFromFinalizedHeader() error = %v", err)
	}

	if timestamp != header.Execution.Timestamp {
		t.Errorf("timestamp = %d, want %d", timestamp, header.Execution.Timestamp)
	}
	if update.Rlp != nil {
		t.Error("Rlp should be nil for pre-Gloas")
	}
}

func TestBuildExecutionUpdateFromFinalizedHeader_Gloas(t *testing.T) {
	// Create a valid RLP-encoded header
	gethHeader := &types.Header{
		ParentHash:  common.HexToHash("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"),
		UncleHash:   common.HexToHash("0x0000000000000000000000000000000000000000000000000000000000000000"),
		Coinbase:    common.HexToAddress("0x0000000000000000000000000000000000000000"),
		Root:        common.HexToHash("0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"),
		TxHash:      common.HexToHash("0x0000000000000000000000000000000000000000000000000000000000000000"),
		ReceiptHash: common.HexToHash("0x0000000000000000000000000000000000000000000000000000000000000000"),
		Difficulty:  common.Big0,
		Number:      common.Big1,
		GasLimit:    30000000,
		GasUsed:     21000,
		Time:        1234567890,
		Extra:       []byte{},
		MixDigest:   common.Hash{},
		Nonce:       types.BlockNonce{},
	}

	rlpHeader, err := rlp.EncodeToBytes(gethHeader)
	if err != nil {
		t.Fatalf("Failed to encode header: %v", err)
	}

	blockHash := gethHeader.Hash()

	header := &beacon.LightClientHeader{
		Execution:          nil, // Gloas doesn't use ExecutionPayloadHeader
		ExecutionBranch:    make([]hexutil.Bytes, 4),
		ExecutionBlockHash: blockHash[:], // Gloas uses block hash
	}

	ctx := context.Background()
	mockClient := &MockRPCClient{
		RawHeader: rlpHeader,
	}

	update, timestamp, err := BuildExecutionUpdateFromFinalizedHeader(ctx, mockClient, header, false)
	if err != nil {
		t.Fatalf("BuildExecutionUpdateFromFinalizedHeader() error = %v", err)
	}

	if timestamp != gethHeader.Time {
		t.Errorf("timestamp = %d, want %d", timestamp, gethHeader.Time)
	}
	if update.Rlp == nil {
		t.Error("Rlp should not be nil for Gloas")
	}
	if !bytes.Equal(update.StateRoot, gethHeader.Root.Bytes()) {
		t.Error("StateRoot mismatch")
	}
	if update.BlockNumber != gethHeader.Number.Uint64() {
		t.Errorf("BlockNumber = %d, want %d", update.BlockNumber, gethHeader.Number.Uint64())
	}
}
