# e2e test harness.
#
#   make server   # start the Rust verifier gRPC server
#   make test     # build an update from BEACON_ENDPOINT/EXECUTION_ENDPOINT
#                 # and verify it via the server
#
# Environment variables for `make test`:
#   BEACON_ENDPOINT     (required) beacon API endpoint, e.g. http://localhost:60043
#   EXECUTION_ENDPOINT  (required) execution RPC endpoint, e.g. http://localhost:60038
#   IBC_ADDRESS         (optional) account whose storage root is verified (default: zero address)
#   IBC_CLIENT_ID       (optional) verify membership of "clients/<id>/clientState" on IBC_ADDRESS
#   E2E_SERVER_ADDR     (optional) verifier server address (default: localhost:50151)

.PHONY: server test proto-gen

# The Rust server build needs the vendored proto import closure.
proto/third_party:
	$(MAKE) proto-gen

server: proto/third_party
	cd server && cargo run --release

test:
	cd client && go test -v -count=1 -timeout 1500s ./...

# Regenerate the Go gRPC stubs (same gogo pipeline as prover/types).
DOCKER := $(shell which docker)
protoVer=0.14.0
protoImageName=ghcr.io/cosmos/proto-builder:$(protoVer)

proto-gen:
	$(DOCKER) run --user 0 --rm -v $(CURDIR)/..:/workspace --workdir /workspace $(protoImageName) sh ./e2e/proto/protocgen.sh
