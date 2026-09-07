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

// Field positions in the RLP-encoded execution block header. The header is a flat
// RLP list and forks only ever append to it, so the leading positions are stable.
//
// Decoding positionally rather than into go-ethereum's types.Header is deliberate:
// Glamsterdam appends block_access_list_hash (EIP-7928) and slot_number (EIP-7732),
// which types.Header does not know about, and a struct decode rejects the header with
// "input list has too many elements". Only these three fields are needed here; the raw
// RLP is passed through to the verifier, which checks keccak256(rlp) == block hash.
const (
	execHeaderStateRootIndex   = 3
	execHeaderBlockNumberIndex = 8
	execHeaderTimestampIndex   = 11
	execHeaderMinFields        = execHeaderTimestampIndex + 1
)

// BuildExecutionUpdateFromBlockHash builds ExecutionUpdate using RLP verification (Gloas)
func BuildExecutionUpdateFromBlockHash(ctx context.Context, executionClient execution.RPCClient, blockHash []byte) (*types.ExecutionUpdate, error) {
	hash := common.BytesToHash(blockHash)

	// Fetch RLP-encoded header via debug_getRawHeader
	rlpHeader, err := execution.GetRawHeader(ctx, executionClient, hash)
	if err != nil {
		return nil, fmt.Errorf("failed to get raw header: %w", err)
	}

	// Decode RLP to extract state_root and block_number
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

	// For Gloas, we use RLP verification instead of SSZ merkle proofs
	// The verifier will check: keccak256(rlp) == execution_block_hash
	return &types.ExecutionUpdate{
		StateRoot:   stateRoot.Bytes(),
		BlockNumber: blockNumber,
		Rlp:         rlpHeader,
		// For Gloas the execution root is the block hash itself; the verifier
		// checks that this equals the consensus update's finalized execution root.
		BlockHash: hash.Bytes(),
	}, nil
}

// BuildExecutionUpdateFromFinalizedHeader builds ExecutionUpdate from finalized header.
// Handles both Gloas (RLP-based) and pre-Gloas (SSZ merkle proof) cases.
// If includeBlockHashPreGloas is true, it also includes BlockHash and BlockHashBranch for pre-Gloas (required for optimism).
func BuildExecutionUpdateFromFinalizedHeader(ctx context.Context, executionClient execution.RPCClient, finalizedHeader *beacon.LightClientHeader, includeBlockHashPreGloas bool) (*types.ExecutionUpdate, error) {
	if finalizedHeader.IsGloas() {
		return BuildExecutionUpdateFromBlockHash(ctx, executionClient, finalizedHeader.ExecutionBlockHash)
	}
	return BuildExecutionUpdate(finalizedHeader.Execution, includeBlockHashPreGloas)
}

// ExecutionHeaderTimestamp returns the timestamp, in unix seconds, of the execution block
// that `executionUpdate` describes.
//
// This mirrors `ExecutionUpdateInfo::timestamp` on the verifier side, which derives the same
// value rather than accepting one from a relayer. A prover recording an initial consensus
// state must use this, so that the timestamp it stores is the one the light client will
// derive when it applies the next update.
//
// Neither branch performs a request: pre-Gloas the value is in the light client header, and
// post-Gloas it is in the RLP that `executionUpdate` already carries.
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
