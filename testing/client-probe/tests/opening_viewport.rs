use domm_client_probe::{FixtureProbeBackend, ThinClientProbe, render_opening_viewport};
use domm_game::first_playable_fixture;

#[test]
fn gate_b_loads_and_renders_opening_viewport_from_final_game_view_dto() {
    let fixture = first_playable_fixture();
    let mut backend = FixtureProbeBackend::new(fixture.clone());
    let session = backend.start_first_playable_session();
    let mut probe = ThinClientProbe::new(backend);

    let loaded = probe
        .load_opening_viewport(fixture.principals.player_one, &session.session_id)
        .expect("client probe should load the opening viewport");
    let rendered =
        render_opening_viewport(&loaded).expect("client probe should render visible DTOs");

    assert_eq!(loaded.game_view.session.session_id, fixture.ids.session_id);
    assert_eq!(
        loaded.game_view.participant.participant_id,
        fixture.ids.participant_one_id
    );
    assert_eq!(loaded.chunks.len(), 4);
    assert!(
        loaded
            .events
            .events
            .iter()
            .any(|event| event.event_type == "session_started")
    );
    assert!(!loaded.sync_required);

    assert_eq!(rendered.width, 24);
    assert_eq!(rendered.height, 24);
    assert_eq!(rendered.rows.len(), 24);
    assert!(rendered.rows.iter().all(|row| row.len() == 24));
    assert!(
        rendered
            .visible_champions
            .iter()
            .any(|name| name == "Mara of the Toll")
    );
    assert!(rendered.visible_towns.iter().any(|name| name == "West Woe"));
    assert!(
        rendered
            .visible_resources
            .iter()
            .any(|id| id == "pile:west-wood-1")
    );
    assert!(
        rendered
            .visible_neutrals
            .iter()
            .any(|id| id == "neutral:west-mine")
    );
    assert!(
        rendered
            .event_summaries
            .iter()
            .any(|event| event == "session_started#1")
    );
    assert!(!rendered.sync_required);
    assert!(rendered.rows.iter().any(|row| row.contains('C')));
    assert!(rendered.rows.iter().any(|row| row.contains('T')));
    assert!(rendered.rows.iter().any(|row| row.contains('$')));
    assert!(
        loaded
            .objects
            .iter()
            .all(|object| object.subject_id_text != "champion:east")
    );
}
