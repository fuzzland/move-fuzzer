# Aptos Private Node — Simulator Usage

Below shows how to use this crate via the generic `Simulator` trait.

```rust
use aptos_private_node::{AptosPrivateNodeBuilder, Simulator};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let node = AptosPrivateNodeBuilder::new()
        .with_data_dir("./private-node-data")
        .build()?;

    // BCS-encoded SignedTransaction
    let tx_bytes: Vec<u8> = /* ... */ vec![];

    // Overrides: (BCS-encoded StateKey bytes, optional raw value)
    let overrides: Vec<(Vec<u8>, Option<Vec<u8>>)> = vec![];

    // (success, gas_used, write_set, events, fee_statement_bcs, cache_misses)
    let (_ok, gas_used, _ws, _events, _fee, _miss) =
        node.simulate(tx_bytes, overrides, None).await?;
    println!("gas_used={}", gas_used);

    // Read object by BCS-encoded StateKey bytes
    let key_bytes: Vec<u8> = /* bcs::to_bytes(&state_key)? */ vec![];
    let value = node.get_object(&key_bytes).await; // None => not found or error
    println!("value={:?}", value);

    Ok(())
}
```

