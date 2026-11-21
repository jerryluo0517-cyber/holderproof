//! Shared wire types used by the HolderProof Indexer and Consumer.
//!
//! These types deliberately stay independent of Lyquid runtime state. They
//! describe the fixture, checkpoint and certificate exchanged between the two
//! Lyquids.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const CHAIN_ID: u64 = 11_155_111;
/// The only token supported by the Step 4 deterministic fixture.
pub const FIXTURE_TOKEN: &str = "0x5298000000000000000000000000000000000001";
pub const FIXTURE_END_BLOCK: u64 = 1000;
pub const DATASET_ID: &str = "holderproof-fixture-5298-v1";
pub const COMMITTEE_SIZE: usize = 4;
pub const PARSER_VERSION: &str = "holderproof-v1";

/// P0 policy: two distinct approvals from the three-member committee.
pub const COMMITTEE_THRESHOLD: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferEvent {
    pub chain_id: u64,
    pub token: String,
    pub block_number: u64,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    pub from: String,
    pub to: String,
    pub amount: String,
    pub parser_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuerySpec {
    pub chain_id: u64,
    pub token: String,
    pub holder: String,
    pub block: u64,
    pub parser_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub dataset_id: String,
    pub block_hash_start: String,
    pub from_block: u64,
    pub to_block: u64,
    pub block_hash: String,
    pub event_count: u64,
    pub event_merkle_root: String,
    pub parser_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CertificateStatus {
    Finalized,
    Pending,
    Challenged,
    Invalidated,
}

impl fmt::Display for CertificateStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Finalized => "Finalized",
            Self::Pending => "Pending",
            Self::Challenged => "Challenged",
            Self::Invalidated => "Invalidated",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryCertificate {
    pub query_type: QueryType,
    pub fact_hash: String,
    pub node_signatures: Vec<NodeSignature>,
    /// Full native OracleCert; authenticity is enforced by the Lyquor runtime.
    pub runtime_certificate: String,
    pub network_state_id: String,
    pub certificate_id: String,
    pub query_hash: String,
    pub chain_id: u64,
    pub token: String,
    pub holder: String,
    pub block_number: u64,
    /// Decimal string at the wire boundary; convert to `U256` in the consumer.
    pub balance: String,
    pub checkpoint: Checkpoint,
    pub signer_ids: Vec<String>,
    pub status: CertificateStatus,
    pub created_at: u64,
}

impl QuerySpec {
    /// Canonical query identity shared by producer and consumer. Keep the field
    /// order stable: persisted certificates are keyed by this digest.
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let canonical = format!(
            "query|{}|{}|{}|{}|{}",
            self.chain_id, self.token, self.holder, self.block, self.parser_version
        );
        hex::encode(Sha256::digest(canonical.as_bytes()))
    }
}

/// P0 certifies a single historical balance. Thresholds belong to gate policy,
/// so one certificate can be reused by gates with different thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryType {
    Balance,
    CreditScore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSignature {
    pub node_id: String,
    pub signer_id: u32,
    pub signature: String,
}

/// Every field in this value travels inside the runtime-signed call payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceFact {
    pub query: QuerySpec,
    pub query_type: QueryType,
    pub balance: String,
    pub checkpoint: Checkpoint,
}

impl BalanceFact {
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let bytes =
            serde_json::to_vec(self).expect("BalanceFact contains only serializable fields");
        hex::encode(Sha256::digest(bytes))
    }
}

impl QueryCertificate {
    pub fn fact(&self) -> BalanceFact {
        BalanceFact {
            query: QuerySpec {
                chain_id: self.chain_id,
                token: self.token.clone(),
                holder: self.holder.to_ascii_lowercase(),
                block: self.block_number,
                parser_version: self.checkpoint.parser_version.clone(),
            },
            query_type: self.query_type,
            balance: self.balance.clone(),
            checkpoint: self.checkpoint.clone(),
        }
    }
}

