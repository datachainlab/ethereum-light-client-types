# proto-compiler

## Requirement

* libprotoc 33.2

## Usage

```
cargo run -- compile -i <ibc-go-path> -o ../proto/src/prost
```

`<ibc-go-path>` is the path to [cosmos/ibc-go](https://github.com/cosmos/ibc-go) repository.
The version of `ibc-go` must match the version used in `proto/src/IBC_GO_COMMIT`.