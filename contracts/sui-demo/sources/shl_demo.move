/// Module: shl_demo
module shl_demo::shl_demo {
    use sui::event;
    use std::bcs;

    // ===== Structs =====

    /// Demo struct for shift left operations 
    public struct DemoStruct has key, store {
        id: UID,
        value: u64,
        shift_amount: u8,
    }

    /// Nested configuration for shift operations
    public struct ShiftConfig has store, copy, drop {
        shift_amount: u8,
        multiplier: u8,  // Additional field to make nesting meaningful
    }

    /// Demo struct with 1-layer nesting for testing nested struct mutations
    public struct NestedDemoStruct has key, store {
        id: UID,
        value: u64,
        config: ShiftConfig,  // Nested struct containing shift_amount
    }

    // ===== Events =====

    /// Event emitted when a shift left operation is performed
    public struct ShiftLeftEvent has copy, drop {
        value: u64,
        shift_amount: u8,
        result: u64,
    }

    /// Generic event for generic shift operations
    public struct GenericShiftLeftEvent<T: copy + drop> has copy, drop {
        value: u64,
        shift_amount: u8,
        result: u64,
        generic_value: T,
    }

    // ===== Error codes =====

    const EInvalidVectorLength: u64 = 2;

    // ===== Entry functions =====

    /// Performs left shift on u64 integers and emits event
    public entry fun integer_shl(value: u64, shift_amount: u8) {
        let result = value << shift_amount;
        
        event::emit(ShiftLeftEvent {
            value,
            shift_amount,
            result,
        });
    }

    /// Generic left shift function using byte extraction from type parameters
    public entry fun generic_shl<T1: copy + drop, T2: copy + drop>(val1: T1, val2: T2) {
        let bytes1 = bcs::to_bytes(&val1);
        let bytes2 = bcs::to_bytes(&val2);
        
        // Extract u64 from first type's bytes (if length >= 8)
        let value = if (vector::length(&bytes1) >= 8) {
            let mut result = 0u64;
            let mut i = 0;
            while (i < 8) {
                let byte_val = *vector::borrow(&bytes1, i) as u64;
                let shift_bits = (i * 8) as u8;
                result = result | (byte_val << shift_bits);
                i = i + 1;
            };
            result
        } else { 0u64 };
        
        // Extract u8 from second type's bytes (if length > 0)
        let shift_amount = if (vector::length(&bytes2) > 0) {
            *vector::borrow(&bytes2, 0)
        } else { 0u8 };
        
        let result = value << shift_amount;
        
        event::emit(ShiftLeftEvent {
            value,
            shift_amount,
            result,
        });
    }

    /// Vector-based shift operation: uses values[0] as value, values[1] as shift_amount
    public entry fun vector_shl(values: vector<u32>) {
        assert!(vector::length(&values) >= 2, EInvalidVectorLength);
        
        let value = *vector::borrow(&values, 0);
        let shift_amount = *vector::borrow(&values, 1);
        
        // Convert to u8 for shift amount (assuming it fits)
        let shift_u8 = (shift_amount as u8);
        let result = (value as u64) << shift_u8;
        
        event::emit(ShiftLeftEvent {
            value: (value as u64),
            shift_amount: shift_u8,
            result,
        });
    }

    /// Shift operation using owned DemoStruct - takes object by ID
    public entry fun owned_struct_shl(owned: DemoStruct, _ctx: &mut TxContext) {
        let result = owned.value << owned.shift_amount;
        
        event::emit(ShiftLeftEvent {
            value: owned.value,
            shift_amount: owned.shift_amount,
            result,
        });
        
        // Delete the owned object after use
        let DemoStruct { id, value: _, shift_amount: _ } = owned;
        object::delete(id);
    }

    /// Shift operation using immutable shared DemoStruct reference
    public entry fun immutable_shared_struct_shl(shared: &DemoStruct) {
        let result = shared.value << shared.shift_amount;
        
        event::emit(ShiftLeftEvent {
            value: shared.value,
            shift_amount: shared.shift_amount,
            result,
        });
    }

    /// Shift operation using mutable shared DemoStruct reference
    public entry fun mutable_shared_struct_shl(mut_shared: &mut DemoStruct) {
        let result = mut_shared.value << mut_shared.shift_amount;
        
        event::emit(ShiftLeftEvent {
            value: mut_shared.value,
            shift_amount: mut_shared.shift_amount,
            result,
        });
        
        mut_shared.value = result;
        mut_shared.shift_amount = mut_shared.shift_amount + 1;
    }

    // ===== Nested Struct Entry Functions =====

    /// Shift operation using owned NestedDemoStruct - takes nested object by value
    public entry fun nested_owned_struct_shl(owned: NestedDemoStruct, _ctx: &mut TxContext) {
        let result = owned.value << owned.config.shift_amount;
        
        event::emit(ShiftLeftEvent {
            value: owned.value,
            shift_amount: owned.config.shift_amount,
            result,
        });
        
        // Delete the owned nested object after use
        let NestedDemoStruct { id, value: _, config: _ } = owned;
        object::delete(id);
    }

