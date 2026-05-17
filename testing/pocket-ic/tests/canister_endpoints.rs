use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use canic_testkit::pic::{StandaloneCanisterFixture, install_prebuilt_canister_with_cycles};
use domm_degens_canister::{
    CanisterEndpointView, DiagnosticStorageSnapshot, REQUIRED_GAME_ENDPOINTS,
};
use domm_game::{
    ApiError, ApiEventPage, ApiTownView, BattleActionInput, BattleView, BuildPreview,
    ChampionHirePreview, ChampionProgressionView, ChampionView, CommandResponse, CommandResult,
    CommandStatus, CommandStatusView, ContentManifestResponse, DwellingPoolView,
    DwellingRecruitPreview, FIRST_PLAYABLE_RULESET_ID, FIRST_PLAYABLE_RULESET_SLUG, GameView,
    GameViewRequest, LobbyCommandResponse, LobbyCommandResult, MARKET_TRADE_MAX_INPUT,
    MAX_CHUNK_LIMIT, MAX_LIST_LIMIT, MAX_MOVE_PATH_STEPS_LIMIT, MAX_OBJECT_LIMIT, MapChunkPage,
    MarketTradePreview, MatchHistoryPage, MoveCoord, MovementPreview, NavalRoutesView,
    OPENING_QUEST_KEY, ObjectView, ObjectViewPage, ObjectiveProgressView,
    PROCEDURAL_GENERATION_KEY, ParticipantView, PlayerView, ProceduralMapView, QuestPreview,
    RecruitPreview, RecruitTarget, ScenarioRulesView, SessionView, SiegeRulesView,
    SkirmishSettingsView, TavernOffersView, WorldEventsView, opening_viewport_for_slot,
};

