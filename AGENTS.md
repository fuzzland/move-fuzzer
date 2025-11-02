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

- `bin/libafl-aptos` — CLI for the fuzzer. `bin/libafl-aptos/src/main.rs` parses args and orchestrates fuzzing.
- `crates/aptos-fuzzer` — Core fuzzing logic for Aptos:
  - `executor/` — Executes transactions on an in-memory Aptos VM. Publishes a module and records PCs/flags.
  - `feedback.rs` — Objectives for interesting inputs (abort codes, shift-overflow conditions).
  - `input.rs` — Wraps `TransactionPayload` as `AptosFuzzerInput`.
  - `mutator.rs` — Mutates entry function/script arguments.
  - `observers.rs` — Observers for abort codes and shift overflow.
  - `state.rs` — Manages corpus/solutions, cumulative coverage, seed input generation from ABIs.
- `contracts/` — Example Move packages (`aptos-demo`, `fuzzing-demo`).
- `external/aptos-core` — Submodule used for Aptos VM and Move crates.
- `scripts/` — Helper scripts: `setup_aptos.sh` (compile + run), `setup_sui.sh` (CI Sui bootstrap), `integration_test.py` (Sui localnet helper; not used in active Aptos path).

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

- Aptos CLI (`aptos` in PATH) — required by `scripts/setup_aptos.sh` to compile Move packages.
- zstd — required only if using `scripts/setup_sui.sh` to decompress `sui-linux.zst` in CI.
- Python 3 + `pip install -r scripts/requirements.txt` — only if you need the Sui integration script; not required for Aptos fuzzing.

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
