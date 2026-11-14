//! Eligibility calculation for a validated HolderProof certificate.

use crate::invalid;
use holderproof_types::QueryCertificate;
use lyquid::prelude::*;

pub fn evaluate(certificate: &QueryCertificate, threshold: U256) -> LyquidResult<(U256, bool)> {
    let balance = parse_balance(certificate)?;
    Ok((balance, balance >= threshold))
}

fn parse_balance(certificate: &QueryCertificate) -> LyquidResult<U256> {
    certificate
        .balance
        .parse::<U256>()
        .map_err(|_| invalid("invalid certificate balance"))
}
