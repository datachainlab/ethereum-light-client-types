package client

import (
	"context"
	"encoding/hex"
	"errors"
	"os"
	"testing"
	"time"

	"github.com/datachainlab/ethereum-light-client-types/e2e/client/pb"
	"github.com/ethereum/go-ethereum/common"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

func getenv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

// TestVerifyUpdate builds an update from live endpoints and verifies it via
// the Rust e2e server.
func TestVerifyUpdate(t *testing.T) {
	beaconEndpoint := os.Getenv("BEACON_ENDPOINT")
	executionEndpoint := os.Getenv("EXECUTION_ENDPOINT")
	if beaconEndpoint == "" || executionEndpoint == "" {
		t.Fatal("BEACON_ENDPOINT and EXECUTION_ENDPOINT must be set")
	}
	serverAddr := getenv("E2E_SERVER_ADDR", "localhost:50151")
	ibcAddress := common.HexToAddress(os.Getenv("IBC_ADDRESS")) // zero address if unset
	ibcClientID := os.Getenv("IBC_CLIENT_ID")                   // membership verification if set

	// Right after a sync committee period boundary the retry loop below can
	// wait for finality to advance into the new period (up to ~13 minutes on
	// mainnet), so the deadline must comfortably exceed that.
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Minute)
	defer cancel()

	// wait for finality to advance past the period boundary
	var req *pb.VerifyUpdateRequest
	for {
		var err error
		req, err = BuildVerifyUpdateRequest(ctx, beaconEndpoint, executionEndpoint, ibcAddress, ibcClientID)
		if err == nil {
			break
		}
		if !errors.Is(err, ErrFinalityNotAdvanced) {
			t.Fatalf("failed to build request: %v", err)
		}
		t.Logf("waiting for finality to advance: %v", err)
		select {
		case <-ctx.Done():
			t.Fatalf("timed out waiting for finality to advance: %v", err)
		case <-time.After(30 * time.Second):
		}
	}
	t.Logf("request: trusted_slot=%d header_timestamp=%d sync_committee_size=%d membership_path=%q",
		req.TrustedSyncCommittee.TrustedHeight.RevisionHeight, req.HeaderTimestampSecs, req.SyncCommitteeSize, req.MembershipPath)

	conn, err := grpc.NewClient(serverAddr, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		t.Fatalf("failed to connect to e2e server at %s: %v", serverAddr, err)
	}
	defer conn.Close()

	res, err := pb.NewVerifierClient(conn).VerifyUpdate(ctx, req)
	if err != nil {
		t.Fatalf("VerifyUpdate failed: %v", err)
	}
	t.Logf("response: current_committee=%s", hex.EncodeToString(res.CurrentSyncCommittee))

	// a same-period update must keep the sync committees unchanged
	if hex.EncodeToString(res.CurrentSyncCommittee) != hex.EncodeToString(req.TrustedCurrentSyncCommittee) {
		t.Errorf("unexpected current sync committee: %x", res.CurrentSyncCommittee)
	}
	if hex.EncodeToString(res.NextSyncCommittee) != hex.EncodeToString(req.TrustedNextSyncCommittee) {
		t.Errorf("unexpected next sync committee: %x", res.NextSyncCommittee)
	}
}
