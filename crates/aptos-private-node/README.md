# Aptos Private Node — Simulator Usage

Below shows how to use this crate via the generic `Simulator` trait.

```rust
use aptos_private_node::{AptosPrivateNodeBuilder, Simulator};
use aptos_types::{state_store::state_key::StateKey, transaction::SignedTransaction};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Build and init
    let node = AptosPrivateNodeBuilder::new()
        .with_data_dir("./private-node-data")
        .build()?;
    node.initialize_from_genesis().await?;

    // Prepare a transaction (fill as needed)
    let tx: SignedTransaction = todo!("construct a SignedTransaction");

    // Run simulate with optional overrides (Id=StateKey, Obj=Option<Vec<u8>>)
    let result = node
        .simulate(tx, Vec::<(StateKey, Option<Vec<u8>>)>::new(), Option::<()>::None)
        .await?;
    println!("gas_used={}", result.gas_used);

    // Read a single object
    let key = StateKey::raw(b"demo_key");
    let value = node.get_object(&key).await; // Option<Option<Vec<u8>>>
    println!("value={:?}", value);

    Ok(())
}
```

