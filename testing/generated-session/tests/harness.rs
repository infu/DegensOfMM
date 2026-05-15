use std::any::TypeId;

use domm_degens_schema::schema::DegensCanister;
use domm_game::{TURN_DURATION_MS, first_playable_fixture};

#[test]
fn generated_session_layer_can_share_deterministic_fixture_data() {
    let fixture = first_playable_fixture();

    assert_eq!(fixture.clock.turn_duration_ms, TURN_DURATION_MS);
    assert_eq!(fixture.command_nonces.start_session, "nonce:lobby:start:v1");
    assert_ne!(fixture.principals.player_one, fixture.principals.player_two);
}

#[test]
fn generated_session_layer_links_the_schema_canister_type() {
    let _ = TypeId::of::<DegensCanister>();
}
