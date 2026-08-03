#!/usr/bin/env bash

set -eo pipefail

echo "Generating gogo proto code for e2e"

# Run from e2e/proto directory (the repository root is the docker workdir)
cd e2e/proto

# Create symlink to proto definitions if not exists
if [ ! -L "ibc" ] && [ ! -d "ibc" ]; then
    ln -s ../../proto/definitions/ibc ibc
fi

# refresh the vendored import closure used by the Rust server's build.rs
rm -rf third_party
buf export . -o third_party
rm -rf third_party/ibc/lightclients third_party/e2e

# Generate proto files - output goes to e2e/ (..)
buf generate --template buf.gen.gogo.yaml .

# Remove symlink after generation
rm -f ibc

cd ..

# Move generated files to client/pb/
if [ -d "github.com/datachainlab/ethereum-light-client-types/e2e/client/pb" ]; then
    mkdir -p client/pb
    rm -f client/pb/*.pb.go
    cp -r github.com/datachainlab/ethereum-light-client-types/e2e/client/pb/* ./client/pb/
    rm -rf github.com
fi

echo "e2e proto generation complete"
