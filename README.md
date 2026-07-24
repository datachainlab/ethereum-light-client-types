# ethereum-light-client-types

Shared type definitions for Ethereum Light Client implementations.

## Rust

Provides common types, traits, and protobuf definitions used by [ethereum-elc](https://github.com/datachainlab/ethereum-elc) and [optimism-elc](https://github.com/datachainlab/optimism-elc).

### Crates

| Crate | Description |
|-------|-------------|
| `ethereum-light-client-types` | Core types, traits, and error definitions |
| `ethereum-light-client-proto` | Protobuf definitions and generated Rust code |

### Features

- `no_std` compatible
- Common trait definitions for `ClientState` and `ConsensusState`
- Membership proof verification utilities

### Usage

```toml
[dependencies]
ethereum-light-client-types = { git = "https://github.com/datachainlab/ethereum-light-client-types" }
ethereum-light-client-proto = { git = "https://github.com/datachainlab/ethereum-light-client-types" }
```

---

## Go (prover)

Provides beacon and execution layer API clients for building light client proofs.

### Packages

| Package | Description |
|---------|-------------|
| `prover/beacon` | Beacon chain API client |
| `prover/execution` | Execution layer API client |
| `prover/relay` | Relay utilities (slot calculation, merkle proofs, etc.) |
| `prover/types` | Generated protobuf types |

### Features

- Beacon and execution layer API clients
- SSZ merkle proof generation
- Slot/epoch/period calculations

### Usage

```bash
go get github.com/datachainlab/ethereum-light-client-types
```
