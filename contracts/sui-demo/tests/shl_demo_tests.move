#[test_only]
module shl_demo::shl_demo_tests {
    use shl_demo::shl_demo::{Self, DemoStruct};
    use sui::test_scenario::{Self as ts};
    use sui::test_utils::assert_eq;

    const ADMIN: address = @0xAD;
    const USER: address = @0x123;

    #[test]
    fun test_integer_shl() {
        let scenario = ts::begin(ADMIN);
        
        // Test basic shift operation: 5 << 2 = 20
        shl_demo::integer_shl(5, 2);
        
        ts::end(scenario);
    }

    #[test] 
    fun test_generic_shl() {
        let scenario = ts::begin(ADMIN);
        
        // Test u64 generic shift: 10 << 3 = 80
        shl_demo::generic_shl<u64>(10, 3, 100);
        
        ts::end(scenario);
    }


    #[test]
    fun test_vector_shl() {
        let scenario = ts::begin(ADMIN);
        
        // Test vector shift: values[0] = 6, values[1] = 2, so 6 << 2 = 24
        let mut values = vector::empty<u32>();
        vector::push_back(&mut values, 6);
        vector::push_back(&mut values, 2);
        
        shl_demo::vector_shl(values);
        
        ts::end(scenario);
    }

    #[test]
    fun test_owned_struct_shl() {
        let mut scenario = ts::begin(ADMIN);
        {
            let ctx = ts::ctx(&mut scenario);
            
            // Create a DemoStruct and test owned operation
            let demo_struct = shl_demo::new_demo_struct(8, 1, ctx);
            
            // Test: 8 << 1 = 16
            shl_demo::owned_struct_shl(demo_struct, ctx);
        };
        
        ts::end(scenario);
    }

    #[test]
    fun test_shared_struct_operations() {
        let mut scenario = ts::begin(ADMIN);
        
        // Create a shared DemoStruct
        {
            let ctx = ts::ctx(&mut scenario);
            shl_demo::create_shared_demo_struct(12, 2, ctx);
        };
        
        // Test immutable shared struct operation
        ts::next_tx(&mut scenario, USER);
        {
            let shared_obj = ts::take_shared<DemoStruct>(&scenario);
            
            // Test: 12 << 2 = 48
            shl_demo::immutable_shared_struct_shl(&shared_obj);
            
            ts::return_shared(shared_obj);
        };
        
        // Test mutable shared struct operation
        ts::next_tx(&mut scenario, USER);
        {
            let mut shared_obj = ts::take_shared<DemoStruct>(&scenario);
            
            // Test: 12 << 2 = 48, then value incremented to 13
            shl_demo::mutable_shared_struct_shl(&mut shared_obj);
            
            // Verify the value was incremented
            assert_eq(shl_demo::get_value(&shared_obj), 13);
            
            ts::return_shared(shared_obj);
        };
        
        ts::end(scenario);
    }

    #[test]
    fun test_owned_struct_creation_and_transfer() {
        let mut scenario = ts::begin(ADMIN);
        
        // Create and transfer owned DemoStruct
        {
            let ctx = ts::ctx(&mut scenario);
            shl_demo::create_owned_demo_struct(15, 3, ctx);
        };
        
        // Verify the struct was transferred to ADMIN
        ts::next_tx(&mut scenario, ADMIN);
        {
            let owned_obj = ts::take_from_sender<DemoStruct>(&scenario);
            
            // Verify values
            assert_eq(shl_demo::get_value(&owned_obj), 15);
            assert_eq(shl_demo::get_shift_amount(&owned_obj), 3);
            
            ts::return_to_sender(&scenario, owned_obj);
        };
        
        ts::end(scenario);
    }

    #[test]
    fun test_getters_and_setters() {
        let mut scenario = ts::begin(ADMIN);
        
        {
            let ctx = ts::ctx(&mut scenario);
            shl_demo::create_shared_demo_struct(20, 4, ctx);
        };
        
        ts::next_tx(&mut scenario, USER);
        {
            let mut shared_obj = ts::take_shared<DemoStruct>(&scenario);
            
            // Test getters
            assert_eq(shl_demo::get_value(&shared_obj), 20);
            assert_eq(shl_demo::get_shift_amount(&shared_obj), 4);
            
            // Test setters
            shl_demo::set_value(&mut shared_obj, 25);
            shl_demo::set_shift_amount(&mut shared_obj, 5);
            
            // Verify changes
            assert_eq(shl_demo::get_value(&shared_obj), 25);
            assert_eq(shl_demo::get_shift_amount(&shared_obj), 5);
            
            ts::return_shared(shared_obj);
        };
        
        ts::end(scenario);
    }

    #[test]
    fun test_edge_cases() {
        let scenario = ts::begin(ADMIN);
        
        // Test with zero values
        shl_demo::integer_shl(0, 5); // 0 << 5 = 0
        shl_demo::integer_shl(10, 0); // 10 << 0 = 10
        
        // Test with larger values
        shl_demo::integer_shl(1, 63); // 1 << 63 = large number
        
        ts::end(scenario);
    }
}
