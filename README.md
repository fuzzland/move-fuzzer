# move-fuzzer

A unified fuzzing framework for Move-based blockchains. Detects shift violations and other security issues in Move smart contracts through automated fuzzing.

## Features

- **Multi-blockchain support**: Unified framework supporting multiple Move-based blockchains
- **Violation detection**: Real-time detection of arithmetic shift violations and other security issues
- **Intelligent mutation**: Multiple mutation strategies for comprehensive testing coverage
- **State management**: Sophisticated object caching for stateful contract testing
- **Automated testing**: Complete integration test framework

## Quick Start

### Automated Integration Testing

```bash
# Setup venv
python3 -m venv .venv
source .venv/bin/activate
pip install -r scripts/requirements.txt

# Run tests
python scripts/integration_test.py
```

The script will automatically:

1. Build move-fuzzer (cargo build --release)
2. Start a fresh localnet
3. Setup wallet and request tokens from faucet
4. Deploy the shl_demo contract
5. Create necessary test objects (shared structs)
6. Run all fuzzing test cases with 10,000 iterations each
7. Exit with code 0 if all tests pass, 1 if any fail

## Documentation

- **[Architecture](docs/architecture.md)** - System design and core components
- **[Adding New Chain Support](docs/adding-new-chain.md)** - Guide for implementing support for new blockchains
- **[Testing Guide](docs/testing.md)** - Integration testing, manual testing, and scripts documentation

## Supported Chains

- ✅ **Sui** - Full support with shift violation detection
- 🚧 **Aptos** - Planned support

## Project Structure

- `bin/fuzzer` - CLI interface for the fuzzer
- `crates/fuzzer-core` - Generic fuzzing framework with trait abstractions
- `crates/sui-fuzzer` - Sui-specific fuzzer implementation
- `crates/sui-simulator` - Sui transaction simulation environment
- `crates/sui-tracer` - Violation detection for Sui
- `scripts/` - Integration testing and development scripts
- `contracts/` - Demo contracts for testing

## Building

```bash
# Development build
cargo build

# Release build (required for integration tests)
cargo build --release
```
