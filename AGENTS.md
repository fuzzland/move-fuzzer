# AGENTS.md for move-fuzzer

## Purpose

- These brief instructions help LLM agents navigate working in the `move-fuzzer` repo.
- Humans are always responsible for changes being proposed.
- Agents should not directly make PR and commits without human approval.

## Context

You are in the `move-fuzzer` repo helping work on the implementation of a fuzzer for smart contracts written in the Move language.

- Use the `gh` tool to get information about an issue or PR description in the repo.

- Source files in this repo can be very long.
  Check their size to consider if you really need to read the entire thing.

- If tools such as `rg` (`ripgrep`), `gh`, `jq`, or `pre-commit` are not found, ask the user to install them. ALWAYS prefer using `rg` rather than `find` or `grep`.

## Expanding your knowledge about the project

- ALWAYS load a `.agents/pr-{PR_NUMBER}.md` or `.agents/branch-{branch_name_without_slashes}.md` file when you are told a PR number or when the current git branch is not main.
- Keep the file up to date as an engineering notebook of your learnings and project state as you work on a task and after the programmer accept changes. The goal of our notebook is to make it easier to pick up and resume work later.

## Dependencies

First we need to instead dependencies for aptos, sui, and the move language.
Verify if they exist before installing.

- [aptos cli](https://aptos.dev/build/cli/install-cli/install-cli-linux)
- [sui](https://move-book.com/before-we-begin/install-sui)
- [MVR (move registry)](https://move-book.com/before-we-begin/install-move-registry-cli)

# Building guidelines

## Setting up SUI and MOVE

Use `scripts/setup_sui.sh`:

```bash
./scripts/setup_sui.sh
```

## Building the project

Fetch submodules:

```bash
git submodule init
git submodule update
```

Build the project:

```bash
./scripts/setup_aptos.sh
```

You can use each steps in `./scripts/setup_aptos.sh` to run them individually.

# Coding

## Source code

- `bin/libafl-aptos` contains the code for the fuzzer's command-line interface.
  `bin/libafl-aptos/src/main.rs` parses CLI arguments and runs fuzzing.

- `contracts` contains example Move smart contracts to run and evaluate the fuzzer.
- `crates/aptos-fuzzer` is the main directory for the fuzzer.
  - `executor` runs the smart contracts (Aptos executor and custom state).
  - `feedback.rs` implements feedback/objectives (e.g., abort codes, shift overflow) to mark interesting inputs.
  - `input.rs` defines `AptosFuzzerInput` wrapping `TransactionPayload`.
  - `mutator.rs` mutates selected seeds (entry function args and script args).
  - `observers.rs` monitors for bugs (abort codes, shift overflow flags).
  - `state.rs` manages the fuzzer state and corpus; seeds inputs from ABIs and deploys the module for execution.

## Style and testing

- After introducing modifications and new code,
  try to use `clippy` to and fix all the warnings and errors.
  `cargo clippy`.
- After modifications, use the tests to check integration.
  `cargo test`.
- Explain you code in comments, but make your explanation concise and precise.
- Be consistent with existing nearby code style unless asked to do otherwise.
- NEVER leave trailing whitespace on any line.
- Always format your code: `cargo fmt`.

# Scratch space

NEVER create throw away idea exploration files in the top directory of the repo.
Use a `.agents/sandbox/` directory for those.
They will never be committed.