#[test]
fn pocket_ic_canister_exposes_every_required_game_endpoint() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-player-two");
    let viewport = opening_viewport_for_slot(0);

    let inventory: Vec<CanisterEndpointView> = fixture
        .pic()
        .query_call(fixture.canister_id(), "get_canister_endpoint_inventory", ())
        .expect("endpoint inventory should decode");
    assert_eq!(inventory.len(), REQUIRED_GAME_ENDPOINTS.len());
    for endpoint in REQUIRED_GAME_ENDPOINTS {
        assert!(
            inventory.iter().any(|view| view.name == endpoint.name),
            "missing {} from inventory",
            endpoint.name
        );
    }

    let anonymous_player: Result<PlayerView, ApiError> = fixture
        .pic()
        .query_call(fixture.canister_id(), "get_my_player", ())
        .expect("anonymous get_my_player should decode");
    assert_eq!(
        anonymous_player
            .expect_err("anonymous player query should fail")
            .code,
        "anonymous_not_allowed"
    );

    let registered_one = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "register_player",
        (
            Some("presence-one".to_string()),
            Some("Presence One".to_string()),
            "nonce:presence:register:one".to_string(),
        ),
    )
    .expect("player one registration call should decode")
    .expect("player one registration should succeed");
    assert_eq!(registered_one.status, CommandStatus::Applied);
    let player_one_id = match registered_one.result.clone() {
        LobbyCommandResult::Player(player) => {
            assert_eq!(player.display_name, "Presence One");
            assert_eq!(player.principal, player_one);
            player.player_id
        }
        other => panic!("register_player returned unexpected result: {other:?}"),
    };

    let registered_one_replay = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "register_player",
        (
            Some("presence-one".to_string()),
            Some("Presence One".to_string()),
            "nonce:presence:register:one".to_string(),
        ),
    )
    .expect("register replay should decode")
    .expect("register replay should succeed");
    assert_eq!(registered_one_replay.command_id, registered_one.command_id);

    let register_mismatch = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "register_player",
        (
            Some("presence-one-renamed".to_string()),
            Some("Presence One".to_string()),
            "nonce:presence:register:one".to_string(),
        ),
    )
    .expect("register mismatch should decode")
    .expect("register mismatch should return a command response");
    assert_eq!(register_mismatch.status, CommandStatus::Failed);
    assert_eq!(
        register_mismatch
            .error
            .expect("mismatch should carry error")
            .code,
        "duplicate_nonce_payload_mismatch"
    );

    let registered_two = update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "register_player",
        (
            Some("presence-two".to_string()),
            Some("Presence Two".to_string()),
            "nonce:presence:register:two".to_string(),
        ),
    )
    .expect("player two registration call should decode")
    .expect("player two registration should succeed");
    assert_eq!(registered_two.status, CommandStatus::Applied);

    let my_player = query_as::<PlayerView>(&fixture, player_one, "get_my_player", ())
        .expect("get_my_player should decode")
        .expect("player one should be readable");
    assert_eq!(my_player.player_id, player_one_id);

    let created = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "create_session",
        (
            "Presence Match".to_string(),
            FIRST_PLAYABLE_RULESET_ID.to_string(),
            1_u64,
            "nonce:presence:create".to_string(),
        ),
    )
    .expect("create_session should decode")
    .expect("create_session should succeed");
    let session_id = match created.result.clone() {
        LobbyCommandResult::Session(session) => {
            assert_eq!(session.state, "lobby");
            assert_eq!(session.participant_ids.len(), 1);
            session.session_id
        }
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    let fetched_session =
        query_as::<SessionView>(&fixture, player_one, "get_session", (session_id.clone(),))
            .expect("get_session should decode")
            .expect("created session should be readable");
    assert_eq!(fetched_session.session_id, session_id);

    let joined = update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "join_session",
        (
            session_id.clone(),
            "faction:ashen-ledger".to_string(),
            "nonce:presence:join".to_string(),
        ),
    )
    .expect("join_session should decode")
    .expect("join_session should succeed");
    assert_eq!(joined.status, CommandStatus::Applied);

    update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "mark_ready",
        (session_id.clone(), "nonce:presence:ready:one".to_string()),
    )
    .expect("player one ready should decode")
    .expect("player one ready should succeed");
    update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "mark_ready",
        (session_id.clone(), "nonce:presence:ready:two".to_string()),
    )
    .expect("player two ready should decode")
    .expect("player two ready should succeed");

    let unauthorized_start = update_as::<LobbyCommandResponse>(
        &fixture,
        player_two,
        "start_session",
        (session_id.clone(), "nonce:presence:start:wrong".to_string()),
    )
    .expect("unauthorized start should decode")
    .expect("unauthorized start should return a command response");
    assert_eq!(unauthorized_start.status, CommandStatus::Failed);
    assert_eq!(
        unauthorized_start
            .error
            .expect("unauthorized start should carry error")
            .code,
        "not_session_creator"
    );

    let mut active_start = None;
    for step in 0..18 {
        let nonce = format!("nonce:presence:start:{step}");
        let started = update_as::<LobbyCommandResponse>(
            &fixture,
            player_one,
            "start_session",
            (session_id.clone(), nonce.clone()),
        )
        .unwrap_or_else(|error| panic!("start_session step {step} should decode: {error:?}"))
        .unwrap_or_else(|error| panic!("start_session step {step} should succeed: {error:?}"));
        assert_eq!(started.status, CommandStatus::Applied);
        let state = match &started.result {
            LobbyCommandResult::Session(session) => session.state.as_str(),
            other => panic!("start_session returned unexpected result: {other:?}"),
        };
        if state == "active" {
            active_start = Some((started, nonce));
            break;
        }
    }
    let (active_start, active_start_nonce) =
        active_start.expect("phased start_session should finish setup");

    let participant_one = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("get_my_participant should decode")
    .expect("participant one should be readable");
    assert_eq!(participant_one.slot_index, 0);

    let participant_two = query_as::<ParticipantView>(
        &fixture,
        player_two,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("get_my_participant should decode")
    .expect("participant two should be readable");
    assert_eq!(participant_two.slot_index, 1);
    assert!(participant_two.ready);

    let history =
        query_as::<MatchHistoryPage>(&fixture, player_one, "get_match_history", (0_u32, 10_u32))
            .expect("get_match_history should decode")
            .expect("pending match shells should not appear in history yet");
    assert!(history.entries.is_empty());

    let manifest = query_as::<ContentManifestResponse>(
        &fixture,
        player_one,
        "get_content_manifest",
        (FIRST_PLAYABLE_RULESET_SLUG.to_string(), 1_u32),
    )
    .expect("get_content_manifest should decode")
    .expect("content manifest should be backed by seeded rows");
    assert_eq!(manifest.manifest.ruleset.slug, FIRST_PLAYABLE_RULESET_SLUG);
    assert_eq!(manifest.manifest.ruleset.content_manifest_hash.len(), 64);

    let chunk_page = query_as::<MapChunkPage>(
        &fixture,
        player_one,
        "get_visible_map_chunks",
        (session_id.clone(), viewport.clone(), None::<u32>, 8_u32),
    )
    .expect("get_visible_map_chunks should decode")
    .expect("visible map chunks should load from IcyDB rows");
    assert_eq!(chunk_page.chunks.len(), 4);
    assert!(!chunk_page.has_more);
    let remote_viewport = opening_viewport_for_slot(1);
    let surveyed_remote_chunks = query_as::<MapChunkPage>(
        &fixture,
        player_one,
        "get_visible_map_chunks",
        (
            session_id.clone(),
            remote_viewport.clone(),
            None::<u32>,
            8_u32,
        ),
    )
    .expect("surveyed remote map chunks should decode")
    .expect("surveyed base-map chunks should be public static data");
    assert!(!surveyed_remote_chunks.chunks.is_empty());
    assert!(surveyed_remote_chunks.chunks.iter().all(|chunk| {
        !chunk.terrain_blob.is_empty()
            && !chunk.movement_blob.is_empty()
            && !chunk.flags_blob.is_empty()
    }));
    assert!(
        surveyed_remote_chunks
            .chunks
            .iter()
            .all(|chunk| chunk.visible_blob.iter().all(|byte| *byte == 0))
    );

    let object_page = query_as::<ObjectViewPage>(
        &fixture,
        player_one,
        "get_visible_objects",
        (session_id.clone(), viewport.clone(), None::<u32>, 128_u32),
    )
    .expect("get_visible_objects should decode")
    .expect("visible objects should load from IcyDB rows");
    assert!(object_page.objects.iter().any(|object| {
        object.subject_kind == "champion"
            && object.display_name.as_deref() == Some("Mara of the Toll")
    }));
    assert!(object_page.objects.iter().any(|object| {
        object.subject_kind == "town" && object.display_name.as_deref() == Some("West Woe")
    }));
    assert!(
        object_page
            .objects
            .iter()
            .any(|object| object.subject_id_text == "pile:west-wood-1")
    );
    assert!(
        object_page
            .objects
            .iter()
            .any(|object| object.subject_id_text == "neutral:west-mine")
    );
    assert!(
        object_page
            .objects
            .iter()
            .any(|object| object.subject_id_text == "dwelling:west-mudhook")
    );
    assert!(
        object_page
            .objects
            .iter()
            .all(|object| object.subject_id_text != "champion:east")
    );
    let hidden_remote_objects = query_as::<ObjectViewPage>(
        &fixture,
        player_one,
        "get_visible_objects",
        (session_id.clone(), remote_viewport, None::<u32>, 128_u32),
    )
    .expect("remote object page should decode")
    .expect("remote dynamic object page should load without leaking hidden state");
    assert!(
        hidden_remote_objects
            .objects
            .iter()
            .all(|object| object.subject_id_text != "champion:east"
                && object.display_name.as_deref() != Some("East Woe"))
    );

    let object_view = query_as::<ObjectView>(
        &fixture,
        player_one,
        "get_object_view",
        (
            session_id.clone(),
            "world_object".to_string(),
            "pile:west-wood-1".to_string(),
        ),
    )
    .expect("get_object_view should decode")
    .expect("known object detail should load");
    assert_eq!(object_view.subject_id_text, "pile:west-wood-1");
    assert_eq!(object_view.visibility, "visible");

    let hidden_object = query_as::<ObjectView>(
        &fixture,
        player_one,
        "get_object_view",
        (
            session_id.clone(),
            "champion".to_string(),
            "champion:east".to_string(),
        ),
    )
    .expect("hidden get_object_view should decode")
    .expect_err("hidden object detail should fail without leaking state");
    assert_eq!(hidden_object.code, "not_visible");

    let champions = query_as::<Vec<ChampionView>>(
        &fixture,
        player_one,
        "get_my_champions",
        (session_id.clone(),),
    )
    .expect("get_my_champions should decode")
    .expect("champions should load from IcyDB rows");
    assert_eq!(champions.len(), 1);
    let champion_id = champions[0].champion_id.clone();
    assert_eq!(champions[0].name.as_deref(), Some("Mara of the Toll"));
    assert!(
        champions[0].army_stacks.is_empty(),
        "get_my_champions stays a bounded list; get_champion_view returns stack detail"
    );

    let champion = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("get_champion_view should decode")
    .expect("own champion should be visible");
    assert_eq!(champion.champion_id, champion_id);
    assert_eq!(champion.army_stacks.len(), 2);
    assert_eq!(champion.skill_points, 1);

    let progression = query_as::<ChampionProgressionView>(
        &fixture,
        player_one,
        "preview_champion_progression",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("preview_champion_progression should decode")
    .expect("champion progression should be readable");
    assert_eq!(progression.champion_id, champion_id);
    assert!(
        progression
            .level_up_choices
            .iter()
            .any(|choice| choice.skill_key == "sour_sorcery" && choice.enabled)
    );

    let skill_choice = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "select_champion_level_up",
        (
            session_id.clone(),
            champion_id.clone(),
            "sour_sorcery".to_string(),
            "nonce:presence:skill:sour".to_string(),
        ),
    )
    .expect("select_champion_level_up should decode")
    .expect("skill choice should succeed");
    assert_eq!(skill_choice.status, CommandStatus::Applied);
    assert!(matches!(
        skill_choice.result,
        CommandResult::ChampionMagic(_)
    ));
    let skill_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "select_champion_level_up",
        (
            session_id.clone(),
            champion_id.clone(),
            "sour_sorcery".to_string(),
            "nonce:presence:skill:sour".to_string(),
        ),
    )
    .expect("select_champion_level_up replay should decode")
    .expect("skill choice replay should succeed");
    assert_eq!(skill_replay.command_id, skill_choice.command_id);

    let learned_hex = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "learn_champion_spell",
        (
            session_id.clone(),
            champion_id.clone(),
            "hex-spark".to_string(),
            "nonce:presence:learn:hex".to_string(),
        ),
    )
    .expect("learn_champion_spell hex should decode")
    .expect("hex-spark learning should succeed");
    assert_eq!(learned_hex.status, CommandStatus::Applied);

    let learned_march = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "learn_champion_spell",
        (
            session_id.clone(),
            champion_id.clone(),
            "spite-march".to_string(),
            "nonce:presence:learn:march".to_string(),
        ),
    )
    .expect("learn_champion_spell march should decode")
    .expect("spite-march learning should succeed");
    assert_eq!(learned_march.status, CommandStatus::Applied);

    let cast_march = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "cast_adventure_spell",
        (
            session_id.clone(),
            champion_id.clone(),
            "spite-march".to_string(),
            "nonce:presence:cast:march".to_string(),
        ),
    )
    .expect("cast_adventure_spell should decode")
    .expect("spite-march cast should succeed");
    assert_eq!(cast_march.status, CommandStatus::Applied);

    let progressed = query_as::<ChampionProgressionView>(
        &fixture,
        player_one,
        "preview_champion_progression",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("progression after magic should decode")
    .expect("progression after magic should be readable");
    assert!(progressed.skill_keys.contains(&"sour_sorcery".to_string()));
    assert!(
        progressed
            .learned_spell_slugs
            .contains(&"hex-spark".to_string())
    );
    assert!(
        progressed
            .learned_spell_slugs
            .contains(&"spite-march".to_string())
    );
    assert_eq!(progressed.mana, 8);

    let champion_after_learning = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("champion view after learning should decode")
    .expect("own champion after learning should be visible");
    assert!(
        champion_after_learning
            .spell_slugs
            .contains(&"hex-spark".to_string())
    );
    assert!(
        champion_after_learning
            .spell_slugs
            .contains(&"spite-march".to_string())
    );

    let cast_march_again = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "cast_adventure_spell",
        (
            session_id.clone(),
            champion_id.clone(),
            "spite-march".to_string(),
            "nonce:presence:cast:march:again".to_string(),
        ),
    )
    .expect("second cast_adventure_spell should decode")
    .expect("second spite-march cast should succeed");
    assert_eq!(cast_march_again.status, CommandStatus::Applied);
    let spell_event_page = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 50_u32),
    )
    .expect("events after repeated adventure casts should decode")
    .expect("events after repeated adventure casts should load");
    let adventure_casts = spell_event_page
        .events
        .iter()
        .filter(|event| event.event_type == "adventure_spell_cast")
        .collect::<Vec<_>>();
    assert_eq!(
        adventure_casts.len(),
        2,
        "each adventure spell cast should append a distinct public event"
    );
    assert_ne!(adventure_casts[0].event_seq, adventure_casts[1].event_seq);
    assert_ne!(adventure_casts[0].event_key, adventure_casts[1].event_key);

    let game_view = query_as::<GameView>(
        &fixture,
        player_one,
        "get_game_view",
        (
            session_id.clone(),
            GameViewRequest {
                viewport: viewport.clone(),
                chunk_cursor: None,
                chunk_limit: 1,
                object_cursor: None,
                object_limit: 1,
                events_after_seq: 0,
                event_limit: 1,
                include_battle: false,
            },
        ),
    )
    .expect("get_game_view should decode")
    .expect("game view should load from IcyDB rows");
    assert!(game_view.map_chunks.is_empty());
    assert!(!game_view.map_page_info.has_more);
    assert!(game_view.map_page_info.next_cursor.is_none());
    assert!(game_view.objects.is_empty());
    assert!(!game_view.object_page_info.has_more);
    assert!(game_view.object_page_info.next_cursor.is_none());
    assert!(game_view.omitted_fields.contains(&"map_chunks".to_string()));
    assert!(game_view.omitted_fields.contains(&"objects".to_string()));
    assert!(
        !game_view
            .action_affordances
            .iter()
            .any(|action| action.action == "sync_session_turn"
                || action.action == "sync_world_generation")
    );
    assert!(
        game_view
            .events
            .iter()
            .any(|event| event.event_type == "session_started")
    );
    let town_id = "town:west".to_string();

    let town = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("get_town_view should decode")
    .expect("own town should be visible");
    assert_eq!(town.town.name, "West Woe");
    assert!(
        town.buildings
            .iter()
            .any(|building| building.building_slug == "crumbling-hall")
    );

    let event_page = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 10_u32),
    )
    .expect("get_events_after should decode")
    .expect("public event feed should load from IcyDB rows");
    assert_eq!(event_page.page_info.limit, 10);
    assert!(
        event_page
            .events
            .iter()
            .any(|event| event.event_type == "session_started")
    );

    let status_by_id = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status",
        (session_id.clone(), active_start.command_id.clone()),
    )
    .expect("get_command_status by id should decode")
    .expect("start command status should be readable by id");
    assert_eq!(status_by_id.status, CommandStatus::Applied);
    assert_eq!(status_by_id.command_id, active_start.command_id);

    let status_by_nonce = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status",
        (session_id.clone(), active_start_nonce.clone()),
    )
    .expect("get_command_status by nonce should decode")
    .expect("start command status should be readable by nonce");
    assert_eq!(status_by_nonce.status, CommandStatus::Applied);

    let status_by_typed_nonce = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status_by_nonce",
        (
            session_id.clone(),
            "start_session".to_string(),
            active_start_nonce,
        ),
    )
    .expect("get_command_status_by_nonce should decode")
    .expect("start command status should be readable by typed nonce");
    assert_eq!(status_by_typed_nonce.status, CommandStatus::Applied);
    assert_eq!(status_by_typed_nonce.command_id, active_start.command_id);

    let objectives = query_as::<ObjectiveProgressView>(
        &fixture,
        player_one,
        "get_objective_progress",
        (session_id.clone(),),
    )
    .expect("get_objective_progress should decode")
    .expect("objective progress should load from IcyDB rows");
    assert_eq!(objectives.objectives.len(), 2);
    assert!(
        objectives
            .objectives
            .iter()
            .any(|objective| objective.objective_key == "objective:north")
    );

    let rules = query_as::<ScenarioRulesView>(
        &fixture,
        player_one,
        "get_scenario_rules",
        (session_id.clone(),),
    )
    .expect("get_scenario_rules should decode")
    .expect("scenario rules should load from IcyDB rows");
    assert!(
        rules
            .rules
            .iter()
            .any(|rule| rule.rule_key == "rule:conquest" && rule.victory_state == "active")
    );
    assert!(rules.rules.iter().any(|rule| {
        rule.rule_key == "rule:artifact-victory"
            && rule.status == "disabled"
            && rule.disabled_reason.as_deref() == Some("checkpoint_24_schema_only")
    }));

    let skirmish = query_as::<SkirmishSettingsView>(
        &fixture,
        player_one,
        "get_skirmish_settings",
        (session_id.clone(),),
    )
    .expect("get_skirmish_settings should decode")
    .expect("skirmish settings should load from IcyDB rows");
    assert_eq!(
        skirmish.settings.profile_key,
        "skirmish:first-playable-compact"
    );
    assert_eq!(skirmish.settings.generation_key, PROCEDURAL_GENERATION_KEY);
    assert!(skirmish.settings.fog_enabled);
    assert!(!skirmish.settings.naval_enabled);
    assert!(!skirmish.settings.siege_enabled);
    assert!(!skirmish.settings.larger_map_enabled);

    let procedural = query_as::<ProceduralMapView>(
        &fixture,
        player_one,
        "get_procedural_map_state",
        (session_id.clone(),),
    )
    .expect("get_procedural_map_state should decode")
    .expect("procedural map state should load from IcyDB rows");
    assert_eq!(procedural.maps.len(), 1);
    let procedural_map = &procedural.maps[0];
    assert_eq!(procedural_map.generation_key, PROCEDURAL_GENERATION_KEY);
    assert_eq!(procedural_map.status, "validated");
    assert_eq!(procedural_map.chunk_count, 9);
    assert!(procedural_map.water_tile_count > 0);
    assert!(!procedural_map.scenario_hash.is_empty());

    let naval_routes = query_as::<NavalRoutesView>(
        &fixture,
        player_one,
        "get_naval_routes",
        (session_id.clone(),),
    )
    .expect("get_naval_routes should decode")
    .expect("naval route rows should load from IcyDB rows");
    assert_eq!(naval_routes.routes.len(), 1);
    assert_eq!(naval_routes.routes[0].status, "disabled");
    assert!(!naval_routes.routes[0].actionable);
    assert_eq!(
        naval_routes.routes[0].disabled_reason.as_deref(),
        Some("checkpoint_25_schema_only")
    );

    let siege_rules = query_as::<SiegeRulesView>(
        &fixture,
        player_one,
        "get_siege_rules",
        (session_id.clone(),),
    )
    .expect("get_siege_rules should decode")
    .expect("siege rule rows should load from IcyDB rows");
    assert_eq!(siege_rules.rules.len(), 1);
    assert_eq!(siege_rules.rules[0].status, "disabled");
    assert!(!siege_rules.rules[0].actionable);
    assert_eq!(
        siege_rules.rules[0].disabled_reason.as_deref(),
        Some("checkpoint_25_schema_only")
    );

    let world_events = query_as::<WorldEventsView>(
        &fixture,
        player_one,
        "get_world_events",
        (session_id.clone(),),
    )
    .expect("get_world_events should decode")
    .expect("world events should load from IcyDB rows");
    assert_eq!(world_events.events.len(), 1);
    assert_eq!(world_events.events[0].event_window, "week:1");

    let quest_preview = query_as::<QuestPreview>(
        &fixture,
        player_one,
        "preview_quest",
        (session_id.clone(), OPENING_QUEST_KEY.to_string()),
    )
    .expect("preview_quest should decode")
    .expect("quest preview should load from IcyDB rows");
    assert!(quest_preview.can_accept);
    assert_eq!(quest_preview.quest.reward_gold, Some(500));

    let accepted_quest = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "accept_quest",
        (
            session_id.clone(),
            OPENING_QUEST_KEY.to_string(),
            "nonce:presence:quest:accept".to_string(),
        ),
    )
    .expect("accept_quest should decode")
    .expect("accept_quest should succeed");
    assert_eq!(accepted_quest.status, CommandStatus::Applied);
    assert!(matches!(
        accepted_quest.result,
        CommandResult::AdvancedScenario(_)
    ));
    let accepted_quest_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "accept_quest",
        (
            session_id.clone(),
            OPENING_QUEST_KEY.to_string(),
            "nonce:presence:quest:accept".to_string(),
        ),
    )
    .expect("accept_quest replay should decode")
    .expect("accept_quest replay should succeed");
    assert_eq!(accepted_quest_replay.command_id, accepted_quest.command_id);

    let synced_objectives = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_objectives",
        (
            session_id.clone(),
            "nonce:presence:objective:sync".to_string(),
        ),
    )
    .expect("sync_objectives should decode")
    .expect("sync_objectives should succeed");
    assert_eq!(synced_objectives.status, CommandStatus::Applied);
    let synced_objectives_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_objectives",
        (
            session_id.clone(),
            "nonce:presence:objective:sync".to_string(),
        ),
    )
    .expect("sync_objectives replay should decode")
    .expect("sync_objectives replay should succeed");
    assert_eq!(
        synced_objectives_replay.command_id,
        synced_objectives.command_id
    );

    let synced_world_events = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_world_events",
        (
            session_id.clone(),
            "nonce:presence:world-event:sync".to_string(),
        ),
    )
    .expect("sync_world_events should decode")
    .expect("sync_world_events should succeed");
    assert_eq!(synced_world_events.status, CommandStatus::Applied);
    let synced_world_events_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_world_events",
        (
            session_id.clone(),
            "nonce:presence:world-event:sync".to_string(),
        ),
    )
    .expect("sync_world_events replay should decode")
    .expect("sync_world_events replay should succeed");
    assert_eq!(
        synced_world_events_replay.command_id,
        synced_world_events.command_id
    );

    let synced_victory = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_advanced_victory",
        (
            session_id.clone(),
            "nonce:presence:victory:sync".to_string(),
        ),
    )
    .expect("sync_advanced_victory should decode")
    .expect("sync_advanced_victory should succeed");
    assert_eq!(synced_victory.status, CommandStatus::Applied);
    let synced_victory_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_advanced_victory",
        (
            session_id.clone(),
            "nonce:presence:victory:sync".to_string(),
        ),
    )
    .expect("sync_advanced_victory replay should decode")
    .expect("sync_advanced_victory replay should succeed");
    assert_eq!(synced_victory_replay.command_id, synced_victory.command_id);

    let synced_worldgen = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_world_generation",
        (
            session_id.clone(),
            "nonce:presence:worldgen:sync".to_string(),
        ),
    )
    .expect("sync_world_generation should decode")
    .expect("sync_world_generation should succeed");
    assert_eq!(synced_worldgen.status, CommandStatus::Applied);
    assert!(synced_worldgen.events.is_empty());
    match &synced_worldgen.result {
        CommandResult::WorldGeneration(receipt) => {
            assert_eq!(receipt.action, "sync_world_generation");
            assert_eq!(receipt.generation_key, PROCEDURAL_GENERATION_KEY);
            assert_eq!(receipt.state, "validated");
            assert_eq!(receipt.chunk_count, 9);
            assert!(!receipt.scenario_hash.is_empty());
        }
        other => panic!("sync_world_generation returned unexpected result: {other:?}"),
    }
    let synced_worldgen_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_world_generation",
        (
            session_id.clone(),
            "nonce:presence:worldgen:sync".to_string(),
        ),
    )
    .expect("sync_world_generation replay should decode")
    .expect("sync_world_generation replay should succeed");
    assert_eq!(
        synced_worldgen_replay.command_id,
        synced_worldgen.command_id
    );
    assert!(synced_worldgen_replay.events.is_empty());

    let claimed_quest = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "claim_quest_reward",
        (
            session_id.clone(),
            OPENING_QUEST_KEY.to_string(),
            "nonce:presence:quest:claim".to_string(),
        ),
    )
    .expect("claim_quest_reward should decode")
    .expect("claim_quest_reward should succeed");
    assert_eq!(claimed_quest.status, CommandStatus::Applied);
    match &claimed_quest.result {
        CommandResult::AdvancedScenario(receipt) => {
            assert_eq!(receipt.action, "claim_quest_reward");
            assert_eq!(receipt.reward_gold, 500);
        }
        other => panic!("claim_quest_reward returned unexpected result: {other:?}"),
    }
    let claimed_quest_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "claim_quest_reward",
        (
            session_id.clone(),
            OPENING_QUEST_KEY.to_string(),
            "nonce:presence:quest:claim".to_string(),
        ),
    )
    .expect("claim_quest_reward replay should decode")
    .expect("claim_quest_reward replay should succeed");
    assert_eq!(claimed_quest_replay.command_id, claimed_quest.command_id);

    let quest_after_claim = query_as::<QuestPreview>(
        &fixture,
        player_one,
        "preview_quest",
        (session_id.clone(), OPENING_QUEST_KEY.to_string()),
    )
    .expect("preview_quest after claim should decode")
    .expect("quest preview after claim should load");
    assert_eq!(quest_after_claim.quest.status, "claimed");
    assert!(!quest_after_claim.can_claim);

    let accept_claimed_quest = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "accept_quest",
        (
            session_id.clone(),
            OPENING_QUEST_KEY.to_string(),
            "nonce:presence:quest:accept:claimed".to_string(),
        ),
    )
    .expect("accept claimed quest should decode")
    .expect("accept claimed quest should return a failed command response");
    assert_eq!(accept_claimed_quest.status, CommandStatus::Failed);
    assert!(!accept_claimed_quest.retryable);
    assert!(accept_claimed_quest.events.is_empty());
    assert_eq!(
        accept_claimed_quest
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("quest_already_claimed")
    );

    let movement_preview = query_as::<MovementPreview>(
        &fixture,
        player_one,
        "preview_move_path",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(champion.x.saturating_add(1), champion.y)],
        ),
    )
    .expect("preview_move_path should decode")
    .expect("movement preview should be typed and read-only");
    assert_eq!(movement_preview.champion_id, champion_id);
    assert_eq!(movement_preview.total_cost, 5);

    let build_preview = query_as::<BuildPreview>(
        &fixture,
        player_one,
        "preview_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
        ),
    )
    .expect("preview_build_town_structure should decode")
    .expect("build preview should be typed and read-only");
    assert!(build_preview.allowed);
    assert_eq!(build_preview.building_slug, "freehold-training-yard");

    let recruit_preview = query_as::<RecruitPreview>(
        &fixture,
        player_one,
        "preview_recruit_units",
        (
            session_id.clone(),
            town_id.clone(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
        ),
    )
    .expect("preview_recruit_units should decode")
    .expect("recruit preview should be typed and read-only");
    assert!(!recruit_preview.allowed);
    assert_eq!(
        recruit_preview.disabled_reason.as_deref(),
        Some("recruit_pool_empty")
    );

    let forbidden_events = query_as::<ApiEventPage>(
        &fixture,
        player_two,
        "get_events_after",
        (
            session_id.clone(),
            format!("participant:{}", participant_one.participant_id),
            0_u64,
            10_u32,
        ),
    )
    .expect("forbidden audience event query should decode")
    .expect_err("participant event audience should be private");
    assert_eq!(forbidden_events.code, "audience_not_allowed");

    let anonymous_map = query_as::<MapChunkPage>(
        &fixture,
        candid::Principal::anonymous(),
        "get_visible_map_chunks",
        (session_id.clone(), viewport.clone(), None::<u32>, 1_u32),
    )
    .expect("anonymous map query should decode")
    .expect_err("anonymous map query should fail with typed auth");
    assert_eq!(anonymous_map.code, "anonymous_not_allowed");

    let oversized_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (
            session_id.clone(),
            "public".to_string(),
            0_u64,
            MAX_LIST_LIMIT + 1,
        ),
    )
    .expect("oversized event query should decode")
    .expect_err("oversized event query should fail typed limit validation");
    assert_eq!(oversized_events.code, "list_limit_exceeded");

    let oversized_chunks = query_as::<MapChunkPage>(
        &fixture,
        player_one,
        "get_visible_map_chunks",
        (
            session_id.clone(),
            viewport.clone(),
            None::<u32>,
            MAX_CHUNK_LIMIT + 1,
        ),
    )
    .expect("oversized chunk query should decode")
    .expect_err("oversized chunk query should fail typed limit validation");
    assert_eq!(oversized_chunks.code, "viewport_chunk_limit_exceeded");

    let oversized_objects = query_as::<ObjectViewPage>(
        &fixture,
        player_one,
        "get_visible_objects",
        (
            session_id.clone(),
            viewport.clone(),
            None::<u32>,
            MAX_OBJECT_LIMIT + 1,
        ),
    )
    .expect("oversized object query should decode")
    .expect_err("oversized object query should fail typed limit validation");
    assert_eq!(oversized_objects.code, "list_limit_exceeded");

    let oversized_history = query_as::<MatchHistoryPage>(
        &fixture,
        player_one,
        "get_match_history",
        (0_u32, MAX_LIST_LIMIT + 1),
    )
    .expect("oversized history query should decode")
    .expect_err("oversized history query should fail typed limit validation");
    assert_eq!(oversized_history.code, "list_limit_exceeded");

    let oversized_game_view = query_as::<GameView>(
        &fixture,
        player_one,
        "get_game_view",
        (
            session_id.clone(),
            GameViewRequest {
                viewport: viewport.clone(),
                chunk_cursor: None,
                chunk_limit: MAX_CHUNK_LIMIT + 1,
                object_cursor: None,
                object_limit: 4,
                events_after_seq: 0,
                event_limit: 10,
                include_battle: false,
            },
        ),
    )
    .expect("oversized game view query should decode")
    .expect_err("oversized game view query should fail typed limit validation");
    assert_eq!(oversized_game_view.code, "viewport_chunk_limit_exceeded");

    let oversized_path = query_as::<MovementPreview>(
        &fixture,
        player_one,
        "preview_move_path",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(champion.x, champion.y); MAX_MOVE_PATH_STEPS_LIMIT + 1],
        ),
    )
    .expect("oversized movement preview should decode")
    .expect_err("oversized movement preview should fail typed limit validation");
    assert_eq!(oversized_path.code, "movement_path_too_long");

    let move_path = vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)];
    let moved = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            move_path.clone(),
            "nonce:presence:move:wood".to_string(),
        ),
    )
    .expect("submit_move_intent should decode")
    .expect("submit_move_intent should succeed");
    assert_eq!(moved.status, CommandStatus::Applied);

    let moved_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            move_path,
            "nonce:presence:move:wood".to_string(),
        ),
    )
    .expect("submit_move_intent replay should decode")
    .expect("submit_move_intent replay should succeed");
    assert_eq!(moved_replay.command_id, moved.command_id);

    let move_mismatch = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(9, 24)],
            "nonce:presence:move:wood".to_string(),
        ),
    )
    .expect("submit_move_intent mismatch should decode")
    .expect("submit_move_intent mismatch should return command response");
    assert_eq!(move_mismatch.status, CommandStatus::Failed);
    assert_eq!(
        move_mismatch
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("duplicate_nonce_payload_mismatch")
    );

    let move_status = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status",
        (session_id.clone(), "nonce:presence:move:wood".to_string()),
    )
    .expect("move command status should decode")
    .expect("move command should be readable by nonce");
    assert_eq!(move_status.status, CommandStatus::Applied);

    let move_status_by_typed_nonce = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status_by_nonce",
        (
            session_id.clone(),
            "submit_move_intent".to_string(),
            "nonce:presence:move:wood".to_string(),
        ),
    )
    .expect("move command typed nonce status should decode")
    .expect("move command should be readable by typed nonce");
    assert_eq!(move_status_by_typed_nonce.status, CommandStatus::Applied);
    assert_eq!(move_status_by_typed_nonce.command_id, moved.command_id);

    let (synced, _) = sync_until_event(
        &fixture,
        player_one,
        &session_id,
        "nonce:presence:sync-turn:wood:",
        61_000,
        "resource_picked_up",
        4,
    );
    assert_eq!(synced.status, CommandStatus::Applied);

    let champion_after_move = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("moved champion query should decode")
    .expect("moved champion should be visible");
    assert_eq!((champion_after_move.x, champion_after_move.y), (9, 23));

    let tavern_offers = query_as::<TavernOffersView>(
        &fixture,
        player_one,
        "get_tavern_offers",
        (session_id.clone(), town_id.clone()),
    )
    .expect("get_tavern_offers should decode")
    .expect("tavern offers should load from IcyDB rows");
    assert_eq!(tavern_offers.week_number, 1);
    assert_eq!(tavern_offers.offers.len(), 2);
    let tavern_offer = tavern_offers.offers[0].clone();
    assert_eq!(tavern_offer.status, "available");

    let hire_preview = query_as::<ChampionHirePreview>(
        &fixture,
        player_one,
        "preview_hire_champion",
        (
            session_id.clone(),
            town_id.clone(),
            tavern_offer.offer_key.clone(),
        ),
    )
    .expect("preview_hire_champion should decode")
    .expect("hire preview should be typed and read-only");
    assert!(hire_preview.allowed);
    assert_eq!(hire_preview.cost.gold, u64::from(tavern_offer.cost_gold));

    let hired = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "hire_tavern_champion",
        (
            session_id.clone(),
            town_id.clone(),
            tavern_offer.offer_key.clone(),
            "nonce:presence:hire:tavern".to_string(),
        ),
    )
    .expect("hire_tavern_champion should decode")
    .expect("hire_tavern_champion should succeed");
    assert_eq!(hired.status, CommandStatus::Applied);
    let hired_champion_id = match &hired.result {
        CommandResult::ExpandedEconomy(receipt) => {
            assert_eq!(receipt.action, "hire_tavern_champion");
            assert_eq!(
                receipt.offer_key.as_deref(),
                Some(tavern_offer.offer_key.as_str())
            );
            receipt
                .champion_id
                .clone()
                .expect("hire receipt should include champion id")
        }
        other => panic!("hire_tavern_champion returned unexpected result: {other:?}"),
    };
    assert!(!hired_champion_id.is_empty());
    let hired_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "hire_tavern_champion",
        (
            session_id.clone(),
            town_id.clone(),
            tavern_offer.offer_key.clone(),
            "nonce:presence:hire:tavern".to_string(),
        ),
    )
    .expect("hire_tavern_champion replay should decode")
    .expect("hire_tavern_champion replay should succeed");
    assert_eq!(hired_replay.command_id, hired.command_id);

    let tavern_after_hire = query_as::<TavernOffersView>(
        &fixture,
        player_one,
        "get_tavern_offers",
        (session_id.clone(), town_id.clone()),
    )
    .expect("get_tavern_offers after hire should decode")
    .expect("tavern offers after hire should load");
    assert!(
        tavern_after_hire
            .offers
            .iter()
            .any(|offer| offer.offer_key == tavern_offer.offer_key
                && offer.status == "hired"
                && offer.hired_champion_id.as_deref() == Some(hired_champion_id.as_str()))
    );

    let market_preview = query_as::<MarketTradePreview>(
        &fixture,
        player_one,
        "preview_market_trade",
        (
            session_id.clone(),
            "gold".to_string(),
            "crystal".to_string(),
            2_500_u64,
        ),
    )
    .expect("preview_market_trade should decode")
    .expect("market preview should be typed and read-only");
    assert!(market_preview.allowed);
    assert_eq!(market_preview.amount_out, 1);
    assert_eq!(market_preview.rate_key, "gold_to_rare_2500_1");

    let oversized_market = query_as::<MarketTradePreview>(
        &fixture,
        player_one,
        "preview_market_trade",
        (
            session_id.clone(),
            "gold".to_string(),
            "crystal".to_string(),
            MARKET_TRADE_MAX_INPUT + 1,
        ),
    )
    .expect("oversized market preview should decode")
    .expect_err("oversized market preview should fail typed limit validation");
    assert_eq!(oversized_market.code, "invalid_market_trade");

    let traded = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_market_trade",
        (
            session_id.clone(),
            "gold".to_string(),
            "crystal".to_string(),
            2_500_u64,
            "nonce:presence:market:gold-crystal".to_string(),
        ),
    )
    .expect("submit_market_trade should decode")
    .expect("submit_market_trade should succeed");
    assert_eq!(traded.status, CommandStatus::Applied);
    match &traded.result {
        CommandResult::ExpandedEconomy(receipt) => {
            assert_eq!(receipt.action, "submit_market_trade");
            assert_eq!(receipt.from_resource.as_deref(), Some("gold"));
            assert_eq!(receipt.to_resource.as_deref(), Some("crystal"));
            assert_eq!(receipt.amount_out, 1);
        }
        other => panic!("submit_market_trade returned unexpected result: {other:?}"),
    }
    let traded_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_market_trade",
        (
            session_id.clone(),
            "gold".to_string(),
            "crystal".to_string(),
            2_500_u64,
            "nonce:presence:market:gold-crystal".to_string(),
        ),
    )
    .expect("submit_market_trade replay should decode")
    .expect("submit_market_trade replay should succeed");
    assert_eq!(traded_replay.command_id, traded.command_id);

    let dwelling_id = "dwelling:west-mudhook".to_string();
    let dwelling_pool = query_as::<DwellingPoolView>(
        &fixture,
        player_one,
        "get_dwelling_pool",
        (session_id.clone(), dwelling_id.clone()),
    )
    .expect("get_dwelling_pool should decode")
    .expect("dwelling pool should load from IcyDB rows");
    assert_eq!(dwelling_pool.unit_slug, "mudhook-levy");
    assert!(dwelling_pool.direct_recruit);
    assert!(dwelling_pool.available >= 4);
    let dwelling_object_id = dwelling_pool.object_id.clone();

    let enemy_champions = query_as::<Vec<ChampionView>>(
        &fixture,
        player_two,
        "get_my_champions",
        (session_id.clone(),),
    )
    .expect("player two champions should decode")
    .expect("player two champions should load from IcyDB rows");
    let enemy_champion_id = enemy_champions
        .first()
        .expect("player two should have an opening champion")
        .champion_id
        .clone();
    let enemy_dwelling_preview = query_as::<DwellingRecruitPreview>(
        &fixture,
        player_one,
        "preview_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            enemy_champion_id.clone(),
        ),
    )
    .expect("enemy dwelling recruit preview should decode")
    .expect("enemy dwelling recruit preview should return a typed denial");
    assert!(!enemy_dwelling_preview.allowed);
    assert_eq!(
        enemy_dwelling_preview.disabled_reason.as_deref(),
        Some("champion_not_owned")
    );
    let enemy_dwelling_nonce = "nonce:presence:dwelling:enemy-target".to_string();
    let enemy_dwelling_submit = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            enemy_champion_id,
            enemy_dwelling_nonce.clone(),
        ),
    )
    .expect("enemy dwelling recruit submit should decode")
    .expect_err("enemy dwelling recruit should fail before command creation");
    assert_eq!(enemy_dwelling_submit.code, "champion_not_owned");
    let enemy_dwelling_status = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status_by_nonce",
        (
            session_id.clone(),
            "submit_dwelling_recruit".to_string(),
            enemy_dwelling_nonce,
        ),
    )
    .expect("enemy dwelling recruit status lookup should decode")
    .expect_err("pre-command dwelling denial should not leave a command row");
    assert_eq!(enemy_dwelling_status.code, "command_status_not_found");

    let dwelling_preview = query_as::<DwellingRecruitPreview>(
        &fixture,
        player_one,
        "preview_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            champion_id.clone(),
        ),
    )
    .expect("preview_dwelling_recruit should decode")
    .expect("dwelling recruit preview should be typed and read-only");
    assert!(dwelling_preview.allowed);
    assert_eq!(dwelling_preview.available, dwelling_pool.available);

    let dwelling_recruit = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            champion_id.clone(),
            "nonce:presence:dwelling:recruit".to_string(),
        ),
    )
    .expect("submit_dwelling_recruit should decode")
    .expect("submit_dwelling_recruit should succeed");
    assert_eq!(dwelling_recruit.status, CommandStatus::Applied);
    match &dwelling_recruit.result {
        CommandResult::ExpandedEconomy(receipt) => {
            assert_eq!(receipt.action, "submit_dwelling_recruit");
            assert_eq!(
                receipt.object_id.as_deref(),
                Some(dwelling_object_id.as_str())
            );
            assert_eq!(receipt.unit_slug.as_deref(), Some("mudhook-levy"));
            assert_eq!(receipt.quantity, 1);
        }
        other => panic!("submit_dwelling_recruit returned unexpected result: {other:?}"),
    }
    let dwelling_recruit_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            champion_id.clone(),
            "nonce:presence:dwelling:recruit".to_string(),
        ),
    )
    .expect("submit_dwelling_recruit replay should decode")
    .expect("submit_dwelling_recruit replay should succeed");
    assert_eq!(
        dwelling_recruit_replay.command_id,
        dwelling_recruit.command_id
    );

    let dwelling_after_recruit = query_as::<DwellingPoolView>(
        &fixture,
        player_one,
        "get_dwelling_pool",
        (session_id.clone(), dwelling_id.clone()),
    )
    .expect("get_dwelling_pool after recruit should decode")
    .expect("dwelling pool after recruit should load");
    assert_eq!(
        dwelling_after_recruit.available,
        dwelling_pool.available.saturating_sub(1)
    );

    let participant_after_pickup = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after pickup should decode")
    .expect("participant after pickup should be readable");
    assert!(participant_after_pickup.resources.wood > participant_one.resources.wood);

    let built = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
            "nonce:presence:build:yard".to_string(),
        ),
    )
    .expect("submit_build_town_structure should decode")
    .expect("submit_build_town_structure should succeed");
    assert_eq!(built.status, CommandStatus::Applied);
    assert!(built.events.iter().any(|event| {
        event.event_type == "town_building_built"
            && event.audience_key.starts_with("participant:")
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("freehold-training-yard"))
    }));
    assert!(built.events.iter().any(|event| {
        event.event_type == "town_building_built"
            && event.audience_key == "public"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""redacted":true"#))
    }));

    let town_after_build = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after build should decode")
    .expect("town after build should be visible");
    assert!(
        town_after_build
            .buildings
            .iter()
            .any(|building| building.building_slug == "freehold-training-yard")
    );
    assert!(
        town_after_build
            .recruit_pools
            .iter()
            .any(|pool| pool.unit_slug == "mudhook-levy" && pool.available > 0)
    );

    let recruited = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_recruit_units",
        (
            session_id.clone(),
            town_id.clone(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
            "nonce:presence:recruit:levy".to_string(),
        ),
    )
    .expect("submit_recruit_units should decode")
    .expect("submit_recruit_units should succeed");
    assert_eq!(recruited.status, CommandStatus::Applied);
    assert!(recruited.events.iter().any(|event| {
        event.event_type == "units_recruited"
            && event.audience_key.starts_with("participant:")
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("mudhook-levy"))
    }));
    assert!(recruited.events.iter().any(|event| {
        event.event_type == "units_recruited"
            && event.audience_key == "public"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""redacted":true"#))
    }));

    let town_after_recruit = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after recruit should decode")
    .expect("town after recruit should be visible");
    assert!(
        town_after_recruit
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == "mudhook-levy" && stack.quantity == 1)
    );

    let (crystal_mine_sync, crystal_saw_partial_sync) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(10, 23),
            MoveCoord::new(11, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(14, 23),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 30),
        ],
        "nonce:presence:move:crystal-mine",
        "nonce:presence:sync-turn:crystal-mine:",
        244_000_u64,
        "mine_captured",
    );
    assert_eq!(crystal_mine_sync.status, CommandStatus::Applied);
    assert!(crystal_saw_partial_sync);
    assert!(
        crystal_mine_sync
            .events
            .iter()
            .any(|event| event.event_type == "mine_captured")
    );

    advance_time_ms(&fixture, 61_000);
    let income_sync = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_session_turn",
        (
            session_id.clone(),
            "nonce:presence:sync-turn:income".to_string(),
        ),
    )
    .expect("income sync should decode")
    .expect("income sync should succeed");
    assert!(
        income_sync
            .events
            .iter()
            .any(|event| event.event_type == "income_materialized")
    );

    let (guarded_mine_sync, guarded_saw_partial_sync) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:presence:move:guarded-mine",
        "nonce:presence:sync-turn:guarded-mine:",
        488_000_u64,
        "neutral_encounter_pending",
    );
    assert_eq!(guarded_mine_sync.status, CommandStatus::Applied);
    assert!(guarded_saw_partial_sync);
    assert!(guarded_mine_sync.events.iter().any(|event| {
        event.event_type == "neutral_encounter_pending"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("\"battle_id\""))
    }));
    let battle_id = guarded_mine_sync
        .events
        .iter()
        .find(|event| event.event_type == "neutral_encounter_pending")
        .and_then(|event| event.payload.as_deref())
        .and_then(|payload| json_string_field(payload, "battle_id"))
        .expect("neutral encounter event should include battle_id");

    let in_battle_dwelling_preview = query_as::<DwellingRecruitPreview>(
        &fixture,
        player_one,
        "preview_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            champion_id.clone(),
        ),
    )
    .expect("in-battle dwelling recruit preview should decode")
    .expect("in-battle dwelling recruit preview should return a typed denial");
    assert!(!in_battle_dwelling_preview.allowed);
    assert_eq!(
        in_battle_dwelling_preview.disabled_reason.as_deref(),
        Some("champion_in_battle")
    );
    let in_battle_dwelling_nonce = "nonce:presence:dwelling:in-battle".to_string();
    let in_battle_dwelling_submit = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            champion_id.clone(),
            in_battle_dwelling_nonce.clone(),
        ),
    )
    .expect("in-battle dwelling recruit submit should decode")
    .expect_err("in-battle dwelling recruit should fail before command creation");
    assert_eq!(in_battle_dwelling_submit.code, "champion_in_battle");
    let in_battle_dwelling_status = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status_by_nonce",
        (
            session_id.clone(),
            "submit_dwelling_recruit".to_string(),
            in_battle_dwelling_nonce,
        ),
    )
    .expect("in-battle dwelling recruit status lookup should decode")
    .expect_err("pre-command in-battle denial should not leave a command row");
    assert_eq!(in_battle_dwelling_status.code, "command_status_not_found");

    let mut battle = query_as::<BattleView>(
        &fixture,
        player_one,
        "get_battle_state",
        (session_id.clone(), battle_id.clone()),
    )
    .expect("get_battle_state should decode")
    .expect("battle state should be readable");
    assert_eq!(battle.state, "active");
    assert_eq!(battle.battle_type, "neutral");
    assert!(!battle.stacks.is_empty());
    if battle.legal_actions_for_caller.is_empty() {
        advance_time_ms(&fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
        let sync = update_as::<CommandResponse>(
            &fixture,
            player_one,
            "sync_battle",
            (
                session_id.clone(),
                battle_id.clone(),
                "nonce:presence:sync-battle:initial".to_string(),
            ),
        )
        .expect("sync_battle should decode")
        .expect("sync_battle should succeed");
        assert_eq!(sync.status, CommandStatus::Applied);
        battle = query_as::<BattleView>(
            &fixture,
            player_one,
            "get_battle_state",
            (session_id.clone(), battle_id.clone()),
        )
        .expect("get_battle_state after sync should decode")
        .expect("battle state after sync should be readable");
    }
    let active_stack_id = battle
        .active_stack_id
        .clone()
        .expect("battle should have an active stack");
    let active_side = battle
        .stacks
        .iter()
        .find(|stack| stack.battle_stack_id == active_stack_id)
        .expect("active stack should be present")
        .side
        .clone();
    let spell_target_stack_id = battle
        .stacks
        .iter()
        .find(|stack| stack.side != active_side && stack.status == "active" && stack.quantity > 0)
        .expect("battle spell should have an enemy target")
        .battle_stack_id
        .clone();
    let cast_action = battle
        .legal_actions_for_caller
        .iter()
        .find(|action| {
            action.action == "CastAbility"
                && action.ability_key.as_deref() == Some("spell:hex-spark")
        })
        .expect("learned battle spell should be exposed as CastAbility metadata");
    assert!(cast_action.enabled);
    assert!(
        cast_action
            .targets
            .iter()
            .any(|id| id == &spell_target_stack_id)
    );
    assert!(battle.legal_actions_for_caller.iter().any(|action| {
        action.action == "Retreat"
            && !action.enabled
            && action.disabled_reason.as_deref() == Some("retreat_deferred_v1_no_rehire_flow")
    }));
    assert!(battle.legal_actions_for_caller.iter().any(|action| {
        action.action == "Surrender"
            && !action.enabled
            && action.disabled_reason.as_deref() == Some("surrender_deferred_v1_no_payment_terms")
    }));
    let battle_spell_input = BattleActionInput {
        battle_id: battle_id.clone(),
        battle_stack_id: active_stack_id,
        action: "CastAbility".to_string(),
        ability_key: Some("spell:hex-spark".to_string()),
        target_stack_id: Some(spell_target_stack_id),
        destination: None,
    };
    let submitted_battle = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_battle_action",
        (
            session_id.clone(),
            battle_spell_input.clone(),
            "nonce:presence:battle-action".to_string(),
        ),
    )
    .expect("submit_battle_action should decode")
    .expect("submit_battle_action should succeed");
    assert_eq!(submitted_battle.status, CommandStatus::Applied);
    assert!(matches!(
        submitted_battle.result,
        CommandResult::BattleAction(_)
    ));
    assert!(
        submitted_battle
            .events
            .iter()
            .any(|event| event.event_type == "battle_spell_cast")
    );
    let battle_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_battle_action",
        (
            session_id.clone(),
            battle_spell_input,
            "nonce:presence:battle-action".to_string(),
        ),
    )
    .expect("submit_battle_action replay should decode")
    .expect("submit_battle_action replay should succeed");
    assert_eq!(battle_replay.command_id, submitted_battle.command_id);

    let battle_status = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status",
        (
            session_id.clone(),
            "nonce:presence:battle-action".to_string(),
        ),
    )
    .expect("battle command status should decode")
    .expect("battle command should be readable by nonce");
    assert_eq!(battle_status.status, CommandStatus::Applied);

    let battle_status_by_typed_nonce = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status_by_nonce",
        (
            session_id.clone(),
            "submit_battle_action".to_string(),
            "nonce:presence:battle-action".to_string(),
        ),
    )
    .expect("battle command typed nonce status should decode")
    .expect("battle command should be readable by typed nonce");
    assert_eq!(battle_status_by_typed_nonce.status, CommandStatus::Applied);
    assert_eq!(
        battle_status_by_typed_nonce.command_id,
        submitted_battle.command_id
    );

    let battle_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id, "public".to_string(), 0_u64, 200_u32),
    )
    .expect("battle event feed should decode")
    .expect("battle events should be readable");
    assert!(
        battle_events
            .events
            .iter()
            .any(|event| event.event_type == "battle_spell_cast")
    );
}

