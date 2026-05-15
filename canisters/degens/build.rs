fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::any::TypeId::of::<domm_degens_schema::schema::DegensCanister>();

    let config = icydb_config_build::emit_config_for_canister("degens", &["degens"])?;
    let options = icydb::build::BuildOptions::default()
        .with_sql_readonly_enabled(config.canister_sql_readonly_enabled("degens"))
        .with_sql_ddl_enabled(config.canister_sql_ddl_enabled("degens"));

    icydb::build_with_options!("domm_degens_schema::schema::DegensCanister", options);

    Ok(())
}
