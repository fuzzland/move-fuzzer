# Aptos Move Executor — In-Memory Overlay Usage

This crate exposes a lightweight, in-memory Aptos Move executor with an overlay `StateView` (no RocksDB). You can:
- execute BCS-encoded `SignedTransaction`s,
- optionally overlay temporary state objects,
- read objects by `StateKey`.

## Simulator trait

```rust
use aptos_private_node::{StateManager, AptosMoveExecutor, Simulator};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // In-memory state, bootstrap framework via genesis
    let sm = StateManager::new()?;
    let exec = AptosMoveExecutor::new(sm);

    // BCS-encoded SignedTransaction
    let tx_bytes: Vec<u8> = /* ... */ vec![];

    // Optional overrides: (BCS-encoded StateKey, Option<raw value>)
    let overrides: Vec<(Vec<u8>, Option<Vec<u8>>)> = vec![];

    // Execute via Simulator
    let result = exec.simulate(tx_bytes, overrides, None).await?;
    println!("status={:?} gas={} events={} writes={}", result.status, result.gas_used, result.events.len(), result.write_set.len());

    // Read object by BCS-encoded StateKey
    let key_bytes: Vec<u8> = /* bcs::to_bytes(&state_key)? */ vec![];
    let value_opt = exec.get_object(&key_bytes).await; // None => not found/error; Some(None) => tombstoned
    println!("value={:?}", value_opt);

    Ok(())
}
```

## Direct execution

```rust
use aptos_private_node::{StateManager, AptosMoveExecutor};
use aptos_types::transaction::SignedTransaction;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sm = StateManager::new()?; // Genesis is applied automatically
    let exec = AptosMoveExecutor::new(sm);

    let signed: SignedTransaction = /* build or decode */ unimplemented!();
    let res = exec.execute_transaction_with_overlay(signed).await?;
    println!("status={:?} gas={}", res.status, res.gas_used);
    Ok(())
}
```