#[test]
fn pocket_ic_week_two_tavern_and_recruit_growth_materialize_on_turn_advance() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-week-two-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-week-two-player-two");
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "week-two-economy");
    let town_id = "town:west".to_string();

    let week_one = query_as::<TavernOffersView>(
        &fixture,
        player_one,
        "get_tavern_offers",
        (session_id.clone(), town_id.clone()),
    )
    .expect("week-one tavern offers should decode")
    .expect("week-one tavern offers should load");
    assert_eq!(week_one.week_number, 1);
    assert_eq!(week_one.offers.len(), domm_game::TAVERN_OFFERS_PER_WEEK);
    let week_one_keys = week_one
        .offers
        .iter()
        .map(|offer| offer.offer_key.clone())
        .collect::<Vec<_>>();

    let built = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
            "nonce:week-two:build:yard".to_string(),
        ),
    )
    .expect("week-two build should decode")
    .expect("week-two build should succeed");
    assert_eq!(built.status, CommandStatus::Applied);

    let town_after_build = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after week-two build should decode")
    .expect("town after week-two build should load");
    let initial_pool = town_after_build
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == "mudhook-levy")
        .expect("training yard should create a mudhook pool");
    assert_eq!(initial_pool.last_growth_week, 1);

    for turn in 1..=7 {
        advance_time_ms(&fixture, 61_000);
        let synced = update_as::<CommandResponse>(
            &fixture,
            player_one,
            "sync_session_turn",
            (
                session_id.clone(),
                format!("nonce:week-two:sync-turn:{turn}"),
            ),
        )
        .expect("week-two turn sync should decode")
        .expect("week-two turn sync should succeed");
        if synced.status == CommandStatus::Failed {
            assert_eq!(
                synced.error.as_ref().map(|error| error.code.as_str()),
                Some("turn_not_due"),
                "manual sync may be stale only when the timer already advanced the turn"
            );
        } else {
            assert_eq!(synced.status, CommandStatus::Applied);
        }
    }

    let week_two = query_as::<TavernOffersView>(
        &fixture,
        player_one,
        "get_tavern_offers",
        (session_id.clone(), town_id.clone()),
    )
    .expect("week-two tavern offers should decode")
    .expect("week-two tavern offers should load");
    assert_eq!(week_two.week_number, 2);
    assert_eq!(week_two.offers.len(), domm_game::TAVERN_OFFERS_PER_WEEK);
    let week_two_keys = week_two
        .offers
        .iter()
        .map(|offer| offer.offer_key.clone())
        .collect::<Vec<_>>();
    assert_ne!(week_two_keys, week_one_keys);
    assert!(week_two_keys.iter().all(|key| key.contains("week:2")));
    assert!(
        week_two
            .offers
            .iter()
            .all(|offer| offer.status == "available")
    );

    let week_two_repeat = query_as::<TavernOffersView>(
        &fixture,
        player_one,
        "get_tavern_offers",
        (session_id.clone(), town_id.clone()),
    )
    .expect("repeated week-two tavern offers should decode")
    .expect("repeated week-two tavern offers should load");
    let week_two_repeat_keys = week_two_repeat
        .offers
        .iter()
        .map(|offer| offer.offer_key.clone())
        .collect::<Vec<_>>();
    assert_eq!(week_two_repeat_keys, week_two_keys);

    let town_week_two = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id),
    )
    .expect("town week-two view should decode")
    .expect("town week-two view should load");
    let grown_pool = town_week_two
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == "mudhook-levy")
        .expect("week-two town view should include the mudhook pool");
    assert_eq!(grown_pool.last_growth_week, 2);
    assert!(grown_pool.available > initial_pool.available);
}

