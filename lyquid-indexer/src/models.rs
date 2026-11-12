use lyquid::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BalanceProposal {
    pub holder: Address,
    pub block: u64,
    pub balance: U256,
}