/// Identifies the immutable fixture source, not a live chain header.
pub fn fixture_block_hash(block: u64) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "0x{}",
        hex::encode(Sha256::digest(
            format!("block|{CHAIN_ID}|{block}").as_bytes()
        ))
    )
}

pub const CREDIT_PROTOCOL: &str = "0x5298000000000000000000000000000000000002";
pub const CREDIT_DATASET_ID: &str = "vericredit-fixture-5298-v1";

pub const DEFAULT_CREDIT_W1: u64 = 35;
pub const DEFAULT_CREDIT_W2: u64 = 20;
pub const DEFAULT_CREDIT_ALPHA: u64 = 2;
pub const DEFAULT_WASH_WINDOW: u64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditParams {
    pub w1: u64,
    pub w2: u64,
    pub alpha: u64,
    pub wash_window: u64,
}

impl Default for CreditParams {
    fn default() -> Self {
        Self {
            w1: DEFAULT_CREDIT_W1,
            w2: DEFAULT_CREDIT_W2,
            alpha: DEFAULT_CREDIT_ALPHA,
            wash_window: DEFAULT_WASH_WINDOW,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditEvent {
    pub chain_id: u64,
    pub protocol: String,
    pub block_number: u64,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    pub user: String,
    pub counterparty: String,
    pub event_type: String,
    pub amount: String,
    pub parser_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditQuerySpec {
    pub chain_id: u64,
    pub protocol: String,
    pub user: String,
    pub start_block: u64,
    pub end_block: u64,
    pub params: CreditParams,
    pub parser_version: String,
}

impl CreditQuerySpec {
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let canonical = format!(
            "credit|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.chain_id,
            self.protocol,
            self.user,
            self.start_block,
            self.end_block,
            self.params.w1,
            self.params.w2,
            self.params.alpha,
            self.params.wash_window,
            self.parser_version
        );
        hex::encode(Sha256::digest(canonical.as_bytes()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditMetrics {
    pub raw_volume: String,
    pub clean_volume: String,
    pub wash_volume_removed: String,
    pub repay_count: u64,
    pub liquidation_count: u64,
    pub score: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditFact {
    pub query: CreditQuerySpec,
    pub query_type: QueryType,
    pub metrics: CreditMetrics,
    pub checkpoint: Checkpoint,
}

impl CreditFact {
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(self).expect("CreditFact is serializable");
        hex::encode(Sha256::digest(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditCertificate {
    pub query_type: QueryType,
    pub fact_hash: String,
    pub node_signatures: Vec<NodeSignature>,
    pub runtime_certificate: String,
    pub network_state_id: String,
    pub certificate_id: String,
    pub query_hash: String,
    pub query: CreditQuerySpec,
    pub metrics: CreditMetrics,
    pub checkpoint: Checkpoint,
    pub signer_ids: Vec<String>,
    pub status: CertificateStatus,
    pub created_at: u64,
}

impl CreditCertificate {
    pub fn fact(&self) -> CreditFact {
        CreditFact {
            query: self.query.clone(),
            query_type: self.query_type,
            metrics: self.metrics.clone(),
            checkpoint: self.checkpoint.clone(),
        }
    }
}

pub const DEFAULT_EPOCH_INTERVAL: u64 = 250;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochFact {
    pub epoch_id: u64,
    pub interval: u64,
    pub checkpoint: Checkpoint,
    pub total_transfer_volume: String,
    pub unique_holders: u64,
}

impl EpochFact {
    pub fn hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(self).expect("EpochFact is serializable");
        hex::encode(Sha256::digest(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochCheckpoint {
    pub epoch_id: u64,
    pub interval: u64,
    pub fact_hash: String,
    pub checkpoint: Checkpoint,
    pub total_transfer_volume: String,
    pub unique_holders: u64,
    pub signer_ids: Vec<String>,
    pub node_signatures: Vec<NodeSignature>,
    pub runtime_certificate: String,
    pub network_state_id: String,
    pub status: CertificateStatus,
    pub created_at: u64,
}

pub const SEPOLIA_DATASET_ID: &str = "sepolia-rpc-v1";