#[test]
fn pocket_ic_gate_j_strategic_loop_persists_icydb_rows() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-gate-j-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-gate-j-two");
    let viewport = opening_viewport_for_slot(0);
    let mut metrics = GateJMetrics::default();

    let non_controller_diagnostic = gate_query_as::<DiagnosticStorageSnapshot>(
        &mut metrics,
        &fixture,
        player_one,
        "get_diagnostic_storage_snapshot",
        (entity_names(&["GameSession"]),),
    )
    .expect_err("diagnostics must be controller-gated");
    assert_eq!(non_controller_diagnostic.code, "controller_required");

    let initial_storage =
        gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert_eq!(initial_storage.total_rows, 0);

    let session_id = gate_start_active_two_player_session(
        &mut metrics,
        &fixture,
        player_one,
        player_two,
        "gate-j",
    );
    let active_session = gate_query_as::<SessionView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_session",
        (session_id.clone(),),
    )
    .expect("active session should be readable");
    assert_eq!(active_session.state, "active");

    let active_storage = gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert_eq!(row_count(&active_storage, "GameSession"), 1);
    assert_eq!(row_count(&active_storage, "GameParticipant"), 2);
    assert_eq!(row_count(&active_storage, "Champion"), 2);
    assert_eq!(row_count(&active_storage, "Town"), 2);
    assert!(row_count(&active_storage, "MapChunk") > 0);
    assert!(row_count(&active_storage, "VisibilityChunk") > 0);

    let chunk_page = gate_query_as::<MapChunkPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_visible_map_chunks",
        (session_id.clone(), viewport.clone(), None::<u32>, 8_u32),
    )
    .expect("opening map chunks should be visible");
    assert_eq!(chunk_page.chunks.len(), 4);
    let object_page = gate_query_as::<ObjectViewPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_visible_objects",
        (session_id.clone(), viewport.clone(), None::<u32>, 128_u32),
    )
    .expect("opening objects should be visible");
    assert!(
        object_page
            .objects
            .iter()
            .any(|object| object.subject_id_text == "pile:west-wood-1")
    );
    assert!(
        object_page
            .objects
            .iter()
            .any(|object| object.subject_id_text == "neutral:west-mine")
    );

    let participant_before_pickup = gate_query_as::<ParticipantView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant before pickup should be readable");
    let champion_id = gate_owned_champion_id(&mut metrics, &fixture, player_one, &session_id);
    let champion_before_pickup = gate_query_as::<ChampionView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id.clone()),
    )
    .expect("owned champion should be readable");
    assert_eq!(
        (champion_before_pickup.x, champion_before_pickup.y),
        (8, 24)
    );

    let moved_to_wood = gate_submit_move_intent(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        "nonce:gate-j:move:wood",
    );
    assert_eq!(moved_to_wood.status, CommandStatus::Applied);
    let (_synced_wood, _) = gate_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        "nonce:gate-j:sync:wood:",
        61_000,
        "resource_picked_up",
        4,
    );
    let participant_after_pickup = gate_query_as::<ParticipantView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after pickup should be readable");
    assert!(participant_after_pickup.resources.wood > participant_before_pickup.resources.wood);
    let pickup_storage = gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert!(
        row_count(&pickup_storage, "ParticipantObjectVisit")
            > row_count(&active_storage, "ParticipantObjectVisit")
    );
    assert!(
        row_count(&pickup_storage, "ResourceLedgerEntry")
            > row_count(&active_storage, "ResourceLedgerEntry")
    );
    assert!(
        row_count(&pickup_storage, "MovementSnapshot")
            > row_count(&active_storage, "MovementSnapshot")
    );

    let built = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            "town:west".to_string(),
            "building:freehold-training-yard".to_string(),
            "nonce:gate-j:build:yard".to_string(),
        ),
    )
    .expect("training yard build should succeed");
    metrics.observe_command_response(&built);
    assert_eq!(built.status, CommandStatus::Applied);
    let town_after_build = gate_query_as::<ApiTownView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), "town:west".to_string()),
    )
    .expect("town after build should be readable");
    assert!(
        town_after_build
            .buildings
            .iter()
            .any(|building| building.building_slug == "freehold-training-yard")
    );
    let pool_after_build = town_after_build
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == "mudhook-levy")
        .expect("training yard should create mudhook levy pool")
        .available;
    assert!(pool_after_build > 0);
    let build_storage = gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert!(row_count(&build_storage, "TownBuilding") > row_count(&pickup_storage, "TownBuilding"));

    let recruited = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "submit_recruit_units",
        (
            session_id.clone(),
            "town:west".to_string(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
            "nonce:gate-j:recruit:levy".to_string(),
        ),
    )
    .expect("mudhook levy recruit should succeed");
    metrics.observe_command_response(&recruited);
    assert_eq!(recruited.status, CommandStatus::Applied);
    let town_after_recruit = gate_query_as::<ApiTownView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), "town:west".to_string()),
    )
    .expect("town after recruit should be readable");
    let pool_after_recruit = town_after_recruit
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == "mudhook-levy")
        .expect("mudhook levy pool should remain visible")
        .available;
    assert_eq!(pool_after_recruit, pool_after_build - 1);
    assert!(
        town_after_recruit
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == "mudhook-levy" && stack.quantity == 1)
    );
    let recruit_storage =
        gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert!(
        row_count(&recruit_storage, "TownGarrisonStack")
            > row_count(&build_storage, "TownGarrisonStack")
    );

    let (crystal_mine_sync, crystal_saw_partial_sync) = gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(10, 23),
            MoveCoord::new(11, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(14, 23),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 30),
        ],
        "nonce:gate-j:move:crystal-mine",
        "nonce:gate-j:sync:crystal-mine:",
        244_000_u64,
        "mine_captured",
    );
    assert_eq!(crystal_mine_sync.status, CommandStatus::Applied);
    assert!(crystal_saw_partial_sync);
    let crystal_storage =
        gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert!(
        row_count(&crystal_storage, "MovementSnapshot")
            > row_count(&recruit_storage, "MovementSnapshot")
    );
    assert!(
        row_count(&crystal_storage, "ParticipantObjectVisit")
            > row_count(&recruit_storage, "ParticipantObjectVisit")
    );

    advance_time_ms(&fixture, 61_000);
    let income_sync = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "sync_session_turn",
        (session_id.clone(), "nonce:gate-j:sync:income".to_string()),
    )
    .expect("income sync should succeed");
    metrics.observe_command_response(&income_sync);
    assert!(
        income_sync
            .events
            .iter()
            .any(|event| event.event_type == "income_materialized")
    );
    let income_storage = gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert!(
        row_count(&income_storage, "ResourceLedgerTurnSummary")
            > row_count(&crystal_storage, "ResourceLedgerTurnSummary")
    );

    let (guarded_mine_sync, guarded_saw_partial_sync) = gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:gate-j:move:guarded-mine",
        "nonce:gate-j:sync:guarded-mine:",
        488_000_u64,
        "neutral_encounter_pending",
    );
    assert_eq!(guarded_mine_sync.status, CommandStatus::Applied);
    assert!(guarded_saw_partial_sync);
    assert!(guarded_mine_sync.events.iter().any(|event| {
        event.event_type == "neutral_encounter_pending"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("\"battle_id\""))
    }));

    let champion_after_guard = gate_query_as::<ChampionView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id),
    )
    .expect("champion after guarded contact should be readable");
    assert_eq!(champion_after_guard.status, "in_battle");
    let final_events = gate_query_as::<ApiEventPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_events_after",
        (session_id, "public".to_string(), 0_u64, 200_u32),
    )
    .expect("final public events should be readable");
    metrics.observe_event_page(&final_events);
    assert!(
        final_events
            .events
            .iter()
            .any(|event| event.event_type == "neutral_encounter_pending")
    );

    let final_storage = gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_PROGRESS_ENTITIES);
    assert!(row_count(&final_storage, "Battle") > row_count(&income_storage, "Battle"));
    assert!(row_count(&final_storage, "BattleStack") > 0);
    assert!(row_count(&final_storage, "BattleOccupancy") > 0);
    assert!(row_count(&final_storage, "BattleObstacle") > 0);
    assert!(
        row_count(&final_storage, "MovementSnapshot")
            > row_count(&income_storage, "MovementSnapshot")
    );
    assert!(final_storage.total_rows > initial_storage.total_rows);
    assert!(final_storage.stable_memory_pages >= initial_storage.stable_memory_pages);

    let command_storage =
        gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_J_COMMAND_EVENT_ENTITIES);
    metrics.print_report(&initial_storage, &final_storage, &command_storage);
}

