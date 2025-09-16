#[test_only]
module shift_left::shift_left_tests {
    use shift_left::shift_left::{Self};

    #[test]
    fun test_owner_can_call_shift_left() {
        use sui::test_utils;
        
        // Create a test context
        let mut ctx = tx_context::dummy();
        
        // Create owner capability using test helper
        let owner_cap = shift_left::create_owner_cap_for_testing(&mut ctx);
        
        // This should not panic
        shift_left::shift_left(&owner_cap, 5, 2, &mut ctx);
        
        // Clean up
        test_utils::destroy(owner_cap);
    }

}
