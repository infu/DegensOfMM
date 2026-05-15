//! Degens canister entrypoint.

extern crate canic_cdk as ic_cdk;

icydb::start!();

#[allow(dead_code)]
fn icydb_admin_sql_load_default() -> Result<(), icydb::Error> {
    Ok(())
}

canic_cdk::export_candid!();
