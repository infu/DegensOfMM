use domm_client_probe::{FixtureProbeBackend, ThinClientProbe, render_opening_viewport};
use domm_game::{HeadlessGameDriver, first_playable_fixture};

#[test]
fn gate_b_loads_and_renders_opening_viewport_from_public_dtos() {
    let fixture = first_playable_fixture();
    let backend = FixtureProbeBackend::new(fixture.clone()).expect("fixture backend should build");
    let mut driver = HeadlessGameDriver::new(backend, fixture.clone());
    let match_view = driver
        .create_join_start_inspect()
        .expect("public lobby path should start a match");
    let backend = driver.into_backend();
    let mut probe = ThinClientProbe::new(backend);

    let loaded = probe
        .load_opening_viewport(fixture.principals.player_one, &match_view.session_id)
        .expect("client probe should load the opening viewport");
    let rendered =
        render_opening_viewport(&loaded).expect("client probe should render visible DTOs");

    assert_eq!(loaded.match_view.session_id, fixture.ids.session_id);
    assert_eq!(
        loaded.participant.participant_id,
        fixture.ids.participant_one_id
    );
    assert_eq!(loaded.chunks.len(), 4);
    assert_eq!(loaded.events.events.len(), 1);
    assert_eq!(loaded.events.events[0].event_type, "session_started");
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
