# AGENTS.md for move-fuzzer

## Purpose

- Help LLM agents work effectively in the `move-fuzzer` repo.
- Humans review and approve all proposed changes; do not push commits/PRs directly.

## Scope and Responsibilities

- Primary target: Aptos Move fuzzing with LibAFL. Sui code in `crates/sui-old-unused` is historical and not part of the active build unless explicitly requested.
- Keep changes minimal and aligned with current architecture. Avoid refactors unless asked.
- Prefer surgical edits with clear rationale and code comments where non-obvious.

## Quick Start

Prerequisites (verify before installing):

- Rust toolchain pinned by `rust-toolchain.toml` (nightly). Use the pinned toolchain automatically via rustup.
- Aptos CLI: https://aptos.dev/build/cli/install-cli/install-cli-linux
- Recommended tools: `rg` (ripgrep), `gh`, `jq`. Only use `rg` over `grep/find` for speed.

Fetch submodules (Aptos core is a submodule):

```bash
git submodule init
git submodule update
```

Build the fuzzer binary in release:

```bash
cargo build --release --bin libafl-aptos
```

Run the demo end-to-end (compiles a contract and fuzzes it):

```bash
./scripts/setup_aptos.sh -c fuzzing-demo -t 30
```

Skip rebuilds (reuse an existing build):

```bash
./scripts/setup_aptos.sh --no-build -c fuzzing-demo -t 30
```

Contractc (`-c`) (fuzz targets) are located in `./contracts`, with the following options:

- aptos-demo
- fuzzing-demo
- sui-demo

Manual run with a custom module/ABI:

1. Compile your package: `aptos move compile --included-artifacts all`
2. Identify one `.mv` under `build/<pkg>/bytecode_modules/` and the corresponding `abis/` directory.
3. Run the fuzzer:

```bash
target/release/libafl-aptos --module-path <path/to/module.mv> --abi-path <path/to/abis> --timeout 60
```

## Project Layout

- `bin/libafl-aptos`: CLI for the fuzzer. `bin/libafl-aptos/src/main.rs` parses args and orchestrates fuzzing.
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

### Details

- CLI binary (`bin/libafl-aptos`)

  - `bin/libafl-aptos/src/main.rs`: Entry point configuring LibAFL components and running the fuzzing loop.
    - Constructs `AptosMoveExecutor` (LibAFL Executor) that executes transactions against an in-memory Aptos VM.
    - Feedback: `MaxMapFeedback` over the PC hitcount observer (coverage); objective: `EagerOrFeedback(ShiftOverflowObjective, AbortCodeObjective)`.
    - Scheduler/Stages: `QueueScheduler` + single `StdMutationalStage(AptosFuzzerMutator)`.
    - Events/Monitor: `SimpleEventManager(NopMonitor)` prints minimal stats.
    - Seeds: drains initial inputs from state (ABI-derived) and re-inserts via `fuzzer.add_input` to emit proper events.
    - Controls timeout and graceful shutdown (Ctrl+C) and prints periodic stats.
  - `bin/libafl-aptos/src/utils.rs`: `print_fuzzer_stats` helper: compact, AFL-like runtime/corpus/coverage reporting.

- Core library (`crates/aptos-fuzzer`)

  - `src/lib.rs`: Export modules.
  - `src/input.rs`: `AptosFuzzerInput` wraps `aptos_types::transaction::TransactionPayload` with getters/mut-getters as input generation for fuzzing.
  - `src/mutator.rs`: `AptosFuzzerMutator` (LibAFL Mutator) for transaction arguments.
    - Entry functions: treats each BCS-encoded argument blob as a byte vector, resizes/fills with PRNG bytes.
    - Scripts: mutates `TransactionArgument` variants with random values/lengths.
    - Returns `MutationResult::Mutated/Skipped` as per LibAFL contract; `post_exec` is a no-op hook.
  - `src/observers.rs`: monitor errors
    - `AbortCodeObserver`: Records the last Move abort code (if any) for a run.
    - `ShiftOverflowObserver`: Boolean flag set when any left-shift lost high bits.
    - Both reset state in `pre_exec` as required by Observer contract.
  - `src/feedback.rs`: Objectives deciding if an input is "interesting", to keep in queue.
    - `AbortCodeObjective`: Marks inputs interesting on VM crashes or Move aborts; optionally filter by target codes. Deduplicates by execution-path hash.
    - `ShiftOverflowObjective`: Marks inputs interesting when `ShiftOverflowObserver` flagged a loss. Deduplicates by execution-path hash.
    - Both implement `StateInitializer` to allow state bootstrapping if needed.
  - `src/state.rs`: `AptosFuzzerState` (LibAFL State) combining corpus, solutions, RNG, metadata, and Aptos-specific fields.
    - Seeds initial inputs from ABIs: scans `--abi-path` for `EntryABI`/`EntryFunctionABI`, builds default-arg payloads (skips unsupported types), and inserts into corpus.
    - Tracks cumulative coverage (`MAP_SIZE = 2^16`) for stats; observer map resets each exec.
    - Records execution paths (Vec<u32> of PCs) and computes a stable FNV-1a `u64` path ID; deduplicates interesting inputs by path ID.
    - Implements LibAFL traits: `HasCorpus`, `HasSolutions`, `HasExecutions`, `HasRand`, `HasMetadata`, `HasCurrentCorpusId`, etc.
  - `src/executor/aptos_move_executor.rs`: `AptosMoveExecutor` (LibAFL Executor) wrapping Aptos VM execution.
    - Runs `execute_user_payload_no_checking` on an in-memory state; supports `EntryFunction`/`Script` payloads.
    - Computes AFL-like edge coverage: builds a per-function base ID, XOR with PC values and previous location to update a 64K hitcount map (like AFL’s edge map).
    - Updates cumulative coverage in state; sets observers: abort code and shift-overflow flag; classifies VM invariant violation/panic as `ExitKind::Crash` (objective).
    - Exposes `pc_observer()` for `MaxMapFeedback` and tracks `total_instructions_executed`.
  - `src/executor/aptos_custom_state.rs`: Minimal, in-memory Aptos state/view.
    - Implements the necessary Aptos traits (e.g., `ModuleStorage`, `ResourceResolver`, table/resource group views) to run transactions in-process.
    - Stores modules and resources in hash maps; exposes config/state getters; keeps a `RuntimeEnvironment` for the VM.
  - `src/executor/custom_state_view.rs`: Thin adapter implementing `TStateView` for the in-memory state so VM loaders/caches can work.
  - `src/executor/types.rs`: `TransactionResult` struct mirroring executed transaction result fields.

