//! Shared wire types used by the HolderProof Indexer and Consumer.
//!
//! These types deliberately stay independent of Lyquid runtime state. They
//! describe the fixture, checkpoint and certificate exchanged between the two
//! Lyquids.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const CHAIN_ID: u64 = 11_155_111;
pub const PARSER_VERSION: &str = "holderproof-v1";

/// Committee agreement threshold: the devnet committee is 4 nodes, so
/// `n / 2 + 1 == 3`. Keep this in one place because the Indexer proposes with
/// it and the Consumer enforces it when validating a certificate.
pub const COMMITTEE_THRESHOLD: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySpec {
    pub chain_id: u64,
    pub token: String,
    pub holder: String,
    pub block: u64,
    pub parser_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCertificate {
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
