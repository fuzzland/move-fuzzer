use aptos_types::contract_event::ContractEvent;
use aptos_types::fee_statement::FeeStatement;
use aptos_types::transaction::TransactionStatus;
use aptos_types::write_set::WriteSet;

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub status: TransactionStatus,
    pub gas_used: u64,
    pub write_set: WriteSet,
    pub events: Vec<ContractEvent>,
    pub fee_statement: Option<FeeStatement>,
}
