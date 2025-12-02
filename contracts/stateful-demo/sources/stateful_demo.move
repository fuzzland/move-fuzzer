module stateful_demo::counter_check {
    use std::signer;

    struct Counter has key {
        val: u64,
    }

    const E_BUG: u64 = 0x1337;

    public entry fun initialize(account: &signer) {
        if (!exists<Counter>(signer::address_of(account))) {
            move_to(account, Counter { val: 0 });
        }
    }

    public entry fun increment(account: &signer) acquires Counter {
        let addr = signer::address_of(account);
        if (exists<Counter>(addr)) {
            let c = borrow_global_mut<Counter>(addr);
            c.val = c.val + 1;
        }
    }

    public entry fun check_bug(account: &signer) acquires Counter {
        let addr = signer::address_of(account);
        if (exists<Counter>(addr)) {
            let c = borrow_global<Counter>(addr);
            if (c.val == 5) {
                abort E_BUG
            }
        }
    }
}