#[test]
fn pocket_ic_gate_k_battle_aftermath_victory_history_persist_icydb_rows() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-gate-k-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-gate-k-two");
    let session_id = start_active_two_player_session(&fixture, player_one, player_two, "gate-k");
    let west_champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let east_champion_id = owned_champion_id(&fixture, player_two, &session_id);

    let initial_storage = diagnostic_snapshot(&fixture, GATE_K_ENTITIES);
    assert_eq!(row_count(&initial_storage, "PlayerMatchSummary"), 2);

    let (neutral_sync, _) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
            MoveCoord::new(12, 24),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:gate-k:move:neutral",
        "nonce:gate-k:sync:neutral:",
        1_000_u64,
        "neutral_encounter_pending",
    );
    let neutral_battle_id = battle_id_from_events(&neutral_sync, "neutral_encounter_pending");
    let neutral_view = query_as::<BattleView>(
        &fixture,
        player_one,
        "get_battle_state",
        (session_id.clone(), neutral_battle_id.clone()),
    )
    .expect("neutral battle view should decode")
    .expect("neutral battle view should load");
    assert_eq!(neutral_view.battle_type, "neutral");
    assert!(!neutral_view.legal_actions_for_caller.is_empty());

    resolve_battle_to_end(
        &fixture,
        player_one,
        &session_id,
        &neutral_battle_id,
        "nonce:gate-k:neutral-battle",
    );
    let west_after_neutral = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), west_champion_id.clone()),
    )
    .expect("west champion after neutral should decode")
    .expect("west champion after neutral should load");
    assert_eq!(west_after_neutral.status, "active");
    assert_eq!((west_after_neutral.x, west_after_neutral.y), (12, 22));

    let first_east_stage = ((west_after_neutral.x + 1)..=22)
        .map(|x| MoveCoord::new(x, west_after_neutral.y))
        .collect::<Vec<_>>();
    submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        first_east_stage,
        "nonce:gate-k:move:east-stage-1",
        "nonce:gate-k:sync:east-stage-1:",
        122_000_u64,
        "session_turn_synced",
    );
    let second_east_stage = (23..=32)
        .map(|x| MoveCoord::new(x, west_after_neutral.y))
        .collect::<Vec<_>>();
    submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        second_east_stage,
        "nonce:gate-k:move:east-stage-2",
        "nonce:gate-k:sync:east-stage-2:",
        183_000_u64,
        "session_turn_synced",
    );
    let mut east_path = (33..=39)
        .map(|x| MoveCoord::new(x, west_after_neutral.y))
        .collect::<Vec<_>>();
    east_path.push(MoveCoord::new(39, 23));
    east_path.push(MoveCoord::new(39, 24));
    let (champion_sync, _) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        east_path,
        "nonce:gate-k:move:champion",
        "nonce:gate-k:sync:champion:",
        244_000_u64,
        "champion_encounter_pending",
    );
    let champion_battle_id = battle_id_from_events(&champion_sync, "champion_encounter_pending");
    resolve_battle_to_end(
        &fixture,
        player_one,
        &session_id,
        &champion_battle_id,
        "nonce:gate-k:champion-battle",
    );
    let east_after_defeat = query_as::<ChampionView>(
        &fixture,
        player_two,
        "get_champion_view",
        (session_id.clone(), east_champion_id),
    )
    .expect("east champion after defeat should decode")
    .expect("east champion after defeat should load");
    assert_eq!(east_after_defeat.status, "defeated");

    let (town_contact_sync, _) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![MoveCoord::new(40, 24), MoveCoord::new(41, 24)],
        "nonce:gate-k:move:town",
        "nonce:gate-k:sync:town:",
        305_000_u64,
        "town_encounter_pending",
    );
    let town_battle_id = battle_id_from_events(&town_contact_sync, "town_encounter_pending");
    advance_time_ms(&fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
    let town_sync = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_battle",
        (
            session_id.clone(),
            town_battle_id,
            "nonce:gate-k:town-battle:sync".to_string(),
        ),
    )
    .expect("town sync_battle should decode")
    .expect("town sync_battle should succeed");
    assert_eq!(
        town_sync.status,
        CommandStatus::Applied,
        "town sync response: {town_sync:?}"
    );
    assert!(
        town_sync
            .events
            .iter()
            .any(|event| event.event_type == "town_captured")
    );
    assert!(
        town_sync
            .events
            .iter()
            .any(|event| event.event_type == "victory_finalized")
    );

    let finished =
        query_as::<SessionView>(&fixture, player_one, "get_session", (session_id.clone(),))
            .expect("finished session should decode")
            .expect("finished session should load");
    assert_eq!(finished.state, "finished");

    let west_history =
        query_as::<MatchHistoryPage>(&fixture, player_one, "get_match_history", (0_u32, 10_u32))
            .expect("winner history should decode")
            .expect("winner history should load");
    assert!(
        west_history
            .entries
            .iter()
            .any(|entry| entry.session_id == session_id && entry.result == "win")
    );
    let east_history =
        query_as::<MatchHistoryPage>(&fixture, player_two, "get_match_history", (0_u32, 10_u32))
            .expect("loser history should decode")
            .expect("loser history should load");
    assert!(
        east_history
            .entries
            .iter()
            .any(|entry| entry.session_id == session_id && entry.result == "loss")
    );

    let final_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id, "public".to_string(), 0_u64, 200_u32),
    )
    .expect("Gate K event feed should decode")
    .expect("Gate K event feed should load");
    for expected in [
        "battle_action_applied",
        "neutral_defeated",
        "champion_defeated",
        "town_captured",
        "victory_finalized",
    ] {
        assert!(
            final_events
                .events
                .iter()
                .any(|event| event.event_type == expected),
            "missing event {expected}"
        );
    }

    let final_storage = diagnostic_snapshot(&fixture, GATE_K_ENTITIES);
    assert!(row_count(&final_storage, "Battle") >= 3);
    assert!(row_count(&final_storage, "GameCommand") > row_count(&initial_storage, "GameCommand"));
    assert_eq!(row_count(&final_storage, "PlayerMatchSummary"), 2);
}

