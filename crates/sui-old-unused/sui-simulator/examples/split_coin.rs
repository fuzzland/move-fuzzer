use std::str::FromStr;
use std::time::Instant;

use sui_simulator::{RpcSimulator, SimulateResult, Simulator};
use sui_types::base_types::{ObjectID, SuiAddress};
use sui_types::object::Object;
use sui_types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_types::transaction::TransactionData;

const RPC_URL: &str = "http://177.54.159.23:9000";

#[tokio::main]
async fn main() {
    println!("========== RpcSimulator Split Coin ==========");
    tracing_subscriber::fmt::init();

    // Create the simulator from the real database
    let start = Instant::now();
    let simulator = RpcSimulator::new(RPC_URL).await;

    println!(
        "✅ [{:?}] Simulator created successfully: {}",
        start.elapsed(),
        simulator.name()
    );

    let sender = SuiAddress::from_str("0x15610fa7ee546b96cb580be4060fae1c4bb15eca87f9a0aa931512bad445fc76").unwrap();
    let recipient = SuiAddress::random_for_testing_only();
    let coin_id =
        ObjectID::from_hex_literal("0xac5e1a72a13b546345883ea9156f9f6426d2aa41a5f96d9e6b951cb15a55fb24").unwrap();
    let coin = simulator.get_object(&coin_id).await.expect("Coin not found");
    let coin_ref = coin.compute_object_reference();
    let split_amount = 100_000_000;

    let gas_budget = 10_000_000_000;
    let gas_price = 2_000_000;
    let gas_coin = Object::new_gas_with_balance_and_owner_for_testing(1_000_000_000_000, sender);
    let gas_payment = vec![gas_coin.compute_object_reference()];
    let override_objects = vec![(gas_coin.id(), gas_coin)];

    let mut ptb = ProgrammableTransactionBuilder::new();
    ptb.split_coin(recipient, coin_ref, vec![split_amount]);
    let pt = ptb.finish();
    let tx = TransactionData::new_programmable(sender, gas_payment, pt, gas_budget, gas_price);

    let start = Instant::now();
    let result = simulator
        .simulate(tx, override_objects, None)
        .await
        .expect("Simulation failed");

    println!("✅ [{:?}] Simulation completed successfully", start.elapsed());
    let SimulateResult {
        effects,
        events,
        object_changes,
        balance_changes,
        ..
    } = result;

    println!("Effects          🧀 {:?}", effects);
    println!("Events           🧀 {:?}", events);
    println!("Object Changes   🧀 {:?}", object_changes);
    println!("Balance Changes  🧀 {:?}", balance_changes);
    println!("========== RpcSimulator Split Coin ==========");
}
