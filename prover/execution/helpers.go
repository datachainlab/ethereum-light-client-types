package execution

import (
	"context"
	"fmt"
	"math/big"
)

// GetBlockTimestamp fetches the timestamp of a block by its number
func GetBlockTimestamp(ctx context.Context, client Client, blockNumber uint64) (uint64, error) {
	header, err := client.HeaderByNumber(ctx, new(big.Int).SetUint64(blockNumber))
	if err != nil {
		return 0, fmt.Errorf("HeaderByNumber failed: %w", err)
	}
	return header.Time, nil
}
