######## Rust Proto ########

IBC_GO_PATH ?= $HOME/go/src/github.com/cosmos/ibc-go

.PHONY: proto
proto:
	cd proto-compiler && cargo run -- compile -i $(IBC_GO_PATH) -o ../proto/src/prost

######## Go Proto ########

DOCKER := $(shell which docker)

protoVer=0.14.0
protoImageName=ghcr.io/cosmos/proto-builder:$(protoVer)
protoImage=$(DOCKER) run --user 0 --rm -v $(CURDIR):/workspace --workdir /workspace $(protoImageName)

.PHONY: proto-update-deps proto-gen
proto-update-deps:
	@echo "Updating Protobuf dependencies"
	$(DOCKER) run --user 0 --rm -v $(CURDIR)/prover/proto:/workspace --workdir /workspace $(protoImageName) buf mod update

proto-gen:
	@echo "Generating Protobuf files"
	@$(protoImage) sh ./prover/proto/protocgen.sh

######## Lint ########

.PHONY: lint-tools
lint-tools:
	rustup component add rustfmt clippy
	cargo install cargo-machete

.PHONY: fmt
fmt:
	@cargo fmt --all $(CARGO_FMT_OPT)

.PHONY: lint
lint:
	@$(MAKE) CARGO_FMT_OPT=--check fmt
	@cargo clippy --locked --tests $(CARGO_TARGET) -- -D warnings
	@cargo machete
