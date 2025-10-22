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

```
./scripts/setup_aptos.sh -c fuzzing-demo -t 30