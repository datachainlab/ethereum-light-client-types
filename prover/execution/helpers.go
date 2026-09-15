package execution

import (
	"context"
	"fmt"
	"math/big"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
)

// GetRawHeader fetches the RLP-encoded block header. Requires the node's debug namespace.
func GetRawHeader(ctx context.Context, client RPCClient, blockHash common.Hash) ([]byte, error) {
	var result hexutil.Bytes
	if err := client.CallContext(ctx, &result, "debug_getRawHeader", blockHash); err != nil {
		return nil, fmt.Errorf("debug_getRawHeader failed: %w", err)
	}
	return result, nil
}

// GetBlockTimestamp fetches the timestamp of a block by its number
func GetBlockTimestamp(ctx context.Context, client Client, blockNumber uint64) (uint64, error) {
	header, err := client.HeaderByNumber(ctx, new(big.Int).SetUint64(blockNumber))
	if err != nil {
		return 0, fmt.Errorf("HeaderByNumber failed: %w", err)
	}
	return header.Time, nil
}

type BlockHeaderFields struct {
	Hash      common.Hash
	Timestamp uint64
}

// GetBlockHeaderFields fetches the hash and timestamp of a block by its number.
//
// The hash must come from the node, so this uses eth_getBlockByNumber rather than
// go-ethereum's HeaderByNumber: Glamsterdam appends block_access_list_hash (EIP-7928) and
// slot_number (EIP-7843), which types.Header does not know about, so a locally recomputed
// keccak256(rlp) is wrong.
func GetBlockHeaderFields(ctx context.Context, client RPCClient, blockNumber uint64) (*BlockHeaderFields, error) {
	var raw struct {
		Hash      *common.Hash    `json:"hash"`
		Timestamp *hexutil.Uint64 `json:"timestamp"`
	}
	if err := client.CallContext(ctx, &raw, "eth_getBlockByNumber", hexutil.EncodeUint64(blockNumber), false); err != nil {
		return nil, fmt.Errorf("eth_getBlockByNumber failed: block_number=%v %w", blockNumber, err)
	}
	if raw.Hash == nil || raw.Timestamp == nil {
		return nil, fmt.Errorf("block not found: block_number=%v", blockNumber)
	}
	return &BlockHeaderFields{Hash: *raw.Hash, Timestamp: uint64(*raw.Timestamp)}, nil
}
