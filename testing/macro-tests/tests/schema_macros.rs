use std::any::TypeId;

use domm_degens_schema::schema::{
    COMMIT_MEMORY_ID, DATA_MEMORY_ID, DegensCanister, DegensStore, INDEX_MEMORY_ID,
    SCHEMA_MEMORY_ID,
};

#[test]
fn schema_canister_and_store_types_are_available() {
    let canister = TypeId::of::<DegensCanister>();
    let store = TypeId::of::<DegensStore>();

    assert_ne!(canister, store);
}

#[test]
fn schema_memory_ids_match_spec_checkpoint_zero_baseline() {
    assert_eq!(DATA_MEMORY_ID, 20);
    assert_eq!(INDEX_MEMORY_ID, 21);
    assert_eq!(SCHEMA_MEMORY_ID, 22);
    assert_eq!(COMMIT_MEMORY_ID, 119);
}
