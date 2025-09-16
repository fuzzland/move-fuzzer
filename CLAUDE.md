# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

move-fuzzer is a unified fuzzing framework for Move-based blockchains, currently supporting Sui with planned Aptos support. It detects shift violations and other security issues in Move smart contracts through automated fuzzing.

## Commands

### Building
```bash
# Build in release mode (required for integration tests)
cargo build --release

# Build for development
cargo build
```

### Running the Fuzzer
```bash
# Basic Sui fuzzing command
cargo run -p fuzzer -- sui \
    --rpc-url http://localhost:9000 \
    --package <PACKAGE_ID> \
    --module <MODULE_NAME> \
    --function <FUNCTION_NAME> \
    --args <ARGS>

# With debug logging
RUST_LOG=debug cargo run -p fuzzer -- sui [options]

# With type arguments and custom iterations
cargo run -p fuzzer -- sui \
    --rpc-url http://localhost:9000 \
    --package <PACKAGE_ID> \
    --module <MODULE_NAME> \
    --function <FUNCTION_NAME> \
    --type-args u64 u8 \
    --args 5 2 \
    --iterations 10000
```

### Testing
```bash
# Automated integration testing
python3 -m venv .venv
source .venv/bin/activate
pip install -r scripts/requirements.txt
python scripts/integration_test.py

# Run tests for specific crates
cargo test -p fuzzer-core
cargo test -p sui-fuzzer
```

### Setting up Sui Localnet
```bash
# Start localnet (requires custom Sui build with tracing)
cd scripts
RUST_LOG="off,sui_node=info" ./sui start --with-faucet --force-regenesis

# Setup client and deploy demo contract
scripts/sui client new-address ed25519 move-fuzzer
scripts/sui client switch --address move-fuzzer
scripts/sui client new-env --alias local --rpc http://127.0.0.1:9000
scripts/sui client switch --env local
scripts/sui client faucet

# Deploy contract
cd contracts/sui-demo
../../scripts/sui move build
../../scripts/sui client publish --gas-budget 100000000
```

## Architecture

### Core Components

**fuzzer-core**: Generic fuzzing framework with trait-based abstraction
- `ChainAdapter` trait: Blockchain-specific implementations
- `ChainValue` trait: Blockchain-specific value types
- `CoreFuzzer`: Main fuzzing orchestrator with caching and mutation

**sui-fuzzer**: Sui-specific implementation
- `SuiAdapter`: Implements `ChainAdapter` for Sui blockchain
- `CloneableValue`: Sui's implementation of `ChainValue`
- `SuiMutationOrchestrator`: Handles parameter mutation strategies

**sui-simulator**: Execution environment
- `DBSimulator`: Database-backed Sui transaction simulation
- `RPCSimulator`: RPC-backed simulation (alternative implementation)

**sui-tracer**: Violation detection
- `ShiftViolationTracer`: Detects arithmetic shift violations during execution
- Integrates with Sui's Move VM tracing infrastructure

### Key Design Patterns

**Adapter Pattern**: `ChainAdapter` trait allows support for multiple blockchains (Sui, planned Aptos) with unified fuzzing logic.

**Strategy Pattern**: `ChainMutationStrategy` trait enables different mutation approaches (boundary values, power of two, random).

**Object Caching**: Sophisticated caching system for mutable shared objects to maintain state consistency across fuzzing iterations.

**Type Resolution**: Dynamic Move type system integration with support for generics and type parameters.

## Dependencies

This project uses a forked version of Sui from `https://github.com/fuzzland/sui.git` (branch: `mainnet-v1.55.0-dryrun-with-tracer`) that includes tracing support for violation detection.

The workspace includes both Sui-specific dependencies (prefixed with `sui-`) and planned Aptos dependencies.

## Development Workflow

1. Make code changes
2. Build with `cargo build`
3. Test with demo contract using manual commands or `python scripts/integration_test.py`
4. For new violation types, extend the tracer in `sui-tracer/src/`
5. For new blockchain support, implement `ChainAdapter` trait

## Important Implementation Details

- Object IDs for shared objects must be carefully managed in the cache
- Type arguments are resolved at runtime for generic functions
- The fuzzer uses Sui's programmable transaction builder for execution
- Shift violations are detected via Move VM tracing during execution
- All simulations run with gas coins and override objects for deterministic execution