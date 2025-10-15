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
