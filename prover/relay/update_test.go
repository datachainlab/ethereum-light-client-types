package relay

import (
	"bytes"
	"testing"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/ethereum/go-ethereum/common/hexutil"
)

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
		// BlockHash is always a well-formed 32-byte value; the zero hash when not included.
		if !bytes.Equal(update.BlockHash, make([]byte, 32)) {
			t.Error("BlockHash should be the 32-byte zero hash when includeBlockHash is false")
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

func TestBuildExecutionUpdateFromFinalizedHeader(t *testing.T) {
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
		ExecutionBranch: make([]hexutil.Bytes, 4),
	}

	update, timestamp, err := BuildExecutionUpdateFromFinalizedHeader(header, false)
	if err != nil {
		t.Fatalf("BuildExecutionUpdateFromFinalizedHeader() error = %v", err)
	}

	if timestamp != header.Execution.Timestamp {
		t.Errorf("timestamp = %d, want %d", timestamp, header.Execution.Timestamp)
	}
	if !bytes.Equal(update.StateRoot, header.Execution.StateRoot) {
		t.Error("StateRoot mismatch")
	}
	if update.BlockNumber != header.Execution.BlockNumber {
		t.Errorf("BlockNumber = %d, want %d", update.BlockNumber, header.Execution.BlockNumber)
	}
}