- Example contracts (`contracts/`)

  - `contracts/aptos-demo`: Simple demos focusing on shifts, invariants, structs; used for quick sanity.
  - `contracts/fuzzing-demo`: Larger suite of entry functions targeting control-flow diversity and typical bug patterns (branches, state machines, overflows, invariants).
  - Build outputs: `build/<pkg>/bytecode_modules/*.mv` and `build/<pkg>/abis/*.abi` are consumed by the fuzzer.

- External submodules

  - `external/aptos-core`: Aptos VM, Move crates, frameworks. This is required to build the executor and link Move runtime.

- Scripts (`scripts/`)
  - `scripts/setup_aptos.sh`: Convenience wrapper: builds the binary, compiles a chosen contract (`-c`), auto-detects `--module-path` and `--abi-path`, then runs fuzzing with `--timeout`.
  - `scripts/setup_sui.sh`: CI helper to unpack a Sui binary and create a basic local client config (not used by Aptos fuzzer).
  - `scripts/integration_test.py`: Rich-console Sui localnet runner and smoke tester (legacy/optional; not in Aptos workflow).

## Workflows

- Use `scripts/setup_aptos.sh` to:

  - Build `libafl-aptos` (unless `--no-build` is set).
  - Compile a contract (`aptos move compile --included-artifacts all`).
  - Auto-detect `--module-path` and `--abi-path` and run the fuzzer with `--timeout`.
  - Contracts available: `contracts/aptos-demo`, `contracts/fuzzing-demo` via `-c` flag.

- Manual CLI flags (`bin/libafl-aptos/src/main.rs`):

  - `--module-path <MODULE_PATH>`: path to compiled `.mv` module to publish before fuzzing.
  - `--abi-path <ABI_PATH>`: file or directory of ABIs used to seed initial inputs.
  - `--timeout <SECONDS>`: stop after N seconds (0 means run until interrupted).

- Large files: Many sources are long. Use targeted reads to save time:
  - `rg` for search, `sed -n 'START,ENDp'` for ranges, avoid reading entire files unless necessary.

## Tools and Conventions

- Prefer `rg` over `grep/find` for code and file searches. Ask the user to install `rg`, `gh`, or `jq` if missing.
- Use `gh` to fetch issue/PR descriptions when a PR number is provided.
- Maintain a notebook under `.agents/`:
  - Use `.agents/pr-{PR_NUMBER}.md` for PR work.
  - Use `.agents/branch-{branch_name_without_slashes}.md` when working off-main.
  - Keep notes current so future work can resume quickly.

## Coding Standards

- Toolchain: respect `rust-toolchain.toml` (nightly). Do not change toolchain version unless requested.
- Formatting: run `cargo fmt` before sharing patches.
- Linting: run `cargo clippy -- -D warnings` and address all warnings.
- Tests: run `cargo test` if tests exist; otherwise validate via `scripts/setup_aptos.sh` on the demo packages.
- Comments: explain non-obvious logic concisely. Avoid verbose/chattery comments.
- Style: match nearby code patterns. Keep changes minimal and focused.
- Whitespace: never leave trailing whitespace on any line.

## Dependencies

Verify before installing:

- Aptos CLI (`aptos` in PATH): required by `scripts/setup_aptos.sh` to compile Move packages.
- zstd: required only if using `scripts/setup_sui.sh` to decompress `sui-linux.zst` in CI.
- Python 3 + `pip install -r scripts/requirements.txt`: only if you need the Sui integration script; not required for Aptos fuzzing.

## Sui Notes (optional)

- `scripts/setup_sui.sh` prepares a local Sui client and binary for CI; it is not part of the Aptos fuzzing workflow.
- The `crates/sui-old-unused` tree is legacy and excluded from the active workspace. Do not modify unless a task explicitly requires it.

## Troubleshooting

- `aptos` not found: install the Aptos CLI and ensure it's in PATH. The Aptos script prints an install hint when missing.
- Submodules missing: run `git submodule init && git submodule update`.
- Can't find module/ABI: ensure `aptos move compile --included-artifacts all` was run in the contract directory. Point `--module-path` at a `.mv` under `build/<pkg>/bytecode_modules/` and `--abi-path` at the `abis/` directory.
- Slow runs: use release builds and `--no-build` when iterating. Keep timeboxed runs with `--timeout`.

## Scratch Space

- Do not create ad-hoc files at repo root. Use `.agents/sandbox/` for throwaway exploration that will not be committed.

## PR/Review Policy

- Propose changes via patches for human review. Do not create branches, commits, or PRs without explicit approval.
