//! Access policy checked before any Consumer configuration or result changes.

use crate::invalid;
use lyquid::prelude::*;

pub fn validate(
    caller: Address,
    initializer: Address,
    indexer: Address,
    holder: Address,
) -> LyquidResult<()> {
    if caller != initializer {
        return Err(invalid("only initializer can configure consumer"));
    }
    if indexer == Address::ZERO || holder == Address::ZERO {
        return Err(invalid("indexer and holder must be nonzero addresses"));
    }
    Ok(())
}
