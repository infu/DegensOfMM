use icydb::design::prelude::*;

pub const DATA_MEMORY_ID: u8 = 20;
pub const INDEX_MEMORY_ID: u8 = 21;
pub const SCHEMA_MEMORY_ID: u8 = 22;
pub const COMMIT_MEMORY_ID: u8 = 119;

/// Main game canister declaration.
#[canister(memory_min = 20, memory_max = 120, commit_memory_id = 119)]
pub struct DegensCanister {}

/// Main game store declaration.
#[store(
    ident = "DEGENS_STORE",
    canister = "DegensCanister",
    data_memory_id = 20,
    index_memory_id = 21,
    schema_memory_id = 22
)]
pub struct DegensStore {}
