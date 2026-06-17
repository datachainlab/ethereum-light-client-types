package execution

import (
	"context"
	"math/big"

	gethtypes "github.com/ethereum/go-ethereum/core/types"
)

// Client abstracts execution layer client operations
type Client interface {
	HeaderByNumber(ctx context.Context, number *big.Int) (*gethtypes.Header, error)
}