    /// Shift operation using immutable shared NestedDemoStruct reference
    public entry fun nested_immutable_shared_struct_shl(shared: &NestedDemoStruct) {
        let result = shared.value << shared.config.shift_amount;
        
        event::emit(ShiftLeftEvent {
            value: shared.value,
            shift_amount: shared.config.shift_amount,
            result,
        });
    }

    /// Shift operation using mutable shared NestedDemoStruct reference
    public entry fun nested_mutable_shared_struct_shl(mut_shared: &mut NestedDemoStruct) {
        let result = mut_shared.value << mut_shared.config.shift_amount;
        
        event::emit(ShiftLeftEvent {
            value: mut_shared.value,
            shift_amount: mut_shared.config.shift_amount,
            result,
        });
        
        mut_shared.value = result;
        mut_shared.config.shift_amount = mut_shared.config.shift_amount + 1;
        mut_shared.config.multiplier = mut_shared.config.multiplier + 1;
    }

    // ===== Constructor and utility functions =====

    /// Create a new DemoStruct
    public fun new_demo_struct(value: u64, shift_amount: u8, ctx: &mut TxContext): DemoStruct {
        DemoStruct {
            id: object::new(ctx),
            value,
            shift_amount,
        }
    }

    /// Create and share a DemoStruct
    public entry fun create_shared_demo_struct(value: u64, shift_amount: u8, ctx: &mut TxContext) {
        let demo_struct = new_demo_struct(value, shift_amount, ctx);
        transfer::share_object(demo_struct);
    }

    /// Create and transfer a DemoStruct to sender
    public entry fun create_owned_demo_struct(value: u64, shift_amount: u8, ctx: &mut TxContext) {
        let demo_struct = new_demo_struct(value, shift_amount, ctx);
        transfer::transfer(demo_struct, tx_context::sender(ctx));
    }

    /// Create a new NestedDemoStruct with nested configuration
    public fun new_nested_demo_struct(value: u64, shift_amount: u8, multiplier: u8, ctx: &mut TxContext): NestedDemoStruct {
        NestedDemoStruct {
            id: object::new(ctx),
            value,
            config: ShiftConfig {
                shift_amount,
                multiplier,
            },
        }
    }

    /// Create and share a NestedDemoStruct
    public entry fun create_shared_nested_demo_struct(value: u64, shift_amount: u8, multiplier: u8, ctx: &mut TxContext) {
        let nested_demo_struct = new_nested_demo_struct(value, shift_amount, multiplier, ctx);
        transfer::share_object(nested_demo_struct);
    }

    /// Create and transfer a NestedDemoStruct to sender
    public entry fun create_owned_nested_demo_struct(value: u64, shift_amount: u8, multiplier: u8, ctx: &mut TxContext) {
        let nested_demo_struct = new_nested_demo_struct(value, shift_amount, multiplier, ctx);
        transfer::transfer(nested_demo_struct, tx_context::sender(ctx));
    }

    // ===== Getters =====

    /// Get the value from DemoStruct
    public fun get_value(demo_struct: &DemoStruct): u64 {
        demo_struct.value
    }

    /// Get the shift_amount from DemoStruct
    public fun get_shift_amount(demo_struct: &DemoStruct): u8 {
        demo_struct.shift_amount
    }

    /// Set the value in DemoStruct (for mutable references)
    public fun set_value(demo_struct: &mut DemoStruct, new_value: u64) {
        demo_struct.value = new_value;
    }

    /// Set the shift_amount in DemoStruct (for mutable references)
    public fun set_shift_amount(demo_struct: &mut DemoStruct, new_shift_amount: u8) {
        demo_struct.shift_amount = new_shift_amount;
    }

    // ===== NestedDemoStruct Getters and Setters =====

    /// Get the value from NestedDemoStruct
    public fun get_nested_value(nested_struct: &NestedDemoStruct): u64 {
        nested_struct.value
    }

    /// Get the shift_amount from nested config
    public fun get_nested_shift_amount(nested_struct: &NestedDemoStruct): u8 {
        nested_struct.config.shift_amount
    }

    /// Get the multiplier from nested config
    public fun get_nested_multiplier(nested_struct: &NestedDemoStruct): u8 {
        nested_struct.config.multiplier
    }

    /// Set the value in NestedDemoStruct (for mutable references)
    public fun set_nested_value(nested_struct: &mut NestedDemoStruct, new_value: u64) {
        nested_struct.value = new_value;
    }

    /// Set the shift_amount in nested config (for mutable references)
    public fun set_nested_shift_amount(nested_struct: &mut NestedDemoStruct, new_shift_amount: u8) {
        nested_struct.config.shift_amount = new_shift_amount;
    }

    /// Set the multiplier in nested config (for mutable references)
    public fun set_nested_multiplier(nested_struct: &mut NestedDemoStruct, new_multiplier: u8) {
        nested_struct.config.multiplier = new_multiplier;
    }
}


