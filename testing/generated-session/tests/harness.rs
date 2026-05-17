use std::any::TypeId;

use domm_degens_schema::schema::{
    BattleParticipantRoundReady, DegensCanister, ParticipantTurnReady, SystemJob,
};
use domm_game::{TURN_DURATION_MS, first_playable_fixture};
use icydb::{Create, types::Timestamp};

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

#[test]
fn generated_session_layer_links_durable_job_entities() {
    let _ = TypeId::of::<SystemJob>();
    let _ = TypeId::of::<ParticipantTurnReady>();
    let _ = TypeId::of::<BattleParticipantRoundReady>();
}

#[test]
fn generated_session_layer_exposes_durable_job_create_bindings() {
    let due_at = Timestamp::from_millis(1_000);
    let job: Create<SystemJob> = Create::<SystemJob> {
        job_key: Some("turn_deadline:session:1".to_string()),
        job_kind: Some("turn_deadline".to_string()),
        session_id: None,
        battle_id: Some(None),
        turn_number: Some(Some(1)),
        due_at: Some(due_at),
        status: Some("scheduled".to_string()),
        lease_owner: Some(None),
        lease_expires_at: Some(None),
        attempt_count: Some(0),
        generation: Some(0),
        command_id: Some(None),
        cursor_json: Some(None),
        last_error: Some(None),
    };

    let turn_ready: Create<ParticipantTurnReady> = Create::<ParticipantTurnReady> {
        session_id: None,
        participant_id: None,
        turn_number: Some(1),
        command_id: Some(None),
    };

    let battle_ready: Create<BattleParticipantRoundReady> = Create::<BattleParticipantRoundReady> {
        session_id: None,
        battle_id: None,
        participant_id: None,
        round_number: Some(1),
        command_id: Some(None),
        ready_reason: Some("player_end_turn".to_string()),
    };

    assert_eq!(job.status.as_deref(), Some("scheduled"));
    assert_eq!(turn_ready.turn_number, Some(1));
    assert_eq!(
        battle_ready.ready_reason.as_deref(),
        Some("player_end_turn")
    );
}