#[test]
fn pocket_ic_gate_l_first_playable_canister_e2e_uses_public_endpoints_and_icydb_state() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-gate-l-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-gate-l-two");
    let viewport = opening_viewport_for_slot(0);
    let mut metrics = GateJMetrics::default();

    let initial_storage = gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_L_ENTITIES);
    assert_eq!(initial_storage.total_rows, 0);

    let session_id = gate_start_active_two_player_session(
        &mut metrics,
        &fixture,
        player_one,
        player_two,
        "gate-l",
    );
    let west_participant = gate_query_as::<ParticipantView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("west participant should load");
    let west_participant_id = west_participant.participant_id.clone();
    let west_champion_id = gate_owned_champion_id(&mut metrics, &fixture, player_one, &session_id);
    let east_champion_id = gate_owned_champion_id(&mut metrics, &fixture, player_two, &session_id);

    let opening_view = gate_query_as::<GameView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_game_view",
        (
            session_id.clone(),
            GameViewRequest {
                viewport: viewport.clone(),
                chunk_cursor: None,
                chunk_limit: MAX_CHUNK_LIMIT,
                object_cursor: None,
                object_limit: 128,
                events_after_seq: 0,
                event_limit: 25,
                include_battle: false,
            },
        ),
    )
    .expect("opening game view should load");
    assert_eq!(opening_view.session.state, "active");
    assert!(opening_view.map_chunks.is_empty());
    let opening_chunks = gate_query_as::<MapChunkPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_visible_map_chunks",
        (
            session_id.clone(),
            viewport.clone(),
            None::<u32>,
            MAX_CHUNK_LIMIT,
        ),
    )
    .expect("opening map chunks should load");
    assert!(opening_chunks.chunks.len() >= 4);
    let opening_objects = gate_query_as::<ObjectViewPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_visible_objects",
        (session_id.clone(), viewport.clone(), None::<u32>, 128_u32),
    )
    .expect("opening objects should load");
    assert!(
        opening_objects
            .objects
            .iter()
            .any(|object| object.subject_id_text == "pile:west-wood-1")
    );

    let participant_before_pickup = gate_query_as::<ParticipantView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant before pickup should load");
    let moved_to_wood = gate_submit_move_intent(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        "nonce:gate-l:move:wood",
    );
    assert_eq!(moved_to_wood.status, CommandStatus::Applied);
    let wood_status = gate_query_as::<CommandStatusView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_command_status",
        (session_id.clone(), "nonce:gate-l:move:wood".to_string()),
    )
    .expect("wood movement command status should load");
    assert_eq!(wood_status.status, CommandStatus::Applied);

    let (_synced_wood, _) = gate_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        "nonce:gate-l:sync:wood:",
        61_000,
        "resource_picked_up",
        4,
    );
    let participant_after_pickup = gate_query_as::<ParticipantView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after pickup should load");
    assert!(participant_after_pickup.resources.wood > participant_before_pickup.resources.wood);

    let built = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            "town:west".to_string(),
            "building:freehold-training-yard".to_string(),
            "nonce:gate-l:build:yard".to_string(),
        ),
    )
    .expect("training yard build should succeed");
    metrics.observe_command_response(&built);
    assert_eq!(built.status, CommandStatus::Applied);
    let recruited = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "submit_recruit_units",
        (
            session_id.clone(),
            "town:west".to_string(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
            "nonce:gate-l:recruit:levy".to_string(),
        ),
    )
    .expect("mudhook levy recruit should succeed");
    metrics.observe_command_response(&recruited);
    assert_eq!(recruited.status, CommandStatus::Applied);
    let west_town_after_recruit = gate_query_as::<ApiTownView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), "town:west".to_string()),
    )
    .expect("west town after recruit should load");
    assert!(
        west_town_after_recruit
            .buildings
            .iter()
            .any(|building| building.building_slug == "freehold-training-yard")
    );
    assert!(
        west_town_after_recruit
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == "mudhook-levy" && stack.quantity == 1)
    );

    let (crystal_sync, crystal_saw_partial_sync) = gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![
            MoveCoord::new(10, 23),
            MoveCoord::new(11, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(14, 23),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 30),
        ],
        "nonce:gate-l:move:crystal-mine",
        "nonce:gate-l:sync:crystal-mine:",
        244_000_u64,
        "mine_captured",
    );
    assert_eq!(crystal_sync.status, CommandStatus::Applied);
    assert!(crystal_saw_partial_sync);

    advance_time_ms(&fixture, 61_000);
    let income_sync = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "sync_session_turn",
        (session_id.clone(), "nonce:gate-l:sync:income".to_string()),
    )
    .expect("income sync should succeed");
    metrics.observe_command_response(&income_sync);
    assert!(
        income_sync
            .events
            .iter()
            .any(|event| event.event_type == "income_materialized")
    );

    let (neutral_sync, neutral_saw_partial_sync) = gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![
            MoveCoord::new(14, 29),
            MoveCoord::new(14, 28),
            MoveCoord::new(14, 27),
            MoveCoord::new(14, 26),
            MoveCoord::new(14, 25),
            MoveCoord::new(14, 24),
            MoveCoord::new(14, 23),
            MoveCoord::new(13, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:gate-l:move:guarded-mine",
        "nonce:gate-l:sync:guarded-mine:",
        488_000_u64,
        "neutral_encounter_pending",
    );
    assert!(neutral_saw_partial_sync);
    let neutral_battle_id = battle_id_from_events(&neutral_sync, "neutral_encounter_pending");
    let mid_events = gate_query_as::<ApiEventPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 200_u32),
    )
    .expect("mid-route public events should load");
    metrics.observe_event_page(&mid_events);
    assert!(
        mid_events
            .events
            .iter()
            .any(|event| event.event_type == "neutral_encounter_pending")
    );
    let mid_event_seq = mid_events
        .events
        .last()
        .map(|event| event.event_seq)
        .unwrap_or(0);

    let (first_battle_action, first_action_saw_sync) = gate_submit_retryable_battle_action(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &neutral_battle_id,
        "nonce:gate-l:battle-action:neutral:first",
        "nonce:gate-l:neutral-battle:initial-sync:",
    );
    assert_eq!(first_battle_action.status, CommandStatus::Applied);
    let battle_status = gate_query_as::<CommandStatusView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_command_status",
        (
            session_id.clone(),
            "nonce:gate-l:battle-action:neutral:first".to_string(),
        ),
    )
    .expect("battle action command status should load");
    assert_eq!(battle_status.status, CommandStatus::Applied);
    let (_, neutral_saw_battle_sync) = gate_resolve_battle_to_end(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &neutral_battle_id,
        "nonce:gate-l:neutral-battle:resolve",
    );
    assert!(
        first_action_saw_sync || neutral_saw_battle_sync,
        "Gate L route should exercise sync_battle"
    );
    let post_neutral_events = gate_query_as::<ApiEventPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_events_after",
        (
            session_id.clone(),
            "public".to_string(),
            mid_event_seq,
            64_u32,
        ),
    )
    .expect("post-neutral event feed should load");
    metrics.observe_event_page(&post_neutral_events);
    for expected in [
        "mine_captured",
        "neutral_defeated",
        "battle_aftermath_applied",
    ] {
        assert_eq!(
            post_neutral_events
                .events
                .iter()
                .filter(|event| event.event_type == expected)
                .count(),
            1,
            "expected exactly one {expected} event after guarded battle"
        );
    }
    let resolved_sync = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "sync_battle",
        (
            session_id.clone(),
            neutral_battle_id.clone(),
            "nonce:gate-l:neutral-battle:resolved-noop".to_string(),
        ),
    )
    .expect("resolved neutral sync_battle should no-op");
    metrics.observe_command_response(&resolved_sync);
    assert_eq!(resolved_sync.status, CommandStatus::Applied);
    assert!(
        !resolved_sync.events.iter().any(|event| matches!(
            event.event_type.as_str(),
            "mine_captured" | "neutral_defeated" | "battle_aftermath_applied"
        )),
        "resolved battle sync must not duplicate aftermath events"
    );

    let west_after_neutral = gate_query_as::<ChampionView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), west_champion_id.clone()),
    )
    .expect("west champion after neutral should load");
    assert_eq!(west_after_neutral.status, "active");
    assert_eq!((west_after_neutral.x, west_after_neutral.y), (12, 22));
    let guarded_mine_objects = gate_query_as::<ObjectViewPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_visible_objects",
        (session_id.clone(), viewport.clone(), None::<u32>, 128_u32),
    )
    .expect("guarded mine object page should load");
    assert!(
        guarded_mine_objects
            .objects
            .iter()
            .all(|object| object.subject_id_text != "neutral:west-mine"),
        "defeated neutral guard must not render as active"
    );
    let guarded_mine = guarded_mine_objects
        .objects
        .iter()
        .find(|object| object.subject_id_text == "mine:west-gold")
        .expect("guarded mine should remain visible after capture");
    assert_eq!(
        guarded_mine.owner_participant_id.as_deref(),
        Some(west_participant_id.as_str())
    );
    assert!(
        guarded_mine.details_json.contains(r#""state":"captured""#),
        "captured guarded mine should render captured details: {}",
        guarded_mine.details_json
    );

    let first_east_stage = ((west_after_neutral.x + 1)..=22)
        .map(|x| MoveCoord::new(x, west_after_neutral.y))
        .collect::<Vec<_>>();
    gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        first_east_stage,
        "nonce:gate-l:move:east-stage-1",
        "nonce:gate-l:sync:east-stage-1:",
        549_000_u64,
        "session_turn_synced",
    );
    let second_east_stage = (23..=32)
        .map(|x| MoveCoord::new(x, west_after_neutral.y))
        .collect::<Vec<_>>();
    gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        second_east_stage,
        "nonce:gate-l:move:east-stage-2",
        "nonce:gate-l:sync:east-stage-2:",
        610_000_u64,
        "session_turn_synced",
    );
    let mut champion_path = (33..=39)
        .map(|x| MoveCoord::new(x, west_after_neutral.y))
        .collect::<Vec<_>>();
    champion_path.push(MoveCoord::new(39, 23));
    champion_path.push(MoveCoord::new(39, 24));
    let (champion_sync, _) = gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        champion_path,
        "nonce:gate-l:move:champion",
        "nonce:gate-l:sync:champion:",
        671_000_u64,
        "champion_encounter_pending",
    );
    let champion_battle_id = battle_id_from_events(&champion_sync, "champion_encounter_pending");
    gate_resolve_battle_to_end(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &champion_battle_id,
        "nonce:gate-l:champion-battle",
    );
    let east_after_defeat = gate_query_as::<ChampionView>(
        &mut metrics,
        &fixture,
        player_two,
        "get_champion_view",
        (session_id.clone(), east_champion_id.clone()),
    )
    .expect("east champion after defeat should load");
    assert_eq!(east_after_defeat.status, "defeated");

    let (town_contact_sync, _) = gate_submit_move_and_sync_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![MoveCoord::new(40, 24), MoveCoord::new(41, 24)],
        "nonce:gate-l:move:town",
        "nonce:gate-l:sync:town:",
        732_000_u64,
        "town_encounter_pending",
    );
    let town_battle_id = battle_id_from_events(&town_contact_sync, "town_encounter_pending");
    advance_time_ms(&fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
    let town_sync = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "sync_battle",
        (
            session_id.clone(),
            town_battle_id,
            "nonce:gate-l:town-battle:sync".to_string(),
        ),
    )
    .expect("town sync_battle should succeed");
    metrics.observe_command_response(&town_sync);
    assert_eq!(
        town_sync.status,
        CommandStatus::Applied,
        "town sync response: {town_sync:?}"
    );
    assert!(
        town_sync
            .events
            .iter()
            .any(|event| event.event_type == "town_captured")
    );
    assert!(
        town_sync
            .events
            .iter()
            .any(|event| event.event_type == "victory_finalized")
    );

    let finished = gate_query_as::<SessionView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_session",
        (session_id.clone(),),
    )
    .expect("finished session should load");
    assert_eq!(finished.state, "finished");
    let west_after_victory = gate_query_as::<ChampionView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), west_champion_id),
    )
    .expect("west champion after victory should load");
    assert_eq!(west_after_victory.status, "active");
    assert_eq!((west_after_victory.x, west_after_victory.y), (41, 24));
    let east_town = gate_query_as::<ApiTownView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), "town:east".to_string()),
    )
    .expect("east town after capture should load");
    assert_eq!(
        east_town.town.owner_participant_id.as_str(),
        west_participant_id.as_str()
    );

    let west_history = gate_query_as::<MatchHistoryPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_match_history",
        (0_u32, 10_u32),
    )
    .expect("winner history should load");
    assert!(
        west_history
            .entries
            .iter()
            .any(|entry| entry.session_id == session_id && entry.result == "win")
    );
    let east_history = gate_query_as::<MatchHistoryPage>(
        &mut metrics,
        &fixture,
        player_two,
        "get_match_history",
        (0_u32, 10_u32),
    )
    .expect("loser history should load");
    assert!(
        east_history
            .entries
            .iter()
            .any(|entry| entry.session_id == session_id && entry.result == "loss")
    );

    let final_refresh = gate_query_as::<ApiEventPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_events_after",
        (
            session_id.clone(),
            "public".to_string(),
            mid_event_seq,
            200_u32,
        ),
    )
    .expect("final event refresh should load");
    metrics.observe_event_page(&final_refresh);
    for expected in [
        "battle_action_applied",
        "mine_captured",
        "income_materialized",
        "neutral_defeated",
        "champion_defeated",
        "town_captured",
        "victory_finalized",
    ] {
        assert!(
            final_refresh
                .events
                .iter()
                .any(|event| event.event_type == expected),
            "missing event {expected}"
        );
    }

    let final_storage = gate_diagnostic_snapshot(&mut metrics, &fixture, GATE_L_ENTITIES);
    assert_eq!(row_count(&final_storage, "GameSession"), 1);
    assert_eq!(row_count(&final_storage, "GameParticipant"), 2);
    assert_eq!(row_count(&final_storage, "Champion"), 2);
    assert_eq!(row_count(&final_storage, "Town"), 2);
    assert_eq!(row_count(&final_storage, "PlayerMatchSummary"), 2);
    assert_eq!(row_count(&final_storage, "MapOccupancy"), 3);
    assert!(row_count(&final_storage, "Battle") >= 3);
    assert!(row_count(&final_storage, "BattleStack") > 0);
    assert!(row_count(&final_storage, "BattleObstacle") > 0);
    assert!(row_count(&final_storage, "GameCommand") > 0);
    assert!(row_count(&final_storage, "CommandEffect") > 0);
    assert!(row_count(&final_storage, "GameEvent") > 0);
    assert!(row_count(&final_storage, "MovementSnapshot") > 0);
    assert!(row_count(&final_storage, "ResourceLedgerEntry") > 0);
    assert!(row_count(&final_storage, "ResourceLedgerTurnSummary") > 0);
    assert!(row_count(&final_storage, "ParticipantObjectVisit") > 0);
    assert!(row_count(&final_storage, "ObjectiveProgress") > 0);
    assert!(row_count(&final_storage, "QuestState") > 0);
    assert!(row_count(&final_storage, "WorldEventState") > 0);
    assert!(row_count(&final_storage, "ScenarioRuleState") > 0);
    assert!(row_count(&final_storage, "SkirmishSettingsState") > 0);
    assert!(row_count(&final_storage, "ProceduralMapState") > 0);
    assert!(row_count(&final_storage, "NavalRouteState") > 0);
    assert!(row_count(&final_storage, "SiegeRuleState") > 0);
    assert!(row_count(&final_storage, "TownBuilding") > 0);
    assert!(row_count(&final_storage, "TownGarrisonStack") > 0);
    assert!(row_count(&final_storage, "WorldObject") > 0);
    assert!(row_count(&final_storage, "NeutralArmy") > 0);
    metrics.print_named_report("Gate L", &initial_storage, &final_storage, &final_storage);
}

#[test]
fn pocket_ic_movement_crossing_conflict_uses_persisted_sync_cursor() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-conflict-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-conflict-two");
    let session_id = start_active_two_player_session(&fixture, player_one, player_two, "conflict");
    let west_champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let east_champion_id = owned_champion_id(&fixture, player_two, &session_id);

    let west_preposition_path = (9_u16..=18)
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();
    let east_preposition_path = (29_u16..=38)
        .rev()
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();

    submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        west_preposition_path,
        "nonce:conflict:move:west-preposition",
        "nonce:conflict:sync:west-preposition:",
        1_000_u64,
        "session_turn_synced",
    );
    submit_move_and_sync_until_event(
        &fixture,
        player_two,
        &session_id,
        &east_champion_id,
        east_preposition_path,
        "nonce:conflict:move:east-preposition",
        "nonce:conflict:sync:east-preposition:",
        61_000_u64,
        "session_turn_synced",
    );

    let west_path = (19_u16..=24)
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();
    let east_path = (23_u16..=28)
        .rev()
        .map(|x| MoveCoord::new(x, 24))
        .collect::<Vec<_>>();

    submit_move_intent(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        west_path,
        "nonce:conflict:move:west",
        1_000_u64,
    );
    submit_move_intent(
        &fixture,
        player_two,
        &session_id,
        &east_champion_id,
        east_path,
        "nonce:conflict:move:east",
        1_000_u64,
    );

    let (synced, saw_partial_sync) = sync_until_event(
        &fixture,
        player_one,
        &session_id,
        "nonce:conflict:sync:",
        122_000_u64,
        "champion_encounter_pending",
        8,
    );
    assert_eq!(synced.status, CommandStatus::Applied);
    assert!(saw_partial_sync);

    let west_after = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), west_champion_id),
    )
    .expect("west champion after conflict should decode")
    .expect("west champion after conflict should be visible");
    let east_after = query_as::<ChampionView>(
        &fixture,
        player_two,
        "get_champion_view",
        (session_id, east_champion_id),
    )
    .expect("east champion after conflict should decode")
    .expect("east champion after conflict should be visible");
    assert_eq!(west_after.status, "in_battle");
    assert_eq!(east_after.status, "in_battle");
    assert_eq!((west_after.x, west_after.y), (23, 24));
    assert_eq!((east_after.x, east_after.y), (24, 24));
}

#[test]
fn pocket_ic_stationary_enemy_blocker_starts_champion_encounter() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-blocker-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-blocker-two");
    let session_id = start_active_two_player_session(&fixture, player_one, player_two, "blocker");
    let west_champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let east_champion_id = owned_champion_id(&fixture, player_two, &session_id);

    for (leg, range, now_ms) in [
        (0_u8, (29_u16..=38_u16), 1_000_u64),
        (1_u8, (19_u16..=28_u16), 61_000_u64),
        (2_u8, (9_u16..=18_u16), 122_000_u64),
    ] {
        let east_to_blocker = range
            .rev()
            .map(|x| MoveCoord::new(x, 24))
            .collect::<Vec<_>>();
        let (_, saw_partial_sync) = submit_move_and_sync_until_event(
            &fixture,
            player_two,
            &session_id,
            &east_champion_id,
            east_to_blocker,
            &format!("nonce:blocker:move:east:{leg}"),
            &format!("nonce:blocker:sync:east:{leg}:"),
            now_ms,
            "session_turn_synced",
        );
        assert!(saw_partial_sync);
    }

    submit_move_intent(
        &fixture,
        player_one,
        &session_id,
        &west_champion_id,
        vec![MoveCoord::new(9, 24)],
        "nonce:blocker:move:west",
        183_000_u64,
    );
    let (blocked_sync, _) = sync_until_event(
        &fixture,
        player_one,
        &session_id,
        "nonce:blocker:sync:west:",
        244_000_u64,
        "champion_encounter_pending",
        4,
    );
    assert!(
        blocked_sync
            .events
            .iter()
            .any(|event| event.event_type == "champion_encounter_pending")
    );

    let west_after = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), west_champion_id),
    )
    .expect("west blocker champion query should decode")
    .expect("west blocker champion should be visible");
    let east_after = query_as::<ChampionView>(
        &fixture,
        player_two,
        "get_champion_view",
        (session_id, east_champion_id),
    )
    .expect("east blocker champion query should decode")
    .expect("east blocker champion should be visible");
    assert_eq!(west_after.status, "in_battle");
    assert_eq!(east_after.status, "in_battle");
    assert_eq!((west_after.x, west_after.y), (8, 24));
    assert_eq!((east_after.x, east_after.y), (9, 24));
}

