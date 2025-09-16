# Adding New Chain Support

To add support for a new blockchain (e.g., Aptos), you need to implement the core traits and create the necessary crates following the established patterns.

## Required Traits

### 1. ChainValue
Define blockchain-specific value types with required methods:
- `is_integer()` - Check if value is an integer type
- `is_integer_vector()` - Check if value is a vector of integers
- `contains_integers()` - Recursive check for integers in complex types
- `is_mutable_object()` - Check if value represents a mutable object
- `get_object_id()` - Extract object ID if applicable
- `type_name()` - Get type name for debugging

### 2. ChainAdapter
Core adapter trait defining associated types:
- `Value`: Your ChainValue implementation
- `Address`: Blockchain-specific address type
- `ObjectId`: Object identifier type
- `Object`: Object type for state management
- `ExecutionResult`: Transaction execution result type
- `Mutator`: Mutation strategy implementation

Required methods:
- `resolve_function()` - Parse function metadata from blockchain
- `initialize_parameters()` - Convert string arguments to typed parameters
- `execute()` - Execute transactions on the blockchain
- `extract_violations()` - Detect and extract violations from results
- `extract_object_changes()` - Track object state changes
- `create_mutator()` - Create mutation strategy instance
- Object management methods for caching

### 3. ChainMutationStrategy
Define how to mutate values for fuzzing:
- `mutate()` - Apply mutations to values

## Required Crates

Create the following crates following the Sui implementation pattern:

### 1. {chain}-fuzzer
Main implementation crate containing:
- ChainValue implementation for the blockchain's type system
- ChainAdapter implementation for blockchain operations
- Mutation strategies and orchestrator
- Type parsing and conversion utilities

### 2. {chain}-simulator
Transaction execution environment:
- Transaction simulation without actual blockchain state changes
- Integration with the blockchain's virtual machine
- Support for override objects and deterministic execution

### 3. {chain}-tracer
Violation detection system:
- Integration with the blockchain's VM tracing capabilities
- Detection of specific violation types (shift violations, etc.)
- Real-time violation capture during execution

## Integration Steps

1. **Create the three crates** with proper dependencies
2. **Implement core traits** following the patterns established by sui-fuzzer
3. **Add CLI support** in `bin/fuzzer/src/main.rs` for the new blockchain
4. **Test implementation** using the integration test framework

## Key Considerations

- **Type System**: Handle the blockchain's specific type system and generics
- **Object Model**: Implement proper object caching if the blockchain has stateful objects
- **VM Integration**: Ensure tracer integration works with the blockchain's Move VM implementation
- **Error Handling**: Provide clear error messages for debugging

The framework's trait-based design ensures that once these components are implemented, the core fuzzing logic will work seamlessly with the new blockchain.