package beacon

import (
	"context"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"slices"

	"go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp"
)

var SupportedVersions = []string{"deneb", "electra", "fulu", "gloas"}

var httpClient = &http.Client{
	Transport: otelhttp.NewTransport(http.DefaultTransport),
}

// Fetcher abstracts HTTP GET operations for beacon API
type Fetcher interface {
	Get(ctx context.Context, path string, result any) error
}

// HTTPFetcher is the default HTTP implementation of Fetcher
type HTTPFetcher struct {
	endpoint string
}

func NewHTTPFetcher(endpoint string) *HTTPFetcher {
	return &HTTPFetcher{endpoint: endpoint}
}

func (f *HTTPFetcher) Get(ctx context.Context, path string, result any) error {
	req, err := http.NewRequestWithContext(ctx, "GET", f.endpoint+path, nil)
	if err != nil {
		return err
	}

	r, err := httpClient.Do(req)
	if err != nil {
		return err
	}
	defer r.Body.Close()
	bz, err := io.ReadAll(r.Body)
	if err != nil {
		return err
	}
	if r.StatusCode < 200 || r.StatusCode >= 300 {
		return fmt.Errorf("request to %s returned status code %d: body=%s", f.endpoint+path, r.StatusCode, truncateForError(bz))
	}
	if err := json.Unmarshal(bz, result); err != nil {
		return fmt.Errorf("failed to unmarshal response from %s: body=%s: %w", f.endpoint+path, truncateForError(bz), err)
	}
	return nil
}

// truncateForError bounds a response body embedded in an error message.
func truncateForError(body []byte) string {
	const maxLen = 1024
	if len(body) > maxLen {
		return string(body[:maxLen]) + "...(truncated)"
	}
	return string(body)
}

type Client struct {
	fetcher Fetcher
}

func NewClient(endpoint string) Client {
	return Client{fetcher: NewHTTPFetcher(endpoint)}
}

// NewClientWithFetcher creates a Client with a custom Fetcher (useful for testing)
func NewClientWithFetcher(fetcher Fetcher) Client {
	return Client{fetcher: fetcher}
}

func IsSupportedVersion(v string) bool {
	return slices.Contains(SupportedVersions, v)
}

func (cl Client) GetGenesis(ctx context.Context) (*Genesis, error) {
	var res GenesisResponse
	if err := cl.fetcher.Get(ctx, "/eth/v1/beacon/genesis", &res); err != nil {
		return nil, err
	}
	return ToGenesis(res)
}

func (cl Client) GetBlockRoot(ctx context.Context, slot uint64, allowOptimistic bool) (*BlockRootResponse, error) {
	var res BlockRootResponse
	if err := cl.fetcher.Get(ctx, fmt.Sprintf("/eth/v1/beacon/blocks/%v/root", slot), &res); err != nil {
		return nil, err
	}
	if !allowOptimistic && res.ExecutionOptimistic {
		return nil, fmt.Errorf("optimistic execution not allowed")
	}
	return &res, nil
}

func (cl Client) GetFinalityCheckpoints(ctx context.Context) (*StateFinalityCheckpoints, error) {
	var res StateFinalityCheckpointResponse
	if err := cl.fetcher.Get(ctx, "/eth/v1/beacon/states/head/finality_checkpoints", &res); err != nil {
		return nil, err
	}
	return ToStateFinalityCheckpoints(res)
}

func (cl Client) GetBootstrap(ctx context.Context, finalizedRoot []byte) (*LightClientBootstrapResponse, error) {
	if len(finalizedRoot) != 32 {
		return nil, fmt.Errorf("finalizedRoot length must be 32: actual=%v", finalizedRoot)
	}
	var res LightClientBootstrapResponse
	if err := cl.fetcher.Get(ctx, fmt.Sprintf("/eth/v1/beacon/light_client/bootstrap/0x%v", hex.EncodeToString(finalizedRoot[:])), &res); err != nil {
		return nil, err
	}
	if !IsSupportedVersion(res.Version) {
		return nil, fmt.Errorf("unsupported version: %v", res.Version)
	}
	return &res, nil
}

func (cl Client) GetLightClientUpdates(ctx context.Context, period uint64, count uint64) (LightClientUpdatesResponse, error) {
	var res LightClientUpdatesResponse
	if err := cl.fetcher.Get(ctx, fmt.Sprintf("/eth/v1/beacon/light_client/updates?start_period=%v&count=%v", period, count), &res); err != nil {
		return nil, err
	}
	if len(res) < int(count) {
		return nil, fmt.Errorf("unexpected response length: expected=%v actual=%v", count, len(res))
	}
	// Some public nodes ignore the `count` parameter and return more updates
	// than requested; keep only the requested prefix.
	res = res[:count]
	for i := range res {
		if !IsSupportedVersion(res[i].Version) {
			return nil, fmt.Errorf("unsupported version: %v", res[i].Version)
		}
	}
	return res, nil
}

func (cl Client) GetLightClientUpdate(ctx context.Context, period uint64) (*LightClientUpdateResponse, error) {
	res, err := cl.GetLightClientUpdates(ctx, period, 1)
	if err != nil {
		return nil, err
	}
	return &res[0], nil
}

func (cl Client) GetLightClientFinalityUpdate(ctx context.Context) (*LightClientFinalityUpdateResponse, error) {
	var res LightClientFinalityUpdateResponse
	if err := cl.fetcher.Get(ctx, "/eth/v1/beacon/light_client/finality_update", &res); err != nil {
		return nil, err
	}
	if !IsSupportedVersion(res.Version) {
		return nil, fmt.Errorf("unsupported version: %v", res.Version)
	}
	return &res, nil
}
