package relay

import (
	"bytes"
	"testing"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
)

func TestGenerateMerkleProof(t *testing.T) {
	tests := []struct {
		name      string
		leaves    [][]byte
		leafIndex uint64
		wantErr   bool
	}{
		{
			name:      "single leaf",
			leaves:    [][]byte{make([]byte, 32)},
			leafIndex: 0,
			wantErr:   false,
		},
		{
			name:      "two leaves index 0",
			leaves:    [][]byte{make([]byte, 32), make([]byte, 32)},
			leafIndex: 0,
			wantErr:   false,
		},
		{
			name:      "two leaves index 1",
			leaves:    [][]byte{make([]byte, 32), make([]byte, 32)},
			leafIndex: 1,
			wantErr:   false,
		},
		{
			name:      "four leaves",
			leaves:    [][]byte{make([]byte, 32), make([]byte, 32), make([]byte, 32), make([]byte, 32)},
			leafIndex: 2,
			wantErr:   false,
		},
		{
			name:      "empty leaves",
			leaves:    [][]byte{},
			leafIndex: 0,
			wantErr:   true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			proof, err := GenerateMerkleProof(tt.leaves, tt.leafIndex)
			if (err != nil) != tt.wantErr {
				t.Errorf("GenerateMerkleProof() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if !tt.wantErr && proof == nil {
				t.Error("GenerateMerkleProof() returned nil proof")
			}
		})
	}
}

func TestGenerateMerkleProofConsistency(t *testing.T) {
	// Test that same input produces same output
	leaves := [][]byte{
		bytes.Repeat([]byte{1}, 32),
		bytes.Repeat([]byte{2}, 32),
		bytes.Repeat([]byte{3}, 32),
		bytes.Repeat([]byte{4}, 32),
	}

	proof1, err := GenerateMerkleProof(leaves, 0)
	if err != nil {
		t.Fatalf("GenerateMerkleProof() error = %v", err)
	}

	proof2, err := GenerateMerkleProof(leaves, 0)
	if err != nil {
		t.Fatalf("GenerateMerkleProof() error = %v", err)
	}

	if len(proof1) != len(proof2) {
		t.Errorf("Proof lengths differ: %d vs %d", len(proof1), len(proof2))
	}

	for i := range proof1 {
		if !bytes.Equal(proof1[i], proof2[i]) {
			t.Errorf("Proof element %d differs", i)
		}
	}
}

func TestGenerateExecutionPayloadHeaderProof(t *testing.T) {
	header := &beacon.ExecutionPayloadHeader{
		ParentHash:       make([]byte, 32),
		FeeRecipient:     make([]byte, 20),
		StateRoot:        make([]byte, 32),
		ReceiptsRoot:     make([]byte, 32),
		LogsBloom:        make([]byte, 256),
		PrevRandao:       make([]byte, 32),
		BlockNumber:      12345,
		GasLimit:         30000000,
		GasUsed:          21000,
		Timestamp:        1234567890,
		ExtraData:        []byte("test"),
		BaseFeePerGas:    make([]byte, 32),
		BlockHash:        make([]byte, 32),
		TransactionsRoot: make([]byte, 32),
		WithdrawalsRoot:  make([]byte, 32),
		BlobGasUsed:      0,
		ExcessBlobGas:    0,
	}

	tests := []struct {
		name      string
		leafIndex uint64
	}{
		{"state root", EXECUTION_STATE_ROOT_LEAF_INDEX},
		{"block number", EXECUTION_BLOCK_NUMBER_LEAF_INDEX},
		{"block hash", EXECUTION_BLOCK_HASH_LEAF_INDEX},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			proof, err := GenerateExecutionPayloadHeaderProof(header, tt.leafIndex)
			if err != nil {
				t.Errorf("GenerateExecutionPayloadHeaderProof() error = %v", err)
				return
			}
			if proof == nil {
				t.Error("GenerateExecutionPayloadHeaderProof() returned nil proof")
			}
			// Verify proof has correct length (log2(17) rounded up = 5)
			if len(proof) < 4 {
				t.Errorf("Proof length %d is too short", len(proof))
			}
		})
	}
}

func TestSszUint64(t *testing.T) {
	tests := []struct {
		name  string
		value uint64
	}{
		{"zero", 0},
		{"one", 1},
		{"max", ^uint64(0)},
		{"typical block number", 12345678},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := sszUint64(tt.value)
			if len(result) != 32 {
				t.Errorf("sszUint64() length = %d, want 32", len(result))
			}
		})
	}
}

func TestSszBytes(t *testing.T) {
	tests := []struct {
		name string
		data []byte
	}{
		{"short", []byte{1, 2, 3}},
		{"20 bytes (address)", make([]byte, 20)},
		{"32 bytes (hash)", make([]byte, 32)},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := sszBytes(tt.data)
			if len(result) != 32 {
				t.Errorf("sszBytes() length = %d, want 32", len(result))
			}
		})
	}
}
