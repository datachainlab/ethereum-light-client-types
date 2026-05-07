package relay

import (
	"bytes"
	"testing"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/crypto"
)

func TestIBCCommitmentsSlot(t *testing.T) {
	slot := IBCCommitmentsSlot()
	if len(slot) != 32 {
		t.Errorf("IBCCommitmentsSlot() length = %d, want 32", len(slot))
	}

	// Verify it's the expected value
	expected := common.HexToHash("1ee222554989dda120e26ecacf756fe1235cd8d726706b57517715dde4f0c900")
	if !bytes.Equal(slot, expected[:]) {
		t.Errorf("IBCCommitmentsSlot() = %x, want %x", slot, expected)
	}
}

func TestIBCCommitmentStorageKey(t *testing.T) {
	tests := []struct {
		name string
		path []byte
	}{
		{"empty path", []byte{}},
		{"simple path", []byte("connections/connection-0")},
		{"channel path", []byte("channelEnds/ports/transfer/channels/channel-0")},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := IBCCommitmentStorageKey(tt.path)

			// Verify the result is computed correctly
			pathHash := crypto.Keccak256Hash(tt.path)
			expected := crypto.Keccak256Hash(append(pathHash.Bytes(), ibcCommitmentsSlot.Bytes()...))

			if result != expected {
				t.Errorf("IBCCommitmentStorageKey(%s) = %x, want %x", tt.path, result, expected)
			}
		})
	}
}

func TestIBCCommitmentStorageKeyDifferentPaths(t *testing.T) {
	path1 := []byte("connections/connection-0")
	path2 := []byte("connections/connection-1")

	key1 := IBCCommitmentStorageKey(path1)
	key2 := IBCCommitmentStorageKey(path2)

	if key1 == key2 {
		t.Error("Different paths should produce different storage keys")
	}
}
