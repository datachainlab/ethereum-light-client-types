package relay

import (
	"context"
	"fmt"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/execution"
	"github.com/datachainlab/ethereum-light-client-types/prover/types"
	"github.com/ethereum/go-ethereum/common"
	gethtypes "github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/rlp"
)

// BuildExecutionUpdate builds ExecutionUpdate from ExecutionPayloadHeader (pre-Gloas)
// If includeBlockHash is true, it also includes BlockHash and BlockHashBranch (required for optimism).
func BuildExecutionUpdate(executionHeader *beacon.ExecutionPayloadHeader, includeBlockHash bool) (*types.ExecutionUpdate, error) {
	stateRootBranch, err := GenerateExecutionPayloadHeaderProof(executionHeader, EXECUTION_STATE_ROOT_LEAF_INDEX)
	if err != nil {
		return nil, err
	}
	blockNumberBranch, err := GenerateExecutionPayloadHeaderProof(executionHeader, EXECUTION_BLOCK_NUMBER_LEAF_INDEX)
	if err != nil {
		return nil, err
	}
	update := &types.ExecutionUpdate{
		StateRoot:         executionHeader.StateRoot,
		StateRootBranch:   stateRootBranch,
		BlockNumber:       executionHeader.BlockNumber,
		BlockNumberBranch: blockNumberBranch,
		// BlockHash must always be a well-formed 32-byte value. It is only meaningful
		// for the optimism L1 light client; otherwise send the zero hash (the ethereum
		// light client does not use it).
		BlockHash: make([]byte, 32),
	}
	if includeBlockHash {
		blockHashBranch, err := GenerateExecutionPayloadHeaderProof(executionHeader, EXECUTION_BLOCK_HASH_LEAF_INDEX)
		if err != nil {
			return nil, err
		}
		update.BlockHash = executionHeader.BlockHash
		update.BlockHashBranch = blockHashBranch
	}
	return update, nil
}

// BuildExecutionUpdateFromBlockHash builds ExecutionUpdate using RLP verification (Gloas)
func BuildExecutionUpdateFromBlockHash(ctx context.Context, executionClient execution.RPCClient, blockHash []byte) (*types.ExecutionUpdate, uint64, error) {
	hash := common.BytesToHash(blockHash)

	// Fetch RLP-encoded header via debug_getRawHeader
	rlpHeader, err := execution.GetRawHeader(ctx, executionClient, hash)
	if err != nil {
		return nil, 0, fmt.Errorf("failed to get raw header: %w", err)
	}

	// Decode RLP to extract state_root and block_number
	header := new(gethtypes.Header)
	if err := rlp.DecodeBytes(rlpHeader, header); err != nil {
		return nil, 0, fmt.Errorf("failed to decode RLP header: %w", err)
	}

	// For Gloas, we use RLP verification instead of SSZ merkle proofs
	// The verifier will check: keccak256(rlp) == execution_block_hash
	return &types.ExecutionUpdate{
		StateRoot:   header.Root.Bytes(),
		BlockNumber: header.Number.Uint64(),
		Rlp:         rlpHeader,
	}, header.Time, nil
}

// BuildExecutionUpdateFromFinalizedHeader builds ExecutionUpdate from finalized header.
// Handles both Gloas (RLP-based) and pre-Gloas (SSZ merkle proof) cases.
// If includeBlockHashPreGloas is true, it also includes BlockHash and BlockHashBranch for pre-Gloas (required for optimism).
func BuildExecutionUpdateFromFinalizedHeader(ctx context.Context, executionClient execution.RPCClient, finalizedHeader *beacon.LightClientHeader, includeBlockHashPreGloas bool) (*types.ExecutionUpdate, uint64, error) {
	if finalizedHeader.IsGloas() {
		return BuildExecutionUpdateFromBlockHash(ctx, executionClient, finalizedHeader.ExecutionBlockHash)
	}
	executionHeader := finalizedHeader.Execution
	executionUpdate, err := BuildExecutionUpdate(executionHeader, includeBlockHashPreGloas)
	if err != nil {
		return nil, 0, err
	}
	return executionUpdate, executionHeader.Timestamp, nil
}
