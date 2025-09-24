module aptos_demo::shl_demo {
    use std::bcs;
    use std::vector;
    use std::signer;
    use aptos_framework::event;

    // ===== Event structs and store =====

    #[event]
    public struct ShiftLeftEvent has copy, drop, store {
        value: u64,
        shift_amount: u8,
        result: u64,
    }

    #[event]
    public struct GenericShiftLeftEvent has copy, drop, store {
        value: u64,
        shift_amount: u8,
        result: u64,
        generic_bytes: vector<u8>,
    }

    // ===== Demo structs =====

    public struct DemoStruct has key {
        value: u64,
        shift_amount: u8,
    }

    public struct ShiftConfig has copy, drop, store {
        shift_amount: u8,
        multiplier: u8,
    }

    public struct NestedDemoStruct has key {
        value: u64,
        config: ShiftConfig,
    }

    // ===== Basic integer shift and events =====

    public entry fun integer_shl(_account: &signer, value: u64, shift_amount: u8) {
        let result = value << shift_amount;
        event::emit<ShiftLeftEvent>(ShiftLeftEvent { value, shift_amount, result });
    }

    public entry fun generic_shl(_account: &signer, val1: vector<u8>, val2: vector<u8>) {
        // Interpret val1 low 8 bytes as little-endian u64
        let value = if (vector::length(&val1) >= 8) {
            let acc = 0u64;
            let i = 0u64;
            while (i < 8) {
                let byte = *vector::borrow(&val1, i) as u64;
                acc = acc | (byte << ((i * 8) as u8));
                i = i + 1;
            }; acc
        } else { 0u64 };

        let shift_amount = if (vector::length(&val2) > 0) { *vector::borrow(&val2, 0) } else { 0u8 };
        let result = value << shift_amount;
        event::emit<GenericShiftLeftEvent>(GenericShiftLeftEvent {
            value,
            shift_amount,
            result,
            generic_bytes: bcs::to_bytes(&val1),
        });
    }

    public entry fun vector_shl(_account: &signer, values: vector<u32>) {
        assert!(vector::length(&values) >= 2, 2);
        let value = *vector::borrow(&values, 0) as u64;
        let shift_amount = (*vector::borrow(&values, 1) as u8);
        let result = value << shift_amount;
        event::emit<ShiftLeftEvent>(ShiftLeftEvent { value, shift_amount, result });
    }

    // ===== DemoStruct lifecycle and shift =====

    public entry fun create_demo_struct(account: &signer, value: u64, shift_amount: u8) {
        let addr = signer::address_of(account);
        assert!(!exists<DemoStruct>(addr), 3);
        move_to<DemoStruct>(account, DemoStruct { value, shift_amount });
    }

    public entry fun struct_shl(account: &signer) acquires DemoStruct {
        let addr = signer::address_of(account);
        let s = borrow_global_mut<DemoStruct>(addr);
        let result = s.value << s.shift_amount;
        // emit and update
        event::emit<ShiftLeftEvent>(ShiftLeftEvent { value: s.value, shift_amount: s.shift_amount, result });
        s.value = result;
        s.shift_amount = s.shift_amount + 1;
    }

    // ===== NestedDemoStruct lifecycle and shift =====

    public entry fun create_nested_demo_struct(account: &signer, value: u64, shift_amount: u8, multiplier: u8) {
        let addr = signer::address_of(account);
        assert!(!exists<NestedDemoStruct>(addr), 4);
        let cfg = ShiftConfig { shift_amount, multiplier };
        move_to<NestedDemoStruct>(account, NestedDemoStruct { value, config: cfg });
    }

    public entry fun nested_struct_shl(account: &signer) acquires NestedDemoStruct {
        let addr = signer::address_of(account);
        let s = borrow_global_mut<NestedDemoStruct>(addr);
        let result = s.value << s.config.shift_amount;
        event::emit<ShiftLeftEvent>(ShiftLeftEvent { value: s.value, shift_amount: s.config.shift_amount, result });
        s.value = result;
        s.config.shift_amount = s.config.shift_amount + 1;
        s.config.multiplier = s.config.multiplier + 1;
    }
}


