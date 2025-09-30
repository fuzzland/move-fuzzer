use std::str::FromStr;
use std::time::Instant;

use sui_simulator::{RpcSimulator, SimulateResult, Simulator};
use sui_types::base_types::{ObjectID, SuiAddress};
use sui_types::object::Object;
use sui_types::programmable_transaction_builder::ProgrammableTransactionBuilder;
use sui_types::transaction::{ObjectArg, TransactionData};
use sui_types::Identifier;

const RPC_URL: &str = "http://177.54.159.23:9000";

#[tokio::main]
async fn main() {
    println!("========== RpcSimulator Shift Monitor ==========");
    let value: u64 = u64::MAX;
    let shl_amount: u8 = 10;
    println!("Testcase: {} << {}", value, shl_amount);
    println!("===================================");

    tracing_subscriber::fmt::init();

    // Create the simulator from the real database
    let start = Instant::now();
    let simulator = RpcSimulator::new(RPC_URL).await;

    println!(
        "✅ [{:?}] Simulator created successfully: {}",
        start.elapsed(),
        simulator.name()
    );

    let owner_cap_id =
        ObjectID::from_hex_literal("0x052445c01fa0a538b17e6d83ceb3dae41db8046630ec090c472519bf8411e9d1").unwrap();
    let owner_cap = simulator.get_object(&owner_cap_id).await.expect("OwnerCap not found");
    let owner_cap_obj_ref = owner_cap.compute_object_reference();

    let sender = SuiAddress::from_str("0xc0f620f28826593835606e174e6e9912c342101920519a1e376957691178e345").unwrap();
    let contract =
        ObjectID::from_hex_literal("0x5b859a8617174531b676b8fbc97415fbe2a1921791f1b8ebc2d21eb1457c3ffa").unwrap();

    // Gas parameters
    let gas_budget = 10_000_000_000;
    let gas_price = 2_000_000;

    // Create a gas coin for the transaction
    let gas_coin = Object::new_gas_with_balance_and_owner_for_testing(1_000_000_000_000, sender);
    let gas_payment = vec![gas_coin.compute_object_reference()];
    let override_objects = vec![(gas_coin.id(), gas_coin)];

    // Build the programmable transaction
    let mut ptb = ProgrammableTransactionBuilder::new();

    let owner_cap_arg = ptb.obj(ObjectArg::ImmOrOwnedObject(owner_cap_obj_ref)).unwrap();
    let value_arg = ptb.pure(value).unwrap();
    let shl_amount_arg = ptb.pure(shl_amount).unwrap();

    // Call the shift_left function
    ptb.programmable_move_call(
        contract,
        Identifier::from_str("shift_left").unwrap(),
        Identifier::from_str("shift_left").unwrap(),
        vec![], // No type arguments
        vec![owner_cap_arg, value_arg, shl_amount_arg],
    );

    let pt = ptb.finish();
    let tx = TransactionData::new_programmable(sender, gas_payment, pt, gas_budget, gas_price);

    let start = Instant::now();
    let result = simulator
        .simulate(tx, override_objects, None)
        .await
        .expect("Simulation failed");

    println!("✅ [{:?}] Simulation completed successfully", start.elapsed());
    let SimulateResult { events, .. } = result;

    println!("Events           🧀 {:?}", events);
    println!("========== RpcSimulator Shift Monitor ==========");
}
