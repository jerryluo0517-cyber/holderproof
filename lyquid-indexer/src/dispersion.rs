//! Honest-core dispersion guard against Byzantine outlier & leader cherry-picking
//! attacks on time-skewed oracle observations (n = 3f + 1, m >= 2f + 1).

use crate::invalid;
use lyquid::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DispersionResult {
    pub median: u128,
    pub honest_low: u128,
    pub honest_high: u128,
    pub dispersion_bps: u64,
    pub sample_count: usize,
}

/// Validate `m >= 2f + 1` distinct-signer observations and enforce:
///   (v_{m-f} - v_{f+1}) / M <= max_dispersion_bps / 10_000
/// using pure checked u128 integer arithmetic (no floating point).
pub(crate) fn aggregate_with_dispersion_guard<Signer: PartialEq>(
    observations: &[(Signer, u128)],
    f: usize,
    max_dispersion_bps: u64,
) -> LyquidResult<DispersionResult> {
    let m = observations.len();
    if m < 2 * f + 1 {
        return Err(invalid(format!(
            "need at least {} observations for f={f}, got {m}",
            2 * f + 1
        )));
    }
    for (i, (signer, _)) in observations.iter().enumerate() {
        if observations[..i].iter().any(|(other, _)| other == signer) {
            return Err(invalid("duplicate signer in observation batch"));
        }
    }

    let mut sorted: Vec<u128> = observations.iter().map(|(_, v)| *v).collect();
    sorted.sort_unstable();

    // 1-indexed v_{f+1} -> sorted[f]; 1-indexed v_{m-f} -> sorted[m - 1 - f]
    let honest_low = sorted[f];
    let honest_high = sorted[m - 1 - f];
    let median = sorted[m / 2];
    if median == 0 {
        return Err(invalid("median observation must be positive"));
    }

    let spread = honest_high.saturating_sub(honest_low);
    let lhs = spread
        .checked_mul(10_000)
        .ok_or_else(|| invalid("dispersion numerator overflow"))?;
    let rhs = median
        .checked_mul(max_dispersion_bps as u128)
        .ok_or_else(|| invalid("dispersion threshold overflow"))?;

    if lhs > rhs {
        return Err(invalid(format!(
            "observation dispersion {} bps exceeds max {} bps",
            lhs / median,
            max_dispersion_bps
        )));
    }

    Ok(DispersionResult {
        median,
        honest_low,
        honest_high,
        dispersion_bps: (lhs / median) as u64,
        sample_count: m,
    })
}