fn start_active_two_player_session(
    fixture: &StandaloneCanisterFixture,
    player_one: candid::Principal,
    player_two: candid::Principal,
    nonce_stem: &str,
) -> String {
    update_as::<LobbyCommandResponse>(
        fixture,
        player_one,
        "register_player",
        (
            Some(format!("{nonce_stem}-one")),
            Some(format!("{nonce_stem} One")),
            format!("nonce:{nonce_stem}:register:one"),
        ),
    )
    .expect("player one registration should decode")
    .expect("player one registration should succeed");
    update_as::<LobbyCommandResponse>(
        fixture,
        player_two,
        "register_player",
        (
            Some(format!("{nonce_stem}-two")),
            Some(format!("{nonce_stem} Two")),
            format!("nonce:{nonce_stem}:register:two"),
        ),
    )
    .expect("player two registration should decode")
    .expect("player two registration should succeed");

    let created = update_as::<LobbyCommandResponse>(
        fixture,
        player_one,
        "create_session",
        (
            format!("{nonce_stem} Match"),
            FIRST_PLAYABLE_RULESET_ID.to_string(),
            1_u64,
            format!("nonce:{nonce_stem}:create"),
        ),
    )
    .expect("create_session should decode")
    .expect("create_session should succeed");
    let session_id = match created.result {
        LobbyCommandResult::Session(session) => session.session_id,
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    update_as::<LobbyCommandResponse>(
        fixture,
        player_two,
        "join_session",
        (
            session_id.clone(),
            "faction:ashen-ledger".to_string(),
            format!("nonce:{nonce_stem}:join"),
        ),
    )
    .expect("join_session should decode")
    .expect("join_session should succeed");
    update_as::<LobbyCommandResponse>(
        fixture,
        player_one,
        "mark_ready",
        (session_id.clone(), format!("nonce:{nonce_stem}:ready:one")),
    )
    .expect("player one ready should decode")
    .expect("player one ready should succeed");
    update_as::<LobbyCommandResponse>(
        fixture,
        player_two,
        "mark_ready",
        (session_id.clone(), format!("nonce:{nonce_stem}:ready:two")),
    )
    .expect("player two ready should decode")
    .expect("player two ready should succeed");

    for step in 0..18 {
        let started = update_as::<LobbyCommandResponse>(
            fixture,
            player_one,
            "start_session",
            (
                session_id.clone(),
                format!("nonce:{nonce_stem}:start:{step}"),
            ),
        )
        .expect("start_session should decode")
        .expect("start_session should succeed");
        assert_eq!(started.status, CommandStatus::Applied);
        if matches!(
            started.result,
            LobbyCommandResult::Session(ref session) if session.state == "active"
        ) {
            return session_id;
        }
    }

    panic!("phased start_session should finish setup");
}

#[derive(Default)]
struct GateJMetrics {
    update_calls: u32,
    query_calls: u32,
    observed_event_count: u32,
    total_response_bytes: usize,
    max_response_bytes: usize,
    max_response_method: String,
}

const GATE_J_PROGRESS_ENTITIES: &[&str] = &[
    "GameSession",
    "GameParticipant",
    "Champion",
    "Town",
    "MapChunk",
    "VisibilityChunk",
    "ParticipantObjectVisit",
    "ObjectiveProgress",
    "QuestState",
    "WorldEventState",
    "ScenarioRuleState",
    "SkirmishSettingsState",
    "ProceduralMapState",
    "NavalRouteState",
    "SiegeRuleState",
    "ResourceLedgerEntry",
    "ResourceLedgerTurnSummary",
    "TownBuilding",
    "TownGarrisonStack",
    "MovementSnapshot",
    "Battle",
    "BattleStack",
    "BattleOccupancy",
    "BattleObstacle",
];

const GATE_J_COMMAND_EVENT_ENTITIES: &[&str] = &["GameCommand", "LobbyCommand", "GameEvent"];

const GATE_K_ENTITIES: &[&str] = &[
    "Battle",
    "BattleStack",
    "BattleOccupancy",
    "BattleObstacle",
    "GameCommand",
    "GameEvent",
    "CommandEffect",
    "Champion",
    "ChampionArmyStack",
    "Town",
    "TownGarrisonStack",
    "WorldObject",
    "NeutralArmy",
    "PlayerMatchSummary",
];

const GATE_L_ENTITIES: &[&str] = &[
    "GameSession",
    "GameParticipant",
    "PlayerAccount",
    "PlayerMatchSummary",
    "LobbyCommand",
    "GameCommand",
    "CommandEffect",
    "GameEvent",
    "MapChunk",
    "VisibilityChunk",
    "MapOccupancy",
    "ParticipantObjectVisit",
    "ResourceLedgerEntry",
    "ResourceLedgerTurnSummary",
    "MovementIntent",
    "MovementSnapshot",
    "Town",
    "TownBuilding",
    "TownRecruitPool",
    "TownGarrisonStack",
    "Champion",
    "ChampionArmyStack",
    "Battle",
    "BattleStack",
    "BattleOccupancy",
    "BattleObstacle",
    "WorldObject",
    "NeutralArmy",
    "ObjectiveProgress",
    "QuestState",
    "WorldEventState",
    "ScenarioRuleState",
    "SkirmishSettingsState",
    "ProceduralMapState",
    "NavalRouteState",
    "SiegeRuleState",
];

impl GateJMetrics {
    fn record_query<T: candid::CandidType>(
        &mut self,
        method: &str,
        response: &Result<T, ApiError>,
    ) {
        self.query_calls = self.query_calls.saturating_add(1);
        self.record_response(method, response);
    }

    fn record_update<T: candid::CandidType>(
        &mut self,
        method: &str,
        response: &Result<T, ApiError>,
    ) {
        self.update_calls = self.update_calls.saturating_add(1);
        self.record_response(method, response);
    }

    fn observe_lobby_response(&mut self, response: &LobbyCommandResponse) {
        self.observed_event_count = self
            .observed_event_count
            .saturating_add(response.events.len() as u32);
    }

    fn observe_command_response(&mut self, response: &CommandResponse) {
        self.observed_event_count = self
            .observed_event_count
            .saturating_add(response.events.len() as u32);
    }

    fn observe_event_page(&mut self, page: &ApiEventPage) {
        self.observed_event_count = self
            .observed_event_count
            .saturating_add(page.events.len() as u32);
    }

    fn print_report(
        &self,
        initial_storage: &DiagnosticStorageSnapshot,
        final_storage: &DiagnosticStorageSnapshot,
        command_storage: &DiagnosticStorageSnapshot,
    ) {
        self.print_named_report("Gate J", initial_storage, final_storage, command_storage);
    }

    fn print_named_report(
        &self,
        label: &str,
        initial_storage: &DiagnosticStorageSnapshot,
        final_storage: &DiagnosticStorageSnapshot,
        command_storage: &DiagnosticStorageSnapshot,
    ) {
        let row_command_count = row_count(command_storage, "GameCommand")
            .saturating_add(row_count(command_storage, "LobbyCommand"));
        let row_growth = final_storage
            .total_rows
            .saturating_sub(initial_storage.total_rows);
        eprintln!(
            "{label} Pocket-IC metrics: updates={} queries={} observed_events={} row_commands={} row_events={} total_rows={} row_growth={} stable_pages_start={} stable_pages_final={} response_bytes_total={} max_response_bytes={} max_response_method={}",
            self.update_calls,
            self.query_calls,
            self.observed_event_count,
            row_command_count,
            row_count(command_storage, "GameEvent"),
            final_storage.total_rows,
            row_growth,
            initial_storage.stable_memory_pages,
            final_storage.stable_memory_pages,
            self.total_response_bytes,
            self.max_response_bytes,
            self.max_response_method
        );
    }

    fn record_response<T: candid::CandidType>(
        &mut self,
        method: &str,
        response: &Result<T, ApiError>,
    ) {
        let byte_len = candid::encode_one(response)
            .unwrap_or_else(|error| panic!("{method} response should Candid encode: {error}"))
            .len();
        self.total_response_bytes = self.total_response_bytes.saturating_add(byte_len);
        if byte_len > self.max_response_bytes {
            self.max_response_bytes = byte_len;
            self.max_response_method = method.to_string();
        }
    }
}

fn gate_start_active_two_player_session(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player_one: candid::Principal,
    player_two: candid::Principal,
    nonce_stem: &str,
) -> String {
    let registered_one = gate_update_as::<LobbyCommandResponse>(
        metrics,
        fixture,
        player_one,
        "register_player",
        (
            Some(format!("{nonce_stem}-one")),
            Some(format!("{nonce_stem} One")),
            format!("nonce:{nonce_stem}:register:one"),
        ),
    )
    .expect("player one registration should succeed");
    metrics.observe_lobby_response(&registered_one);

    let registered_two = gate_update_as::<LobbyCommandResponse>(
        metrics,
        fixture,
        player_two,
        "register_player",
        (
            Some(format!("{nonce_stem}-two")),
            Some(format!("{nonce_stem} Two")),
            format!("nonce:{nonce_stem}:register:two"),
        ),
    )
    .expect("player two registration should succeed");
    metrics.observe_lobby_response(&registered_two);

    let created = gate_update_as::<LobbyCommandResponse>(
        metrics,
        fixture,
        player_one,
        "create_session",
        (
            format!("{nonce_stem} Match"),
            FIRST_PLAYABLE_RULESET_ID.to_string(),
            1_u64,
            format!("nonce:{nonce_stem}:create"),
        ),
    )
    .expect("create_session should succeed");
    metrics.observe_lobby_response(&created);
    let session_id = match created.result {
        LobbyCommandResult::Session(session) => session.session_id,
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    let joined = gate_update_as::<LobbyCommandResponse>(
        metrics,
        fixture,
        player_two,
        "join_session",
        (
            session_id.clone(),
            "faction:ashen-ledger".to_string(),
            format!("nonce:{nonce_stem}:join"),
        ),
    )
    .expect("join_session should succeed");
    metrics.observe_lobby_response(&joined);

    let ready_one = gate_update_as::<LobbyCommandResponse>(
        metrics,
        fixture,
        player_one,
        "mark_ready",
        (session_id.clone(), format!("nonce:{nonce_stem}:ready:one")),
    )
    .expect("player one ready should succeed");
    metrics.observe_lobby_response(&ready_one);

    let ready_two = gate_update_as::<LobbyCommandResponse>(
        metrics,
        fixture,
        player_two,
        "mark_ready",
        (session_id.clone(), format!("nonce:{nonce_stem}:ready:two")),
    )
    .expect("player two ready should succeed");
    metrics.observe_lobby_response(&ready_two);

    for step in 0..18 {
        let started = gate_update_as::<LobbyCommandResponse>(
            metrics,
            fixture,
            player_one,
            "start_session",
            (
                session_id.clone(),
                format!("nonce:{nonce_stem}:start:{step}"),
            ),
        )
        .expect("start_session should succeed");
        metrics.observe_lobby_response(&started);
        assert_eq!(started.status, CommandStatus::Applied);
        if matches!(
            started.result,
            LobbyCommandResult::Session(ref session) if session.state == "active"
        ) {
            return session_id;
        }
    }

    panic!("phased start_session should finish setup");
}

fn gate_owned_champion_id(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
) -> String {
    let champions = gate_query_as::<Vec<ChampionView>>(
        metrics,
        fixture,
        player,
        "get_my_champions",
        (session_id.to_string(),),
    )
    .expect("owned champions should load");
    assert_eq!(champions.len(), 1);
    champions[0].champion_id.clone()
}

fn gate_submit_move_intent(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    client_nonce: &str,
) -> CommandResponse {
    let response = gate_update_as::<CommandResponse>(
        metrics,
        fixture,
        player,
        "submit_move_intent",
        (
            session_id.to_string(),
            champion_id.to_string(),
            path,
            client_nonce.to_string(),
        ),
    )
    .expect("submit_move_intent should succeed");
    metrics.observe_command_response(&response);
    assert_eq!(response.status, CommandStatus::Applied);
    response
}

fn gate_submit_move_and_sync_until_event(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    move_nonce: &str,
    sync_nonce_prefix: &str,
    now_ms: u64,
    expected_event_type: &str,
) -> (CommandResponse, bool) {
    let max_sync_calls = path.len().saturating_add(2);
    gate_submit_move_intent(
        metrics,
        fixture,
        player,
        session_id,
        champion_id,
        path,
        move_nonce,
    );
    gate_sync_until_event(
        metrics,
        fixture,
        player,
        session_id,
        sync_nonce_prefix,
        now_ms,
        expected_event_type,
        max_sync_calls,
    )
}

fn gate_sync_until_event(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    sync_nonce_prefix: &str,
    _now_ms: u64,
    expected_event_type: &str,
    max_sync_calls: usize,
) -> (CommandResponse, bool) {
    advance_time_ms(fixture, 61_000);
    let mut saw_partial_sync = false;
    for attempt in 0..max_sync_calls {
        let synced = gate_update_as::<CommandResponse>(
            metrics,
            fixture,
            player,
            "sync_session_turn",
            (
                session_id.to_string(),
                format!("{sync_nonce_prefix}{attempt}"),
            ),
        )
        .expect("sync_session_turn should succeed");
        metrics.observe_command_response(&synced);
        assert_eq!(synced.status, CommandStatus::Applied);
        saw_partial_sync |= synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete");
        if synced
            .events
            .iter()
            .any(|event| event.event_type == expected_event_type)
        {
            return (synced, saw_partial_sync);
        }
    }

    panic!("sync_session_turn did not emit {expected_event_type} after {max_sync_calls} calls");
}

fn gate_submit_retryable_battle_action(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    battle_id: &str,
    action_nonce: &str,
    sync_nonce_prefix: &str,
) -> (CommandResponse, bool) {
    let mut saw_sync_battle = false;
    for step in 0..16 {
        let view = gate_query_as::<BattleView>(
            metrics,
            fixture,
            player,
            "get_battle_state",
            (session_id.to_string(), battle_id.to_string()),
        )
        .expect("battle view should load before retryable action");
        assert_ne!(view.state, "resolved");
        if view.legal_actions_for_caller.is_empty() {
            advance_time_ms(fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
            let synced = gate_update_as::<CommandResponse>(
                metrics,
                fixture,
                player,
                "sync_battle",
                (
                    session_id.to_string(),
                    battle_id.to_string(),
                    format!("{sync_nonce_prefix}{step}"),
                ),
            )
            .expect("sync_battle before retryable action should succeed");
            metrics.observe_command_response(&synced);
            assert_eq!(synced.status, CommandStatus::Applied);
            saw_sync_battle = true;
            continue;
        }

        let input = choose_battle_action(&view);
        let submitted = gate_update_as::<CommandResponse>(
            metrics,
            fixture,
            player,
            "submit_battle_action",
            (
                session_id.to_string(),
                input.clone(),
                action_nonce.to_string(),
            ),
        )
        .expect("retryable battle action should succeed");
        metrics.observe_command_response(&submitted);
        assert_eq!(
            submitted.status,
            CommandStatus::Applied,
            "battle action response: {submitted:?}"
        );
        let replay = gate_update_as::<CommandResponse>(
            metrics,
            fixture,
            player,
            "submit_battle_action",
            (session_id.to_string(), input, action_nonce.to_string()),
        )
        .expect("retryable battle action replay should succeed");
        metrics.observe_command_response(&replay);
        assert_eq!(replay.command_id, submitted.command_id);
        assert_eq!(replay.status, CommandStatus::Applied);
        return (submitted, saw_sync_battle);
    }

    panic!("battle {battle_id} did not expose a retryable caller action");
}

fn gate_resolve_battle_to_end(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    battle_id: &str,
    nonce_prefix: &str,
) -> (BattleView, bool) {
    for step in 0..96 {
        let view = gate_query_as::<BattleView>(
            metrics,
            fixture,
            player,
            "get_battle_state",
            (session_id.to_string(), battle_id.to_string()),
        )
        .expect("battle view should load");
        if view.state == "resolved" {
            let synced = gate_update_as::<CommandResponse>(
                metrics,
                fixture,
                player,
                "sync_battle",
                (
                    session_id.to_string(),
                    battle_id.to_string(),
                    format!("{nonce_prefix}:aftermath"),
                ),
            )
            .expect("resolved battle aftermath sync should succeed");
            metrics.observe_command_response(&synced);
            assert_eq!(synced.status, CommandStatus::Applied);
            let turn_synced = update_as::<CommandResponse>(
                fixture,
                player,
                "sync_session_turn",
                (
                    session_id.to_string(),
                    format!("{nonce_prefix}:post-battle-turn"),
                ),
            )
            .unwrap_or_else(|error| panic!("post-battle sync_session_turn should decode: {error}"));
            metrics.record_update("sync_session_turn", &turn_synced);
            match turn_synced {
                Ok(response) => {
                    metrics.observe_command_response(&response);
                    assert_eq!(response.status, CommandStatus::Applied);
                }
                Err(error) if error.code == "turn_not_due" => {}
                Err(error) => {
                    panic!("post-battle sync_session_turn should succeed or be not due: {error:?}")
                }
            }
            return (view, true);
        }
        if view.legal_actions_for_caller.is_empty() {
            advance_time_ms(fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
            let synced = gate_update_as::<CommandResponse>(
                metrics,
                fixture,
                player,
                "sync_battle",
                (
                    session_id.to_string(),
                    battle_id.to_string(),
                    format!("{nonce_prefix}:sync:{step}"),
                ),
            )
            .expect("sync_battle should succeed");
            metrics.observe_command_response(&synced);
            assert_eq!(synced.status, CommandStatus::Applied);
            continue;
        }

        let input = choose_battle_action(&view);
        let submitted = gate_update_as::<CommandResponse>(
            metrics,
            fixture,
            player,
            "submit_battle_action",
            (
                session_id.to_string(),
                input,
                format!("{nonce_prefix}:action:{step}"),
            ),
        )
        .expect("submit_battle_action should succeed");
        metrics.observe_command_response(&submitted);
        assert_eq!(
            submitted.status,
            CommandStatus::Applied,
            "battle action response: {submitted:?}"
        );
        let synced = gate_update_as::<CommandResponse>(
            metrics,
            fixture,
            player,
            "sync_battle",
            (
                session_id.to_string(),
                battle_id.to_string(),
                format!("{nonce_prefix}:after-action-sync:{step}"),
            ),
        )
        .expect("post-action sync_battle should succeed");
        metrics.observe_command_response(&synced);
        assert_eq!(synced.status, CommandStatus::Applied);
    }

    panic!("battle {battle_id} did not resolve within the test budget");
}

fn gate_diagnostic_snapshot(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    entities: &[&str],
) -> DiagnosticStorageSnapshot {
    let mut combined = DiagnosticStorageSnapshot {
        row_counts: Vec::new(),
        total_rows: 0,
        stable_memory_pages: 0,
    };

    for entity in entities {
        let snapshot = gate_query_as::<DiagnosticStorageSnapshot>(
            metrics,
            fixture,
            candid::Principal::anonymous(),
            "get_diagnostic_storage_snapshot",
            (entity_names(&[*entity]),),
        )
        .expect("controller diagnostic storage snapshot should load");
        assert_eq!(snapshot.row_counts.len(), 1);
        combined.total_rows = combined.total_rows.saturating_add(snapshot.total_rows);
        combined.stable_memory_pages = combined
            .stable_memory_pages
            .max(snapshot.stable_memory_pages);
        combined.row_counts.extend(snapshot.row_counts);
    }

    combined
}

fn diagnostic_snapshot(
    fixture: &StandaloneCanisterFixture,
    entities: &[&str],
) -> DiagnosticStorageSnapshot {
    let mut combined = DiagnosticStorageSnapshot {
        row_counts: Vec::new(),
        total_rows: 0,
        stable_memory_pages: 0,
    };

    for entity in entities {
        let snapshot = query_as::<DiagnosticStorageSnapshot>(
            fixture,
            candid::Principal::anonymous(),
            "get_diagnostic_storage_snapshot",
            (entity_names(&[*entity]),),
        )
        .expect("diagnostic storage snapshot should decode")
        .expect("diagnostic storage snapshot should load");
        assert_eq!(snapshot.row_counts.len(), 1);
        combined.total_rows = combined.total_rows.saturating_add(snapshot.total_rows);
        combined.stable_memory_pages = combined
            .stable_memory_pages
            .max(snapshot.stable_memory_pages);
        combined.row_counts.extend(snapshot.row_counts);
    }

    combined
}

fn entity_names(entities: &[&str]) -> Vec<String> {
    entities
        .iter()
        .map(|entity| (*entity).to_string())
        .collect()
}

fn row_count(snapshot: &DiagnosticStorageSnapshot, entity: &str) -> u32 {
    snapshot
        .row_counts
        .iter()
        .find(|row| row.entity == entity)
        .unwrap_or_else(|| panic!("diagnostic row count missing {entity}"))
        .count
}

fn advance_time_ms(fixture: &StandaloneCanisterFixture, millis: u64) {
    fixture.pic().advance_time(Duration::from_millis(millis));
    fixture.pic().tick();
}

fn json_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!(r#""{field}":"#);
    let start = json.find(&needle)? + needle.len();
    let rest = json.get(start..)?.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_string())
}

fn owned_champion_id(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
) -> String {
    let champions = query_as::<Vec<ChampionView>>(
        fixture,
        player,
        "get_my_champions",
        (session_id.to_string(),),
    )
    .expect("get_my_champions should decode")
    .expect("owned champions should load");
    assert_eq!(champions.len(), 1);
    champions[0].champion_id.clone()
}

fn submit_move_intent(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    client_nonce: &str,
    _now_ms: u64,
) -> CommandResponse {
    let response = update_as::<CommandResponse>(
        fixture,
        player,
        "submit_move_intent",
        (
            session_id.to_string(),
            champion_id.to_string(),
            path,
            client_nonce.to_string(),
        ),
    )
    .expect("submit_move_intent should decode")
    .expect("submit_move_intent should succeed");
    assert_eq!(response.status, CommandStatus::Applied);
    response
}

fn submit_move_and_sync_until_event(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    move_nonce: &str,
    sync_nonce_prefix: &str,
    now_ms: u64,
    expected_event_type: &str,
) -> (CommandResponse, bool) {
    let max_sync_calls = path.len().saturating_add(2);
    submit_move_intent(
        fixture,
        player,
        session_id,
        champion_id,
        path,
        move_nonce,
        now_ms,
    );
    sync_until_event(
        fixture,
        player,
        session_id,
        sync_nonce_prefix,
        now_ms,
        expected_event_type,
        max_sync_calls,
    )
}

fn sync_until_event(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    sync_nonce_prefix: &str,
    _now_ms: u64,
    expected_event_type: &str,
    max_sync_calls: usize,
) -> (CommandResponse, bool) {
    advance_time_ms(fixture, 61_000);
    let mut saw_partial_sync = false;
    for attempt in 0..max_sync_calls {
        let synced = update_as::<CommandResponse>(
            fixture,
            player,
            "sync_session_turn",
            (
                session_id.to_string(),
                format!("{sync_nonce_prefix}{attempt}"),
            ),
        )
        .expect("sync_session_turn should decode")
        .expect("sync_session_turn should succeed");
        assert_eq!(synced.status, CommandStatus::Applied);
        saw_partial_sync |= synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete");
        if synced
            .events
            .iter()
            .any(|event| event.event_type == expected_event_type)
        {
            return (synced, saw_partial_sync);
        }
    }

    panic!("sync_session_turn did not emit {expected_event_type} after {max_sync_calls} calls");
}

fn resolve_battle_to_end(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    battle_id: &str,
    nonce_prefix: &str,
) -> BattleView {
    for step in 0..96 {
        let view = query_as::<BattleView>(
            fixture,
            player,
            "get_battle_state",
            (session_id.to_string(), battle_id.to_string()),
        )
        .expect("battle view should decode")
        .expect("battle view should load");
        if view.state == "resolved" {
            let synced = update_as::<CommandResponse>(
                fixture,
                player,
                "sync_battle",
                (
                    session_id.to_string(),
                    battle_id.to_string(),
                    format!("{nonce_prefix}:aftermath"),
                ),
            )
            .expect("resolved battle aftermath sync should decode")
            .expect("resolved battle aftermath sync should succeed");
            assert_eq!(synced.status, CommandStatus::Applied);
            let turn_synced = update_as::<CommandResponse>(
                fixture,
                player,
                "sync_session_turn",
                (
                    session_id.to_string(),
                    format!("{nonce_prefix}:post-battle-turn"),
                ),
            )
            .expect("post-battle sync_session_turn should decode");
            match turn_synced {
                Ok(response) => assert_eq!(response.status, CommandStatus::Applied),
                Err(error) if error.code == "turn_not_due" => {}
                Err(error) => {
                    panic!("post-battle sync_session_turn should succeed or be not due: {error:?}")
                }
            }
            return view;
        }
        if view.legal_actions_for_caller.is_empty() {
            advance_time_ms(fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
            let synced = update_as::<CommandResponse>(
                fixture,
                player,
                "sync_battle",
                (
                    session_id.to_string(),
                    battle_id.to_string(),
                    format!("{nonce_prefix}:sync:{step}"),
                ),
            )
            .expect("sync_battle should decode")
            .expect("sync_battle should succeed");
            assert_eq!(synced.status, CommandStatus::Applied);
            continue;
        }

        let input = choose_battle_action(&view);
        let submitted = update_as::<CommandResponse>(
            fixture,
            player,
            "submit_battle_action",
            (
                session_id.to_string(),
                input,
                format!("{nonce_prefix}:action:{step}"),
            ),
        )
        .expect("submit_battle_action should decode")
        .expect("submit_battle_action should succeed");
        assert_eq!(
            submitted.status,
            CommandStatus::Applied,
            "battle action response: {submitted:?}"
        );
        let synced = update_as::<CommandResponse>(
            fixture,
            player,
            "sync_battle",
            (
                session_id.to_string(),
                battle_id.to_string(),
                format!("{nonce_prefix}:after-action-sync:{step}"),
            ),
        )
        .expect("post-action sync_battle should decode")
        .expect("post-action sync_battle should succeed");
        assert_eq!(synced.status, CommandStatus::Applied);
    }

    panic!("battle {battle_id} did not resolve within the test budget");
}

fn choose_battle_action(view: &BattleView) -> BattleActionInput {
    let active_stack_id = view
        .active_stack_id
        .clone()
        .expect("active battle should have an active stack");
    for preferred in ["RangedAttack", "MeleeAttack", "Attack"] {
        if let Some(action) = view.legal_actions_for_caller.iter().find(|action| {
            action.enabled && action.action == preferred && !action.targets.is_empty()
        }) {
            return BattleActionInput {
                battle_id: view.battle_id.clone(),
                battle_stack_id: active_stack_id,
                action: action.action.clone(),
                ability_key: action.ability_key.clone(),
                target_stack_id: action.targets.first().cloned(),
                destination: None,
            };
        }
    }
    if let Some(action) = view
        .legal_actions_for_caller
        .iter()
        .find(|action| action.enabled && action.action == "Move" && !action.path.is_empty())
    {
        return BattleActionInput {
            battle_id: view.battle_id.clone(),
            battle_stack_id: active_stack_id,
            action: "Move".to_string(),
            ability_key: None,
            target_stack_id: None,
            destination: best_move_destination(view, action),
        };
    }
    let action = view
        .legal_actions_for_caller
        .iter()
        .find(|action| action.enabled)
        .expect("caller should have at least one enabled battle action");
    BattleActionInput {
        battle_id: view.battle_id.clone(),
        battle_stack_id: active_stack_id,
        action: action.action.clone(),
        ability_key: action.ability_key.clone(),
        target_stack_id: action.targets.first().cloned(),
        destination: action.path.first().copied(),
    }
}

fn best_move_destination(
    view: &BattleView,
    action: &domm_game::LegalBattleAction,
) -> Option<domm_game::BattleCoord> {
    let active_stack_id = view.active_stack_id.as_deref()?;
    let active_side = view
        .stacks
        .iter()
        .find(|stack| stack.battle_stack_id == active_stack_id)?
        .side
        .clone();
    action.path.iter().copied().min_by_key(|coord| {
        view.stacks
            .iter()
            .filter(|stack| {
                stack.side != active_side && stack.status == "active" && stack.quantity > 0
            })
            .map(|enemy| {
                u16::from(coord.x.abs_diff(enemy.battle_x))
                    + u16::from(coord.y.abs_diff(enemy.battle_y))
            })
            .min()
            .unwrap_or(u16::MAX)
    })
}

fn battle_id_from_events(response: &CommandResponse, event_type: &str) -> String {
    response
        .events
        .iter()
        .find(|event| event.event_type == event_type)
        .and_then(|event| event.payload.as_deref())
        .and_then(|payload| json_string_field(payload, "battle_id"))
        .unwrap_or_else(|| panic!("{event_type} event should include battle_id"))
}

fn query_as<T>(
    fixture: &StandaloneCanisterFixture,
    caller: candid::Principal,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) -> Result<Result<T, ApiError>, String>
where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    fixture
        .pic()
        .query_call_as(fixture.canister_id(), caller, method, args)
        .map_err(|error| format!("{error:?}"))
}

fn gate_query_as<T>(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    caller: candid::Principal,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) -> Result<T, ApiError>
where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    let response = query_as::<T>(fixture, caller, method, args)
        .unwrap_or_else(|error| panic!("{method} should decode from query call: {error}"));
    metrics.record_query(method, &response);
    response
}

fn update_as<T>(
    fixture: &StandaloneCanisterFixture,
    caller: candid::Principal,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) -> Result<Result<T, ApiError>, String>
where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    fixture
        .pic()
        .update_call_as(fixture.canister_id(), caller, method, args)
        .map_err(|error| format!("{error:?}"))
}

fn gate_update_as<T>(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    caller: candid::Principal,
    method: &str,
    args: impl candid::utils::ArgumentEncoder,
) -> Result<T, ApiError>
where
    T: candid::CandidType + for<'de> serde::Deserialize<'de>,
{
    let response = update_as::<T>(fixture, caller, method, args)
        .unwrap_or_else(|error| panic!("{method} should decode from update call: {error}"));
    metrics.record_update(method, &response);
    response
}

fn install_degens_canister_fixture() -> StandaloneCanisterFixture {
    let wasm_path = build_degens_canister();
    let wasm = fs::read(&wasm_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", wasm_path.display()));
    install_prebuilt_canister_with_cycles(
        wasm,
        candid::encode_args(()).expect("empty init args encode"),
        100_000_000_000_000,
    )
}

fn build_degens_canister() -> PathBuf {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");
    let target_dir = workspace_root.join("target/pocket-ic-endpoint-presence");
    let linker_wrapper_dir = write_host_linker_wrapper(&target_dir);
    let nested_path = path_with_prefix(&linker_wrapper_dir);
    let output = Command::new("cargo")
        .current_dir(&workspace_root)
        .args([
            "build",
            "-p",
            "domm-degens-canister",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("PATH", nested_path)
        .output()
        .expect("failed to run cargo build for degens canister");
    assert!(
        output.status.success(),
        "canister wasm build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("domm_degens_canister.wasm")
}

fn write_host_linker_wrapper(target_dir: &Path) -> PathBuf {
    let wrapper_dir = target_dir.join("host-linker-wrapper");
    fs::create_dir_all(&wrapper_dir)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", wrapper_dir.display()));
    let wrapper_path = wrapper_dir.join("cc");
    let real_cc = system_cc_path();
    fs::write(
        &wrapper_path,
        format!(
            "#!/bin/sh\nexec '{}' \"$@\" -fuse-ld=bfd\n",
            shell_single_quote(&real_cc)
        ),
    )
    .unwrap_or_else(|error| panic!("failed to write {}: {error}", wrapper_path.display()));
    make_executable(&wrapper_path);
    wrapper_dir
}

fn system_cc_path() -> String {
    let output = Command::new("sh")
        .args(["-c", "command -v cc"])
        .output()
        .expect("failed to resolve system cc");
    assert!(
        output.status.success(),
        "failed to resolve system cc\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("system cc path should be UTF-8")
        .trim()
        .to_string()
}

fn shell_single_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn path_with_prefix(prefix: &Path) -> std::ffi::OsString {
    let mut paths = vec![prefix.to_path_buf()];
    if let Some(existing_path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&existing_path));
    }
    env::join_paths(paths).expect("nested PATH should join")
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .unwrap_or_else(|error| panic!("failed to stat {}: {error}", path.display()))
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|error| panic!("failed to chmod {}: {error}", path.display()));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
