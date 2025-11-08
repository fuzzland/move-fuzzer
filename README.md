# Move fuzzer (rename it later)

This is a WIP fuzzer for move smart contracts. It is built using [LibAFL](https://github.com/AFLplusplus/libafl)

## We're hiring!

https://x.com/shoucccc/status/1973125981222412314

## Background

Despite these safeguards, Move contracts still inhabit a large state space that is hard to reason about manually or exhaustively test upfront.
This fuzzer focuses on bugs in execution semantics
that are hard to prevent by linear type in Move and compile-time static checking.
For example, those depend on runtime resources, module publishing order, abort codes, and arithmetic corner cases.

## Contributing

The fuzzer itself is at a very early stage. Feel free to grab an issue and start working on it.

**Guidelines**:

1. Make sure you ran `cargo clippy -- -D warnings` and fix the warnings before submitting a PR.
2. AI generated code is allowed but please review it first.
   E.g. delete chatty comments and unnecessary code.
   Make sure to manage your agent well and shitty code won't be reviewed.
3. Use `cargo fmt` to format your code before submitting a PR. Consistent formatting is helpful in reducing diff noise.

## Usage

### Prerequisites

1. [Rust](https://www.rust-lang.org/tools/install)
2. [Aptos CLI](https://aptos.dev/build/cli)

### Quick start on Aptos

```sh
./scripts/setup_aptos.sh -c fuzzing-demo -t 30
```

### Building

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

### Running

You can run the fuzzer binary directly after building:

1. Compile the target contract from `contracts/<name>`: `aptos move compile --included-artifacts all`, which produces:
   - Move bytecode modules at `contracts/<name>/build/<pkg>/bytecode_modules/<Module>.mv`
   - Move IR file at `contracts/<name>/mir.json`
2. Run the fuzzer:

```sh
./target/release/libafl-aptos --mir-path <MIR_PATH> --module-path <MODULE_PATH> --timeout <TIMEOUT>"
target/release/libafl-aptos \
  --module-path contracts/<name>/build/<pkg>/bytecode_modules/<Module>.mv \
  --mir-path contracts/<name>/mir.json \
  --timeout 60
```

- `--module-path` publishes the compiled module into the in-memory Aptos VM before fuzzing.
- `--mir-path` points at the JSON-derived corpus seed (generated from the contract’s ABIs/MIR description).
- `--timeout` stops the fuzzing loop after the requested number of seconds (0 keeps it running until Ctrl+C).

### Project Layout

- `bin/libafl-aptos`: CLI for the fuzzer. `bin/libafl-aptos/src/main.rs` parses args for the fuzzer.
- `crates/aptos-fuzzer`: Core fuzzing logic for Aptos:
  - `executor/`: Executes transactions on an in-memory Aptos VM. Publishes a module and records PCs/flags.
  - `feedback.rs`: Objectives for interesting inputs (abort codes, shift-overflow conditions).
  - `input.rs`: Wraps `TransactionPayload` as `AptosFuzzerInput`.
  - `mutator.rs`: Mutates entry function/script arguments.
  - `observers.rs`: Observers for abort codes and shift overflow.
  - `state.rs`: Manages corpus/solutions, cumulative coverage, seed input generation from ABIs.
- `contracts/`: Example Move packages (`aptos-demo`, `fuzzing-demo`).
- `external/aptos-core`: Submodule used for Aptos VM and Move crates.
- `scripts/`: Helper scripts: `setup_aptos.sh` (compile + run), `setup_sui.sh` (CI Sui bootstrap), `integration_test.py` (Sui localnet helper; not used in active Aptos path).
