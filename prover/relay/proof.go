package relay

import (
	"context"
	"math/big"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/crypto"

	lctypes "github.com/datachainlab/ethereum-light-client-types/prover/types"
)

// StateProof is the result of an eth_getProof call, containing the
// RLP-encoded account proof, the account's storage hash and the
// RLP-encoded storage proofs for the requested storage keys.
type StateProof struct {
	StorageHash     [32]byte
	AccountProofRLP []byte
	StorageProofRLP [][]byte
}

// ProofClient abstracts the eth_getProof RPC operations
type ProofClient interface {
	GetProof(ctx context.Context, address common.Address, storageKeys [][]byte, blockNumber *big.Int) (*StateProof, error)
}

// ibcCommitmentsSlot is the storage slot for IBC commitments mapping in the IBC handler contract.
// This is keccak256("ibc.commitment") - 1, following EIP-1967 style slot calculation.
var ibcCommitmentsSlot = common.HexToHash("1ee222554989dda120e26ecacf756fe1235cd8d726706b57517715dde4f0c900")

// IBCCommitmentsSlot returns the storage slot bytes for IBC commitments mapping.
func IBCCommitmentsSlot() []byte {
	return ibcCommitmentsSlot[:]
}

// IBCCommitmentStorageKey calculates the storage key for an IBC commitment.
// The key is computed as: keccak256(keccak256(path) || ibcCommitmentsSlot)
func IBCCommitmentStorageKey(path []byte) common.Hash {
	return crypto.Keccak256Hash(append(
		crypto.Keccak256Hash(path).Bytes(),
		ibcCommitmentsSlot.Bytes()...,
	))
}

// BuildAccountUpdate builds an AccountUpdate from the account proof at the given block number
func BuildAccountUpdate(ctx context.Context, proofClient ProofClient, ibcAddress common.Address, blockNumber uint64) (*lctypes.AccountUpdate, error) {
	proof, err := proofClient.GetProof(
		ctx,
		ibcAddress,
		nil,
		new(big.Int).SetUint64(blockNumber),
	)
	if err != nil {
		return nil, err
	}
	return &lctypes.AccountUpdate{
		AccountProof:       proof.AccountProofRLP,
		AccountStorageRoot: proof.StorageHash[:],
	}, nil
}

// BuildStateProof builds a storage proof for the given IBC commitment path
func BuildStateProof(ctx context.Context, proofClient ProofClient, ibcAddress common.Address, path []byte, height int64) ([]byte, error) {
	storageKey := IBCCommitmentStorageKey(path)
	storageKeyHex, err := storageKey.MarshalText()
	if err != nil {
		return nil, err
	}

	stateProof, err := proofClient.GetProof(
		ctx,
		ibcAddress,
		[][]byte{storageKeyHex},
		big.NewInt(height),
	)
	if err != nil {
		return nil, err
	}
	return stateProof.StorageProofRLP[0], nil
}
