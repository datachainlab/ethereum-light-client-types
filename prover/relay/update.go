package relay

import (
	"context"
	"fmt"

	"github.com/datachainlab/ethereum-light-client-types/prover/beacon"
	"github.com/datachainlab/ethereum-light-client-types/prover/execution"
	"github.com/datachainlab/ethereum-light-client-types/prover/types"
	"github.com/ethereum/go-ethereum/common"
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

// Field positions in the RLP-encoded execution block header. Forks only append to the
// list, so the leading positions are stable.
//
// Decoding positionally rather than into go-ethereum's types.Header is deliberate:
// Glamsterdam appends block_access_list_hash (EIP-7928) and slot_number (EIP-7843),
// which types.Header does not know about, so a struct decode rejects the header with
// "input list has too many elements".
const (
	execHeaderStateRootIndex   = 3
	execHeaderBlockNumberIndex = 8
	execHeaderTimestampIndex   = 11
	execHeaderMinFields        = execHeaderTimestampIndex + 1
)

// BuildExecutionUpdateFromBlockHash builds ExecutionUpdate for Gloas, where the verifier
// proves the header by checking keccak256(rlp) == execution_block_hash instead of walking
// SSZ merkle branches.
func BuildExecutionUpdateFromBlockHash(ctx context.Context, executionClient execution.RPCClient, blockHash []byte) (*types.ExecutionUpdate, error) {
	hash := common.BytesToHash(blockHash)

	rlpHeader, err := execution.GetRawHeader(ctx, executionClient, hash)
	if err != nil {
		return nil, fmt.Errorf("failed to get raw header: %w", err)
	}

	fields, err := decodeExecutionHeaderFields(rlpHeader)
	if err != nil {
		return nil, err
	}

	var stateRoot common.Hash
	if err := rlp.DecodeBytes(fields[execHeaderStateRootIndex], &stateRoot); err != nil {
		return nil, fmt.Errorf("failed to decode state root: %w", err)
	}
	var blockNumber uint64
	if err := rlp.DecodeBytes(fields[execHeaderBlockNumberIndex], &blockNumber); err != nil {
		return nil, fmt.Errorf("failed to decode block number: %w", err)
	}

	return &types.ExecutionUpdate{
		StateRoot:   stateRoot.Bytes(),
		BlockNumber: blockNumber,
		Rlp:         rlpHeader,
		// For Gloas the execution root is the block hash itself.
		BlockHash: hash.Bytes(),
	}, nil
}

// BuildExecutionUpdateFromFinalizedHeader builds ExecutionUpdate from a finalized header.
// If includeBlockHashPreGloas is true, it also includes BlockHash and BlockHashBranch for pre-Gloas (required for optimism).
func BuildExecutionUpdateFromFinalizedHeader(ctx context.Context, executionClient execution.RPCClient, finalizedHeader *beacon.LightClientHeader, includeBlockHashPreGloas bool) (*types.ExecutionUpdate, error) {
	if finalizedHeader.IsGloas() {
		return BuildExecutionUpdateFromBlockHash(ctx, executionClient, finalizedHeader.ExecutionBlockHash)
	}
	return BuildExecutionUpdate(finalizedHeader.Execution, includeBlockHashPreGloas)
}

// ExecutionHeaderTimestamp returns the timestamp, in unix seconds, of the execution block
// that `executionUpdate` describes. It mirrors `ExecutionUpdateInfo::timestamp` on the
// verifier side, so an initial consensus state records the timestamp the light client will
// derive when it applies the next update.
func ExecutionHeaderTimestamp(finalizedHeader *beacon.LightClientHeader, executionUpdate *types.ExecutionUpdate) (uint64, error) {
	if !finalizedHeader.IsGloas() {
		return finalizedHeader.Execution.Timestamp, nil
	}
	fields, err := decodeExecutionHeaderFields(executionUpdate.Rlp)
	if err != nil {
		return 0, err
	}
	var timestamp uint64
	if err := rlp.DecodeBytes(fields[execHeaderTimestampIndex], &timestamp); err != nil {
		return 0, fmt.Errorf("failed to decode timestamp: %w", err)
	}
	return timestamp, nil
}

func decodeExecutionHeaderFields(rlpHeader []byte) ([]rlp.RawValue, error) {
	var fields []rlp.RawValue
	if err := rlp.DecodeBytes(rlpHeader, &fields); err != nil {
		return nil, fmt.Errorf("failed to decode RLP header: %w", err)
	}
	if len(fields) < execHeaderMinFields {
		return nil, fmt.Errorf("unexpected RLP header: got %d fields, want at least %d", len(fields), execHeaderMinFields)
	}
	return fields, nil
}
