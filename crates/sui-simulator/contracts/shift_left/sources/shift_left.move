module shift_left::shift_left {
    use sui::event;

    /// Owner capability
    public struct OwnerCap has key {
        id: UID,
    }

    /// Event emitted when shift_left is called
    public struct ShiftLeftEvent has copy, drop {
        original_value: u64,
        shift_amount: u8,
        result: u64,
    }

    /// Initialize the contract and transfer OwnerCap to deployer
    fun init(ctx: &mut TxContext) {
        let owner_cap = OwnerCap {
            id: object::new(ctx),
        };
        transfer::transfer(owner_cap, tx_context::sender(ctx));
    }

    /// Only owner can call this function to perform left shift operation
    public entry fun shift_left(
        _owner_cap: &OwnerCap,
        value: u64,
        shl_amount: u8,
        _ctx: &mut TxContext
    ) {
        // Perform left shift operation
        let result = value << shl_amount;
        
        
        // Emit event with the result since we can't return it
        event::emit(ShiftLeftEvent {
            original_value: value,
            shift_amount: shl_amount,
            result,
        });
    }

    /// Test helper function to create OwnerCap for testing
    #[test_only]
    public fun create_owner_cap_for_testing(ctx: &mut TxContext): OwnerCap {
        OwnerCap {
            id: object::new(ctx),
        }
    }
}