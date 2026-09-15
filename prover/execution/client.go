package execution

import (
	"context"
	"math/big"

	gethtypes "github.com/ethereum/go-ethereum/core/types"
)

// RPCClient abstracts raw JSON-RPC operations for execution layer
type RPCClient interface {
	CallContext(ctx context.Context, result interface{}, method string, args ...interface{}) error
}

// Client abstracts execution layer client operations
type Client interface {
	HeaderByNumber(ctx context.Context, number *big.Int) (*gethtypes.Header, error)
}
