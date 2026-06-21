package relay

import (
	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/types"
)

// BuildExecutionUpdate builds ExecutionUpdate from ExecutionPayloadHeader.
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

// BuildExecutionUpdateFromFinalizedHeader builds ExecutionUpdate from a finalized header.
// If includeBlockHash is true, it also includes BlockHash and BlockHashBranch (required for optimism).
func BuildExecutionUpdateFromFinalizedHeader(finalizedHeader *beacon.LightClientHeader, includeBlockHash bool) (*types.ExecutionUpdate, uint64, error) {
	executionHeader := finalizedHeader.Execution
	executionUpdate, err := BuildExecutionUpdate(executionHeader, includeBlockHash)
	if err != nil {
		return nil, 0, err
	}
	return executionUpdate, executionHeader.Timestamp, nil
}
