#!/usr/bin/env bash

set -eo pipefail

echo "Generating gogo proto code"

# Run from prover/proto directory
cd prover/proto

# Create symlink to proto definitions if not exists
if [ ! -L "ibc" ] && [ ! -d "ibc" ]; then
    ln -s ../../proto/definitions/ibc ibc
fi

# Generate proto files - output goes to prover/ (..)
buf generate --template buf.gen.gogo.yaml .

# Remove symlink after generation
rm -f ibc

cd ..

# Move generated files to types/ directory
if [ -d "github.com/datachainlab/ethereum-light-client-types/prover/types" ]; then
    mkdir -p types
    cp -r github.com/datachainlab/ethereum-light-client-types/prover/types/* ./types/
    rm -rf github.com
fi

echo "Go proto generation complete"
