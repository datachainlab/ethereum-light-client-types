package execution

import (
	"context"
	"fmt"
	"math/big"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/common/hexutil"
)

// GetRawHeader fetches RLP-encoded block header via debug_getRawHeader
func GetRawHeader(ctx context.Context, client RPCClient, blockHash common.Hash) ([]byte, error) {
	var result hexutil.Bytes
	if err := client.CallContext(ctx, &result, "debug_getRawHeader", blockHash); err != nil {
		return nil, fmt.Errorf("debug_getRawHeader failed: %w", err)
	}
	return result, nil
}

// GetBlockTimestamp fetches the timestamp of a block by its number
func GetBlockTimestamp(ctx context.Context, client Client, blockNumber uint64) (uint64, error) {
	header, err := client.HeaderByNumber(ctx, big.NewInt(int64(blockNumber)))
	if err != nil {
		return 0, fmt.Errorf("HeaderByNumber failed: %w", err)
	}
	return header.Time, nil
}
