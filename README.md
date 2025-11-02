# Move fuzzer (rename it later)

This is a WIP fuzzer for move smart contracts. It is built using [LibAFL](https://github.com/AFLplusplus/libafl)

## We're hiring!

https://x.com/shoucccc/status/1973125981222412314

Will complete README later

## Contributing

The fuzzer itself is at very early stage. Feel free to grab an issue and start working on it.

Make sure you ran `cargo clippy -- -D warnings` before submitting a PR.

AI generated code is allowed but please review it first. e.g. delete chatty comments. Make sure to manage your agent well and shitty code won't be reviewed.

# Run Demonstration

```sh
./scripts/setup_aptos.sh -c fuzzing-demo -t 30
```

## Skip rebuilds

You can build the project in release mode by

```sh
cargo build --release --bin libafl-aptos
```

By default, the script builds the fuzzer binary.
To reuse an existing build, pass `--no-build`.
The `--no-build` option fist check in `target/release/libafl-aptos`,
if it does not exist,
it will use `target/debug/libafl-aptos`.

```sh
./scripts/setup_aptos.sh --no-build -c fuzzing-demo -t 30
```
