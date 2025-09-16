# Architecture

move-fuzzer uses a trait-based architecture to support multiple Move-based blockchains through a unified fuzzing framework.

## Core Components

### fuzzer-core

Generic fuzzing framework providing the foundation for all blockchain implementations.

- **ChainAdapter trait**: Abstraction for blockchain-specific operations
- **ChainValue trait**: Abstraction for blockchain-specific value types
- **CoreFuzzer**: Main fuzzing orchestrator with caching and mutation logic
- **FuzzerConfig**: Unified configuration structure

### sui-fuzzer

Sui-specific implementation of the fuzzing framework.

- **SuiAdapter**: Implements `ChainAdapter` for Sui blockchain operations
- **CloneableValue**: Sui's implementation of `ChainValue` trait
- **SuiMutationOrchestrator**: Handles parameter mutation strategies for Sui

### sui-simulator

Provides isolated transaction execution environment for deterministic Sui fuzzing without affecting actual blockchain state.

### sui-tracer

Detects and captures shift violations in real-time during Sui Move VM execution.

## Key Design Patterns

### Adapter Pattern

The `ChainAdapter` trait enables support for multiple blockchains with unified fuzzing logic. Each blockchain implements this trait to provide:

- Function resolution and parameter initialization
- Transaction execution
- Violation detection
- Object caching for stateful operations

### Strategy Pattern

`ChainMutationStrategy` trait allows different mutation approaches:

- **BoundaryValue**: Tests edge cases and boundary conditions
- **PowerOfTwo**: Tests power-of-two values that often trigger edge cases
- **Random**: Applies uniformly random mutations for broader coverage

### Object Caching

Sophisticated caching system maintains state consistency across fuzzing iterations:

- Tracks mutable shared objects between function calls
- Updates cached objects based on execution results
- Ensures deterministic behavior despite blockchain state changes

## Data Flow

1. **Configuration**: Parse fuzzer configuration and target function
2. **Resolution**: Resolve function metadata and type information
3. **Initialization**: Create initial parameter values from arguments
4. **Fuzzing Loop**:
   - Apply mutations to parameters
   - Execute function via ChainAdapter
   - Detect violations via tracer
   - Update object cache with results
   - Report violations if found
5. **Reporting**: Generate detailed violation reports

This architecture allows adding new blockchain support by implementing the core traits without modifying the fuzzing logic.
