use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use canic_testkit::pic::{StandaloneCanisterFixture, install_prebuilt_canister_with_cycles};
use domm_degens_canister::{
    CanisterEndpointView, DiagnosticStorageSnapshot, DiagnosticSystemJobPage,
    DiagnosticSystemJobView, REQUIRED_GAME_ENDPOINTS,
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
    RecruitPreview, RecruitTarget, ScenarioRulesView, SessionView, SetupProgressView,
    SiegeRulesView, SkirmishSettingsView, TavernOffersView, Viewport, WorldEventsView,
    opening_viewport_for_slot,
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

    let active_start_nonce = "nonce:presence:start".to_string();
    let active_start =
        start_session_once_and_wait_active(&fixture, player_one, &session_id, &active_start_nonce);
    let setup_progress = query_as::<SetupProgressView>(
        &fixture,
        player_one,
        "get_setup_progress",
        (session_id.clone(),),
    )
    .expect("get_setup_progress should decode")
    .expect("setup progress should be readable");
    assert_eq!(setup_progress.session_id, session_id);
    assert_eq!(setup_progress.session_state, "active");
    assert!(setup_progress.setup_complete);
    assert_eq!(
        setup_progress.completed_effect_count,
        setup_progress.total_effect_count
    );
    assert!(setup_progress.next_effect_key.is_none());
    assert_eq!(
        setup_progress.setup_command_status.as_deref(),
        Some("applied")
    );

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
        61_000,
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

    let _income_sync = sync_turn_until_event(
        &fixture,
        player_one,
        &session_id,
        "nonce:presence:sync-turn:income:",
        "income_materialized",
        4,
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
        advance_time_without_timers(&fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
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
fn pocket_ic_one_call_setup_progress_replay_and_upgrade_resume() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-setup-gate-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-setup-gate-player-two");
    let session_id =
        create_ready_two_player_session(&fixture, player_one, player_two, "setup-gate");
    let start_nonce = "nonce:setup-gate:start".to_string();

    let started = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "start_session",
        (session_id.clone(), start_nonce.clone()),
    )
    .expect("one-call setup start should decode")
    .expect("one-call setup start should succeed");
    assert_eq!(started.status, CommandStatus::Applied);
    match &started.result {
        LobbyCommandResult::Session(session) => assert_eq!(session.state, "starting"),
        other => panic!("start_session returned unexpected result: {other:?}"),
    }

    let initial_progress = setup_progress(&fixture, player_one, &session_id);
    assert_eq!(initial_progress.session_state, "starting");
    assert!(!initial_progress.setup_complete);
    assert!(initial_progress.total_effect_count > 1);
    assert_eq!(initial_progress.completed_effect_count, 0);
    assert_eq!(
        initial_progress.next_effect_key.as_deref(),
        Some("seed_ruleset_content")
    );
    assert_eq!(
        initial_progress.setup_command_status.as_deref(),
        Some("pending")
    );
    assert_eq!(
        initial_progress.setup_job_status.as_deref(),
        Some("scheduled")
    );

    let replayed = update_as::<LobbyCommandResponse>(
        &fixture,
        player_one,
        "start_session",
        (session_id.clone(), start_nonce),
    )
    .expect("start replay while starting should decode")
    .expect("start replay while starting should succeed");
    assert_eq!(replayed.command_id, started.command_id);
    let replayed_progress = setup_progress(&fixture, player_one, &session_id);
    assert!(replayed_progress.completed_effect_count >= initial_progress.completed_effect_count);
    assert!(
        replayed_progress.completed_effect_count < replayed_progress.total_effect_count,
        "setup should still be starting before the upgrade checkpoint: {replayed_progress:?}"
    );
    assert_eq!(
        replayed_progress.setup_command_id,
        initial_progress.setup_command_id
    );
    let replay_jobs = diagnostic_system_jobs(&fixture, Some(session_id.clone()), None);
    assert_eq!(
        replay_jobs
            .jobs
            .iter()
            .filter(|job| job.job_key == format!("setup_session:{session_id}"))
            .count(),
        1,
        "replaying the start nonce must not duplicate the setup job: {:?}",
        replay_jobs.jobs
    );

    upgrade_degens_canister(&fixture);
    let mut progress = setup_progress(&fixture, player_one, &session_id);
    assert!(progress.completed_effect_count >= replayed_progress.completed_effect_count);
    let mut observed_counts = vec![progress.completed_effect_count];
    for _ in 0..80 {
        if progress.setup_complete {
            break;
        }
        advance_time_for_timers(&fixture, 5);
        progress = setup_progress(&fixture, player_one, &session_id);
        if observed_counts.last() != Some(&progress.completed_effect_count) {
            observed_counts.push(progress.completed_effect_count);
        }
    }

    assert!(
        progress.setup_complete,
        "setup progress after ticks: {progress:?}"
    );
    assert_eq!(
        progress.completed_effect_count, progress.total_effect_count,
        "completed setup should report all setup effects"
    );
    assert!(
        observed_counts
            .iter()
            .any(|count| *count > 0 && *count < progress.total_effect_count),
        "setup should expose intermediate progress across fresh timer messages: {observed_counts:?}"
    );
    let active =
        query_as::<SessionView>(&fixture, player_one, "get_session", (session_id.clone(),))
            .expect("post-upgrade setup session should decode")
            .expect("post-upgrade setup session should load");
    assert_eq!(active.state, "active");
    let setup_jobs = diagnostic_system_jobs(&fixture, Some(session_id.clone()), None);
    assert!(
        setup_jobs.jobs.iter().any(|job| {
            job.job_key == format!("setup_session:{session_id}") && job.status == "completed"
        }),
        "setup job should complete after post-upgrade timer resume: {:?}",
        setup_jobs.jobs
    );
}

#[test]
fn pocket_ic_timer_jobs_repair_deadlines_and_recover_expired_leases() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-timer-jobs-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-timer-jobs-player-two");
    let session_id = start_active_two_player_session(&fixture, player_one, player_two, "timers");

    let initial_view = compact_game_view(&fixture, player_one, &session_id);
    assert_eq!(initial_view.session.current_turn, 1);
    assert!(!initial_view.render_time.sync_required);

    let system_job_snapshot = diagnostic_snapshot(&fixture, &["SystemJob"]);
    assert!(
        row_count(&system_job_snapshot, "SystemJob") > 0,
        "active session setup should schedule durable system jobs"
    );

    let turn_one_key = turn_deadline_job_key(&session_id, 1);
    let turn_one_job = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("scheduled".to_string()),
    )
    .jobs
    .into_iter()
    .find(|job| job.job_key == turn_one_key)
    .expect("turn one deadline job should be scheduled");
    assert_eq!(turn_one_job.job_kind, "turn_deadline");
    assert_eq!(turn_one_job.turn_number, Some(1));

    upgrade_degens_canister(&fixture);
    let repaired_turn_one = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("scheduled".to_string()),
    );
    assert!(
        repaired_turn_one
            .jobs
            .iter()
            .any(|job| job.job_key == turn_one_key && job.status == "scheduled"),
        "post-upgrade repair should preserve or recreate the active turn deadline"
    );

    advance_time_for_timers(
        &fixture,
        millis_until_due(
            initial_view.render_time.server_now_ms,
            turn_one_job.due_at_ms,
        ),
    );
    replay_player_registration(&fixture, player_one, "timers", "one");
    let after_upgrade_timer = compact_game_view(&fixture, player_one, &session_id);
    assert_eq!(after_upgrade_timer.session.current_turn, 2);
    assert!(!after_upgrade_timer.render_time.sync_required);

    let duplicate_turn_one = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_session_turn",
        (
            session_id.clone(),
            "nonce:timers:duplicate:turn-one".to_string(),
        ),
    )
    .expect("duplicate sync after timer should decode")
    .expect("duplicate sync after timer should return a command response");
    assert_eq!(duplicate_turn_one.status, CommandStatus::Failed);
    assert_eq!(
        duplicate_turn_one
            .error
            .as_ref()
            .map(|error| error.code.as_str()),
        Some("turn_not_due")
    );

    let turn_two_key = turn_deadline_job_key(&session_id, 2);
    let turn_two_job = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("scheduled".to_string()),
    )
    .jobs
    .into_iter()
    .find(|job| job.job_key == turn_two_key)
    .expect("turn two deadline job should be scheduled");

    let forced_running = force_system_job_running(
        &fixture,
        &turn_two_key,
        after_upgrade_timer
            .render_time
            .server_now_ms
            .saturating_sub(1),
    );
    assert_eq!(forced_running.status, "running");
    assert_eq!(
        forced_running.attempt_count,
        turn_two_job.attempt_count.saturating_add(1)
    );
    assert_eq!(forced_running.lease_owner.as_deref(), Some("diagnostic"));

    advance_time_for_timers(
        &fixture,
        millis_until_due(
            after_upgrade_timer.render_time.server_now_ms,
            turn_two_job.due_at_ms,
        ),
    );
    replay_player_registration(&fixture, player_one, "timers", "one");
    let after_lease_recovery = compact_game_view(&fixture, player_one, &session_id);
    assert_eq!(after_lease_recovery.session.current_turn, 3);

    let completed_jobs = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("completed".to_string()),
    );
    let recovered_job = completed_jobs
        .jobs
        .iter()
        .find(|job| job.job_key == turn_two_key)
        .expect("expired running turn deadline should recover and complete");
    assert_eq!(recovered_job.status, "completed");
    assert!(
        recovered_job.attempt_count > forced_running.attempt_count,
        "lease recovery should reclaim the running job before completing it"
    );

    let public_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 100_u32),
    )
    .expect("timer event feed should decode")
    .expect("timer event feed should load");
    let advanced_payloads = public_events
        .events
        .iter()
        .filter(|event| event.event_type == "session_turn_advanced")
        .filter_map(|event| event.payload.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(advanced_payloads.len(), 2);
    assert!(advanced_payloads[0].contains(r#""current_turn":2"#));
    assert!(advanced_payloads[1].contains(r#""current_turn":3"#));
}

#[test]
fn pocket_ic_timer_jobs_deadline_resolves_multistep_movement_without_sync() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-timer-move-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-timer-move-player-two");
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "timer-move");
    let champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let opening_view = compact_game_view(&fixture, player_one, &session_id);
    let turn_one_key = turn_deadline_job_key(&session_id, 1);
    let turn_one_job = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("scheduled".to_string()),
    )
    .jobs
    .into_iter()
    .find(|job| job.job_key == turn_one_key)
    .expect("turn one deadline job should be scheduled before movement");

    submit_move_intent(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        (9_u16..=18)
            .map(|x| MoveCoord::new(x, 24))
            .collect::<Vec<_>>(),
        "nonce:timer-move:submit",
        opening_view.render_time.server_now_ms,
    );

    advance_time_for_timers(
        &fixture,
        millis_until_due(
            opening_view.render_time.server_now_ms,
            turn_one_job.due_at_ms,
        ),
    );
    let mut after_deadline = compact_game_view(&fixture, player_one, &session_id);
    for _ in 0..12 {
        if after_deadline.session.current_turn >= 2 {
            break;
        }
        advance_time_for_timers(&fixture, 61_000);
        after_deadline = compact_game_view(&fixture, player_one, &session_id);
    }
    assert_eq!(
        after_deadline.session.current_turn, 2,
        "deadline timer should finish movement continuations without sync_session_turn"
    );

    let champion = query_as::<ChampionView>(
        &fixture,
        player_one,
        "get_champion_view",
        (session_id.clone(), champion_id),
    )
    .expect("timer-moved champion should decode")
    .expect("timer-moved champion should load");
    assert_eq!((champion.x, champion.y), (18, 24));

    let completed_jobs = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("completed".to_string()),
    );
    let completed_turn_one = completed_jobs
        .jobs
        .iter()
        .find(|job| job.job_key == turn_one_key)
        .expect("turn one deadline job should complete after movement");
    assert!(
        completed_turn_one.attempt_count > turn_one_job.attempt_count.saturating_add(1),
        "multi-step movement should require timer continuation attempts"
    );

    let public_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 100_u32),
    )
    .expect("timer movement event feed should decode")
    .expect("timer movement event feed should load");
    let turn_two_advanced_before_stale = public_events
        .events
        .iter()
        .filter(|event| event.event_type == "session_turn_advanced")
        .filter(|event| {
            event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""current_turn":2"#))
        })
        .count();
    assert_eq!(turn_two_advanced_before_stale, 1);
    assert!(
        public_events
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete"),
        "deadline-driven movement should expose partial movement progress"
    );

    force_system_job_running(&fixture, &turn_one_key, 0);
    let _ = run_diagnostic_system_job(&fixture, &turn_one_key);
    let stale_reclaimed = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("completed".to_string()),
    )
    .jobs
    .iter()
    .any(|job| job.job_key == turn_one_key);
    assert!(
        stale_reclaimed,
        "stale forced job should return to completed"
    );
    let public_events_after_stale = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id, "public".to_string(), 0_u64, 100_u32),
    )
    .expect("post-stale timer event feed should decode")
    .expect("post-stale timer event feed should load");
    let turn_two_advanced_after_stale = public_events_after_stale
        .events
        .iter()
        .filter(|event| event.event_type == "session_turn_advanced")
        .filter(|event| {
            event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""current_turn":2"#))
        })
        .count();
    assert_eq!(
        turn_two_advanced_after_stale, turn_two_advanced_before_stale,
        "stale duplicate timer processing must not duplicate turn-one effects"
    );
}

#[test]
fn pocket_ic_timer_jobs_refresh_scenario_maintenance_without_sync_wrappers() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-scenario-jobs-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-scenario-jobs-player-two");
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "scenario-jobs");

    let initial_rules = query_as::<ScenarioRulesView>(
        &fixture,
        player_one,
        "get_scenario_rules",
        (session_id.clone(),),
    )
    .expect("initial scenario rules should decode")
    .expect("initial scenario rules should load");
    let initial_max_turn = initial_rules
        .rules
        .iter()
        .find(|rule| rule.rule_key == "rule:max-turn")
        .expect("max-turn rule should be seeded");
    assert_eq!(initial_max_turn.current_value, 1);
    assert_eq!(initial_max_turn.last_checked_turn, 1);

    for turn in 1..=7 {
        for (player, suffix) in [(player_one, "one"), (player_two, "two")] {
            let ended = update_as::<CommandResponse>(
                &fixture,
                player,
                "end_turn",
                (
                    session_id.clone(),
                    format!("nonce:scenario-jobs:end-turn:{turn}:{suffix}"),
                ),
            )
            .expect("end_turn should decode")
            .expect("end_turn should succeed");
            assert_eq!(ended.status, CommandStatus::Applied);
        }

        let expected_turn = turn + 1;
        let expected_jobs = [
            format!("scenario_objectives:{session_id}:{expected_turn}"),
            format!("world_events:{session_id}:{expected_turn}"),
            format!("advanced_victory:{session_id}:{expected_turn}"),
        ];
        let mut view = compact_game_view(&fixture, player_one, &session_id);
        let mut jobs_done = false;
        let mut observed_jobs = Vec::new();
        for _ in 0..16 {
            advance_time_for_timers(&fixture, 1);
            view = compact_game_view(&fixture, player_one, &session_id);
            if view.session.current_turn == expected_turn {
                let completed = diagnostic_system_jobs(
                    &fixture,
                    Some(session_id.clone()),
                    Some("completed".to_string()),
                );
                jobs_done = expected_jobs
                    .iter()
                    .all(|key| completed.jobs.iter().any(|job| job.job_key == *key));
                if jobs_done {
                    break;
                }
                observed_jobs = diagnostic_system_jobs(&fixture, Some(session_id.clone()), None)
                    .jobs
                    .into_iter()
                    .map(|job| {
                        format!(
                            "{}:{}:{:?}",
                            job.job_key,
                            job.status,
                            job.last_error.as_deref()
                        )
                    })
                    .collect();
            }
        }
        assert_eq!(view.session.current_turn, turn + 1);
        assert!(
            jobs_done,
            "turn {turn} should complete scenario maintenance jobs {:?}; observed jobs: {:?}",
            expected_jobs, observed_jobs
        );
    }

    let objectives = query_as::<ObjectiveProgressView>(
        &fixture,
        player_one,
        "get_objective_progress",
        (session_id.clone(),),
    )
    .expect("job-refreshed objectives should decode")
    .expect("job-refreshed objectives should load");
    assert_eq!(objectives.objectives.len(), 2);
    assert!(
        objectives
            .objectives
            .iter()
            .any(|objective| objective.objective_key == "objective:north")
    );

    let world_events = query_as::<WorldEventsView>(
        &fixture,
        player_one,
        "get_world_events",
        (session_id.clone(),),
    )
    .expect("job-refreshed world events should decode")
    .expect("job-refreshed world events should load");
    assert!(
        world_events
            .events
            .iter()
            .any(|event| event.event_window == "week:2"),
        "week two world event should be materialized by the world_events job: {:?}",
        world_events.events
    );

    let rules = query_as::<ScenarioRulesView>(
        &fixture,
        player_one,
        "get_scenario_rules",
        (session_id.clone(),),
    )
    .expect("job-refreshed scenario rules should decode")
    .expect("job-refreshed scenario rules should load");
    let max_turn = rules
        .rules
        .iter()
        .find(|rule| rule.rule_key == "rule:max-turn")
        .expect("max-turn rule should remain visible");
    assert_eq!(max_turn.current_value, 8);
    assert_eq!(max_turn.last_checked_turn, 8);

    let completed_jobs = diagnostic_system_jobs(
        &fixture,
        Some(session_id.clone()),
        Some("completed".to_string()),
    );
    for (job_kind, touched) in [
        ("scenario_objectives", 2_u32),
        ("world_events", 1_u32),
        ("advanced_victory", 4_u32),
    ] {
        let job_key = format!("{job_kind}:{session_id}:8");
        let job = completed_jobs
            .jobs
            .iter()
            .find(|job| job.job_key == job_key)
            .unwrap_or_else(|| panic!("{job_key} should be completed: {:?}", completed_jobs.jobs));
        let command_id = job
            .command_id
            .as_ref()
            .unwrap_or_else(|| panic!("{job_key} should record its system command"));
        let status = query_as::<CommandStatusView>(
            &fixture,
            player_one,
            "get_command_status",
            (session_id.clone(), command_id.clone()),
        )
        .expect("scenario maintenance command status should decode")
        .expect("scenario maintenance command status should load");
        assert_eq!(status.status, CommandStatus::Applied);
        let result_json = status
            .result_json
            .as_deref()
            .expect("scenario maintenance command should persist result_json");
        assert!(
            result_json.contains(&format!(r#""command_kind":"{job_kind}""#))
                && result_json.contains(&format!(r#""touched":{touched}"#)),
            "{job_key} should report the expected maintenance result, got {result_json}"
        );
    }

    let public_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id, "public".to_string(), 0_u64, 200_u32),
    )
    .expect("scenario maintenance public events should decode")
    .expect("scenario maintenance public events should load");
    assert!(
        public_events.events.iter().all(|event| !matches!(
            event.event_type.as_str(),
            "objectives_synced" | "world_event_synced" | "advanced_victory_synced"
        )),
        "scenario maintenance jobs must not rely on manual sync wrapper events"
    );
}

#[test]
fn pocket_ic_end_turn_closes_turn_and_blocks_stale_actions() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-end-turn-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-end-turn-player-two");
    let session_id = start_active_two_player_session(&fixture, player_one, player_two, "end-turn");
    let champion_id = owned_champion_id(&fixture, player_one, &session_id);

    let player_one_ended = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "end_turn",
        (session_id.clone(), "nonce:end-turn:end:one".to_string()),
    )
    .expect("player one end_turn should decode")
    .expect("player one end_turn should succeed");
    assert_eq!(player_one_ended.status, CommandStatus::Applied);
    assert!(player_one_ended.events.iter().any(|event| {
        event.event_type == "participant_turn_ready"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""all_ready":false"#))
    }));

    let ended_player_move = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id.clone(),
            vec![MoveCoord::new(9, 24)],
            "nonce:end-turn:move-after-ended".to_string(),
        ),
    )
    .expect("ended-player move should decode")
    .expect("ended player should still be able to act until the turn closes");
    assert_eq!(ended_player_move.status, CommandStatus::Applied);
    assert_eq!(ended_player_move.effective_turn, 1);

    let duplicate_end_turn = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "end_turn",
        (
            session_id.clone(),
            "nonce:end-turn:end:one:fresh".to_string(),
        ),
    )
    .expect("fresh duplicate end_turn denial should decode")
    .expect_err("fresh duplicate end_turn should still be rejected");
    assert_eq!(duplicate_end_turn.code, "turn_already_ended");

    let player_one_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "end_turn",
        (session_id.clone(), "nonce:end-turn:end:one".to_string()),
    )
    .expect("player one end_turn replay should decode")
    .expect("player one end_turn replay should succeed");
    assert_eq!(player_one_replay.command_id, player_one_ended.command_id);
    assert_eq!(player_one_replay.status, CommandStatus::Applied);

    let player_two_ended = update_as::<CommandResponse>(
        &fixture,
        player_two,
        "end_turn",
        (session_id.clone(), "nonce:end-turn:end:two".to_string()),
    )
    .expect("player two end_turn should decode")
    .expect("player two end_turn should succeed");
    assert_eq!(player_two_ended.status, CommandStatus::Applied);
    assert!(player_two_ended.events.iter().any(|event| {
        event.event_type == "participant_turn_ready"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""all_ready":true"#))
    }));

    assert!(
        player_two_ended
            .changed_subjects
            .iter()
            .any(|subject| subject.subject_kind == "system_job"),
        "final participant readiness should schedule immediate turn resolution"
    );

    let mut after_turn_resolution = compact_game_view(&fixture, player_one, &session_id);
    if after_turn_resolution.session.current_turn == 1 {
        let closing_end_turn = update_as::<CommandResponse>(
            &fixture,
            player_one,
            "end_turn",
            (
                session_id.clone(),
                "nonce:end-turn:end:one:after-close".to_string(),
            ),
        )
        .expect("closing-turn duplicate end_turn should decode");
        match closing_end_turn {
            Ok(response) => {
                assert_eq!(
                    response.effective_turn, 2,
                    "fresh end_turn should only apply if the timer advanced before the call"
                );
            }
            Err(error) => assert_eq!(error.code, "backend_work_pending"),
        }
    }

    if after_turn_resolution.session.current_turn == 1 {
        advance_time_for_timers(&fixture, 1_000);
        replay_player_registration(&fixture, player_two, "end-turn", "two");
        after_turn_resolution = compact_game_view(&fixture, player_one, &session_id);
    }
    assert_eq!(after_turn_resolution.session.current_turn, 2);

    let player_one_stale_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "end_turn",
        (session_id.clone(), "nonce:end-turn:end:one".to_string()),
    )
    .expect("player one stale end_turn replay should decode")
    .expect("player one stale end_turn replay should succeed");
    assert_eq!(
        player_one_stale_replay.command_id,
        player_one_ended.command_id
    );

    let turn_two_move = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id,
            vec![MoveCoord::new(10, 24)],
            "nonce:end-turn:turn-two-move".to_string(),
        ),
    )
    .expect("turn two move should decode")
    .expect("old turn ready rows should not block turn two commands");
    assert_eq!(turn_two_move.status, CommandStatus::Applied);
    assert_eq!(turn_two_move.effective_turn, 2);

    let player_two_replay = update_as::<CommandResponse>(
        &fixture,
        player_two,
        "end_turn",
        (session_id.clone(), "nonce:end-turn:end:two".to_string()),
    )
    .expect("player two end_turn replay should decode")
    .expect("player two end_turn replay should succeed");
    assert_eq!(player_two_replay.command_id, player_two_ended.command_id);
    let after_replay = compact_game_view(&fixture, player_one, &session_id);
    assert_eq!(after_replay.session.current_turn, 2);
}

#[test]
fn pocket_ic_battle_round_readiness_advances_and_replays() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-battle-ready-player-one");
    let player_two = candid::Principal::self_authenticating(b"domm-battle-ready-player-two");
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "battle-ready");
    let champion_id = owned_champion_id(&fixture, player_one, &session_id);

    let (neutral_sync, _) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
            MoveCoord::new(12, 24),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:battle-ready:move:neutral",
        "nonce:battle-ready:sync:neutral:",
        61_000,
        "neutral_encounter_pending",
    );
    let battle_id = battle_id_from_events(&neutral_sync, "neutral_encounter_pending");
    let opening_view = query_as::<BattleView>(
        &fixture,
        player_one,
        "get_battle_state",
        (session_id.clone(), battle_id.clone()),
    )
    .expect("opening battle state should decode")
    .expect("opening battle state should load");
    assert_eq!(opening_view.current_round, 1);

    let mut first_input = None;
    let mut saw_auto_ready = false;
    for attempt in 0..8 {
        let view = query_as::<BattleView>(
            &fixture,
            player_one,
            "get_battle_state",
            (session_id.clone(), battle_id.clone()),
        )
        .expect("battle readiness loop view should decode")
        .expect("battle readiness loop view should load");
        if view.state != "active" {
            break;
        }
        if !view
            .legal_actions_for_caller
            .iter()
            .any(|action| action.enabled)
        {
            advance_time_for_timers(&fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
            replay_player_registration(&fixture, player_one, "battle-ready", "one");
            continue;
        }

        let input = choose_battle_action(&view);
        let nonce = format!("nonce:battle-ready:action:{attempt}");
        let action = update_as::<CommandResponse>(
            &fixture,
            player_one,
            "submit_battle_action",
            (session_id.clone(), input.clone(), nonce.clone()),
        )
        .expect("battle action should decode")
        .expect("battle action should succeed");
        assert_eq!(action.status, CommandStatus::Applied);
        if first_input.is_none() {
            first_input = Some(input.clone());
            let replay = update_as::<CommandResponse>(
                &fixture,
                player_one,
                "submit_battle_action",
                (session_id.clone(), input.clone(), nonce),
            )
            .expect("battle action replay should decode")
            .expect("battle action replay should succeed");
            assert_eq!(replay.command_id, action.command_id);
        }
        saw_auto_ready |= action
            .changed_subjects
            .iter()
            .any(|subject| subject.subject_kind == "battle_participant_round_ready");
        if action
            .changed_subjects
            .iter()
            .any(|subject| subject.subject_kind == "system_job")
        {
            saw_auto_ready = true;
            break;
        }
    }
    assert!(
        saw_auto_ready,
        "spending all meaningful owned stack actions should auto-ready the battle participant"
    );
    let first_input = first_input.expect("battle readiness route should submit an action");

    advance_time_for_timers(&fixture, 1_000);
    replay_player_registration(&fixture, player_one, "battle-ready", "one");
    let after_auto_defend = query_as::<BattleView>(
        &fixture,
        player_one,
        "get_battle_state",
        (session_id.clone(), battle_id.clone()),
    )
    .expect("post-auto-defend battle state should decode")
    .expect("post-auto-defend battle state should load");
    assert!(
        after_auto_defend.current_round > opening_view.current_round
            || after_auto_defend.state != "active",
        "battle round timer should advance the round or resolve the battle"
    );

    let public_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 100_u32),
    )
    .expect("battle-ready public events should decode")
    .expect("battle-ready public events should load");
    assert!(
        public_events
            .events
            .iter()
            .any(|event| event.event_type == "battle_round_auto_defend"),
        "round advancement should auto-defend remaining stacks"
    );

    if after_auto_defend.state == "active" {
        let ended_round = update_as::<CommandResponse>(
            &fixture,
            player_one,
            "end_battle_turn",
            (
                session_id.clone(),
                battle_id.clone(),
                "nonce:battle-ready:end-round".to_string(),
            ),
        )
        .expect("end_battle_turn should decode")
        .expect("end_battle_turn should succeed");
        assert_eq!(ended_round.status, CommandStatus::Applied);
        assert!(
            ended_round
                .events
                .iter()
                .any(|event| event.event_type == "battle_participant_round_ready")
        );

        let ended_round_replay = update_as::<CommandResponse>(
            &fixture,
            player_one,
            "end_battle_turn",
            (
                session_id.clone(),
                battle_id.clone(),
                "nonce:battle-ready:end-round".to_string(),
            ),
        )
        .expect("end_battle_turn replay should decode")
        .expect("end_battle_turn replay should succeed");
        assert_eq!(ended_round_replay.command_id, ended_round.command_id);

        let blocked_action = update_as::<CommandResponse>(
            &fixture,
            player_one,
            "submit_battle_action",
            (
                session_id.clone(),
                first_input.clone(),
                "nonce:battle-ready:action:after-ended".to_string(),
            ),
        )
        .expect("post-end battle action denial should decode")
        .expect_err("ended battle round should block new battle actions");
        assert_eq!(blocked_action.code, "battle_round_closed");
    }
}

#[test]
fn pocket_ic_render_projection_tracks_live_objects_and_fog() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-render-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-render-two");
    let viewport = opening_viewport_for_slot(0);
    let remote_viewport = opening_viewport_for_slot(1);
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "render-projection");

    let first_page = visible_objects_page(&fixture, player_one, &session_id, &viewport, None, 3);
    assert_eq!(first_page.objects.len(), 3);
    assert!(first_page.has_more);
    assert_eq!(first_page.next_cursor, Some(3));
    let second_page = visible_objects_page(
        &fixture,
        player_one,
        &session_id,
        &viewport,
        first_page.next_cursor,
        3,
    );
    assert!(!second_page.objects.is_empty());
    for second in &second_page.objects {
        assert!(
            first_page.objects.iter().all(|first| {
                first.subject_kind != second.subject_kind
                    || first.subject_id_text != second.subject_id_text
            }),
            "cursor page should not duplicate {}:{}",
            second.subject_kind,
            second.subject_id_text
        );
    }

    let opening_objects =
        visible_objects_page(&fixture, player_one, &session_id, &viewport, None, 128);
    let opening_champion = visible_object(&opening_objects, "champion:west");
    assert_eq!(
        opening_champion.display_name.as_deref(),
        Some("Mara of the Toll")
    );
    assert_eq!((opening_champion.x, opening_champion.y), (8, 24));
    assert!(
        opening_objects
            .objects
            .iter()
            .any(|object| object.subject_id_text == "pile:west-wood-1")
    );
    assert!(
        opening_objects
            .objects
            .iter()
            .any(|object| object.subject_id_text == "neutral:west-mine")
    );

    let remote_objects = visible_objects_page(
        &fixture,
        player_one,
        &session_id,
        &remote_viewport,
        None,
        128,
    );
    let remote_subjects = remote_objects
        .objects
        .iter()
        .map(|object| format!("{}:{}", object.subject_kind, object.subject_id_text))
        .collect::<Vec<_>>();
    assert!(
        remote_objects.objects.is_empty(),
        "remote fog should not leak dynamic objects: {remote_subjects:?}"
    );
}

#[test]
fn pocket_ic_render_projection_tracks_battle_aftermath_objects() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-render-battle-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-render-battle-two");
    let viewport = Viewport::new(8, 20, 7, 5);
    let session_id = start_active_two_player_session(
        &fixture,
        player_one,
        player_two,
        "render-projection-battle",
    );
    let west_participant = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("battle participant query should decode")
    .expect("battle participant should load");
    let west_champion_id = owned_champion_id(&fixture, player_one, &session_id);

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
        "nonce:render-battle:move:neutral",
        "nonce:render-battle:sync:neutral:",
        122_000_u64,
        "neutral_encounter_pending",
    );
    let neutral_battle_id = battle_id_from_events(&neutral_sync, "neutral_encounter_pending");
    resolve_battle_to_end(
        &fixture,
        player_one,
        &session_id,
        &neutral_battle_id,
        "nonce:render-battle:neutral",
    );

    let public_events = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 200_u32),
    )
    .expect("public events should decode")
    .expect("public events should load");
    for expected in [
        "mine_captured",
        "neutral_defeated",
        "battle_aftermath_applied",
    ] {
        assert!(
            public_events
                .events
                .iter()
                .any(|event| event.event_type == expected),
            "render route should publish {expected}"
        );
    }

    let after_battle = visible_objects_page(&fixture, player_one, &session_id, &viewport, None, 32);
    let champion_after_battle = visible_object(&after_battle, "champion:west");
    assert_eq!((champion_after_battle.x, champion_after_battle.y), (12, 22));
    assert!(
        after_battle
            .objects
            .iter()
            .all(|object| object.subject_id_text != "neutral:west-mine"),
        "defeated neutral guards must not render as active objects"
    );
    let guarded_mine = visible_object(&after_battle, "mine:west-gold");
    assert_eq!(
        guarded_mine.owner_participant_id.as_deref(),
        Some(west_participant.participant_id.as_str())
    );
    assert!(
        guarded_mine.details_json.contains(r#""state":"captured""#),
        "captured mine should render captured live details: {}",
        guarded_mine.details_json
    );
}

#[test]
fn pocket_ic_query_budget_keeps_preview_submit_and_render_bounded() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-pocket-query-budget-one");
    let player_two = candid::Principal::self_authenticating(b"domm-pocket-query-budget-two");
    let viewport = opening_viewport_for_slot(0);
    let mut metrics = GateJMetrics::default();
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "query-budget");
    let champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let pickup_path = vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)];

    let preview = gate_query_as::<MovementPreview>(
        &mut metrics,
        &fixture,
        player_one,
        "preview_move_path",
        (
            session_id.clone(),
            champion_id.clone(),
            pickup_path.clone(),
            1_000_u64,
        ),
    )
    .expect("movement preview should stay under query budget");
    assert_eq!(preview.path, pickup_path);
    assert!(preview.total_cost > 0);
    assert!(
        preview
            .stop
            .as_ref()
            .is_some_and(|stop| stop.subject_id_text == "pile:west-wood-1")
    );

    let submitted = gate_update_as::<CommandResponse>(
        &mut metrics,
        &fixture,
        player_one,
        "submit_move_intent",
        (
            session_id.clone(),
            champion_id,
            pickup_path,
            "nonce:query-budget:move:wood".to_string(),
        ),
    )
    .expect("movement submit should stay under update budget");
    assert_eq!(submitted.status, CommandStatus::Applied);

    let first_objects = gate_query_as::<ObjectViewPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_visible_objects",
        (session_id.clone(), viewport.clone(), None::<u32>, 3_u32),
    )
    .expect("small object page should stay bounded");
    assert_eq!(first_objects.objects.len(), 3);
    assert!(first_objects.has_more);
    let second_objects = gate_query_as::<ObjectViewPage>(
        &mut metrics,
        &fixture,
        player_one,
        "get_visible_objects",
        (
            session_id.clone(),
            viewport.clone(),
            first_objects.next_cursor,
            3_u32,
        ),
    )
    .expect("cursor object page should stay bounded");
    assert!(!second_objects.objects.is_empty());

    let compact_view = gate_query_as::<GameView>(
        &mut metrics,
        &fixture,
        player_one,
        "get_game_view",
        (
            session_id,
            GameViewRequest {
                viewport,
                chunk_cursor: None,
                chunk_limit: 2,
                object_cursor: None,
                object_limit: 3,
                events_after_seq: 0,
                event_limit: 4,
                include_battle: false,
            },
        ),
    )
    .expect("compact game view should stay bounded");
    assert!(compact_view.objects.len() <= 3);
    assert!(compact_view.map_chunks.len() <= 2);
    assert_eq!(compact_view.object_page_info.limit, 3);
    assert_eq!(compact_view.map_page_info.limit, 2);
    assert!(
        metrics.max_response_bytes <= 64 * 1024,
        "query budget route response too large: {} bytes from {}",
        metrics.max_response_bytes,
        metrics.max_response_method
    );
    assert!(metrics.query_calls >= 4);
    assert_eq!(metrics.update_calls, 1);
}

#[test]
fn pocket_ic_command_recovery_replays_economy_and_battle_effects() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-command-recovery-one");
    let player_two = candid::Principal::self_authenticating(b"domm-command-recovery-two");
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "command-recovery");
    let town_id = "town:west".to_string();
    let champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let initial_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    assert_eq!(row_count(&initial_storage, "ChampionHire"), 0);
    assert_eq!(row_count(&initial_storage, "DwellingRecruitment"), 0);
    assert!(row_count(&initial_storage, "TavernOffer") > 0);
    assert!(row_count(&initial_storage, "DwellingPool") > 0);

    let built = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
            "nonce:command-recovery:build".to_string(),
        ),
    )
    .expect("build recovery command should decode")
    .expect("build recovery command should succeed");
    assert_eq!(built.status, CommandStatus::Applied);
    let build_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    assert_eq!(
        row_count(&build_storage, "TownBuilding"),
        row_count(&initial_storage, "TownBuilding") + 1
    );
    let participant_after_build = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after build should decode")
    .expect("participant after build should load");
    let build_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
            "nonce:command-recovery:build".to_string(),
        ),
    )
    .expect("build replay should decode")
    .expect("build replay should succeed");
    assert_eq!(build_replay.command_id, built.command_id);
    assert!(build_replay.events.is_empty());
    let participant_after_build_replay = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after build replay should decode")
    .expect("participant after build replay should load");
    assert_eq!(
        participant_after_build_replay.resources,
        participant_after_build.resources
    );
    let build_replay_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    assert_row_count_stable(&build_storage, &build_replay_storage, "CommandEffect");
    assert_row_count_stable(&build_storage, &build_replay_storage, "GameEvent");
    assert_row_count_stable(&build_storage, &build_replay_storage, "ResourceLedgerEntry");
    assert_row_count_stable(&build_storage, &build_replay_storage, "TownBuilding");

    let town_after_build = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after build should decode")
    .expect("town after build should load");
    let recruit_pool_before = town_after_build
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == "mudhook-levy")
        .expect("training yard should expose a mudhook recruit pool")
        .available;
    assert!(recruit_pool_before > 0);

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
            "nonce:command-recovery:recruit".to_string(),
        ),
    )
    .expect("recruit recovery command should decode")
    .expect("recruit recovery command should succeed");
    assert_eq!(recruited.status, CommandStatus::Applied);
    let recruit_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    assert_eq!(
        row_count(&recruit_storage, "TownGarrisonStack"),
        row_count(&build_replay_storage, "TownGarrisonStack") + 1
    );
    let town_after_recruit = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after recruit should decode")
    .expect("town after recruit should load");
    let recruit_pool_after = town_after_recruit
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == "mudhook-levy")
        .expect("recruit pool should remain visible after recruit")
        .available;
    assert_eq!(recruit_pool_after, recruit_pool_before - 1);
    assert!(
        town_after_recruit
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == "mudhook-levy" && stack.quantity == 1)
    );
    let participant_after_recruit = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after recruit should decode")
    .expect("participant after recruit should load");
    let recruit_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_recruit_units",
        (
            session_id.clone(),
            town_id.clone(),
            "unit:mudhook-levy".to_string(),
            1_u32,
            RecruitTarget::TownGarrison { slot_index: None },
            "nonce:command-recovery:recruit".to_string(),
        ),
    )
    .expect("recruit replay should decode")
    .expect("recruit replay should succeed");
    assert_eq!(recruit_replay.command_id, recruited.command_id);
    assert!(recruit_replay.events.is_empty());
    let participant_after_recruit_replay = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after recruit replay should decode")
    .expect("participant after recruit replay should load");
    assert_eq!(
        participant_after_recruit_replay.resources,
        participant_after_recruit.resources
    );
    let town_after_recruit_replay = query_as::<ApiTownView>(
        &fixture,
        player_one,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("town after recruit replay should decode")
    .expect("town after recruit replay should load");
    assert_eq!(
        town_after_recruit_replay
            .recruit_pools
            .iter()
            .find(|pool| pool.unit_slug == "mudhook-levy")
            .expect("recruit pool should exist after replay")
            .available,
        recruit_pool_after
    );
    assert!(
        town_after_recruit_replay
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == "mudhook-levy" && stack.quantity == 1)
    );
    let recruit_replay_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    for entity in [
        "CommandEffect",
        "GameEvent",
        "ResourceLedgerEntry",
        "TownGarrisonStack",
        "TownRecruitPool",
    ] {
        assert_row_count_stable(&recruit_storage, &recruit_replay_storage, entity);
    }

    let tavern_offers = query_as::<TavernOffersView>(
        &fixture,
        player_one,
        "get_tavern_offers",
        (session_id.clone(), town_id.clone()),
    )
    .expect("tavern offers should decode")
    .expect("tavern offers should load");
    let tavern_offer = tavern_offers
        .offers
        .iter()
        .find(|offer| offer.status == "available")
        .expect("an available tavern offer should exist")
        .clone();
    let hired = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "hire_tavern_champion",
        (
            session_id.clone(),
            town_id.clone(),
            tavern_offer.offer_key.clone(),
            "nonce:command-recovery:hire".to_string(),
        ),
    )
    .expect("hire recovery command should decode")
    .expect("hire recovery command should succeed");
    assert_eq!(hired.status, CommandStatus::Applied);
    let hired_champion_id = match &hired.result {
        CommandResult::ExpandedEconomy(receipt) => receipt
            .champion_id
            .clone()
            .expect("hire receipt should include a champion id"),
        other => panic!("hire recovery returned unexpected result: {other:?}"),
    };
    let hire_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    assert_eq!(
        row_count(&hire_storage, "Champion"),
        row_count(&recruit_replay_storage, "Champion") + 1
    );
    assert_eq!(
        row_count(&hire_storage, "ChampionHire"),
        row_count(&recruit_replay_storage, "ChampionHire") + 1
    );
    let participant_after_hire = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after hire should decode")
    .expect("participant after hire should load");
    let hire_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "hire_tavern_champion",
        (
            session_id.clone(),
            town_id.clone(),
            tavern_offer.offer_key.clone(),
            "nonce:command-recovery:hire".to_string(),
        ),
    )
    .expect("hire replay should decode")
    .expect("hire replay should succeed");
    assert_eq!(hire_replay.command_id, hired.command_id);
    assert!(hire_replay.events.is_empty());
    match &hire_replay.result {
        CommandResult::ExpandedEconomy(receipt) => {
            assert_eq!(
                receipt.champion_id.as_deref(),
                Some(hired_champion_id.as_str())
            );
        }
        other => panic!("hire replay returned unexpected result: {other:?}"),
    }
    let participant_after_hire_replay = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after hire replay should decode")
    .expect("participant after hire replay should load");
    assert_eq!(
        participant_after_hire_replay.resources,
        participant_after_hire.resources
    );
    let tavern_after_hire_replay = query_as::<TavernOffersView>(
        &fixture,
        player_one,
        "get_tavern_offers",
        (session_id.clone(), town_id.clone()),
    )
    .expect("tavern offers after hire replay should decode")
    .expect("tavern offers after hire replay should load");
    assert!(tavern_after_hire_replay.offers.iter().any(|offer| {
        offer.offer_key == tavern_offer.offer_key
            && offer.status == "hired"
            && offer.hired_champion_id.as_deref() == Some(hired_champion_id.as_str())
    }));
    let hire_replay_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    for entity in [
        "Champion",
        "ChampionHire",
        "CommandEffect",
        "GameEvent",
        "ResourceLedgerEntry",
        "TavernOffer",
    ] {
        assert_row_count_stable(&hire_storage, &hire_replay_storage, entity);
    }

    let dwelling_id = "dwelling:west-mudhook".to_string();
    let dwelling_pool = query_as::<DwellingPoolView>(
        &fixture,
        player_one,
        "get_dwelling_pool",
        (session_id.clone(), dwelling_id.clone()),
    )
    .expect("dwelling pool should decode")
    .expect("dwelling pool should load");
    assert!(dwelling_pool.available > 0);
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
            "nonce:command-recovery:dwelling".to_string(),
        ),
    )
    .expect("dwelling recovery command should decode")
    .expect("dwelling recovery command should succeed");
    assert_eq!(dwelling_recruit.status, CommandStatus::Applied);
    let dwelling_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    assert_eq!(
        row_count(&dwelling_storage, "DwellingRecruitment"),
        row_count(&hire_replay_storage, "DwellingRecruitment") + 1
    );
    let dwelling_after_recruit = query_as::<DwellingPoolView>(
        &fixture,
        player_one,
        "get_dwelling_pool",
        (session_id.clone(), dwelling_id.clone()),
    )
    .expect("dwelling pool after recruit should decode")
    .expect("dwelling pool after recruit should load");
    assert_eq!(
        dwelling_after_recruit.available,
        dwelling_pool.available.saturating_sub(1)
    );
    let participant_after_dwelling = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after dwelling should decode")
    .expect("participant after dwelling should load");
    let dwelling_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_dwelling_recruit",
        (
            session_id.clone(),
            dwelling_id.clone(),
            "mudhook-levy".to_string(),
            1_u32,
            champion_id.clone(),
            "nonce:command-recovery:dwelling".to_string(),
        ),
    )
    .expect("dwelling replay should decode")
    .expect("dwelling replay should succeed");
    assert_eq!(dwelling_replay.command_id, dwelling_recruit.command_id);
    assert!(dwelling_replay.events.is_empty());
    let dwelling_after_replay = query_as::<DwellingPoolView>(
        &fixture,
        player_one,
        "get_dwelling_pool",
        (session_id.clone(), dwelling_id.clone()),
    )
    .expect("dwelling pool after replay should decode")
    .expect("dwelling pool after replay should load");
    assert_eq!(
        dwelling_after_replay.available,
        dwelling_after_recruit.available
    );
    let participant_after_dwelling_replay = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("participant after dwelling replay should decode")
    .expect("participant after dwelling replay should load");
    assert_eq!(
        participant_after_dwelling_replay.resources,
        participant_after_dwelling.resources
    );
    let dwelling_replay_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    for entity in [
        "ChampionArmyStack",
        "CommandEffect",
        "DwellingPool",
        "DwellingRecruitment",
        "GameEvent",
        "ResourceLedgerEntry",
    ] {
        assert_row_count_stable(&dwelling_storage, &dwelling_replay_storage, entity);
    }

    let (neutral_sync, _) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
            MoveCoord::new(12, 24),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:command-recovery:move:neutral",
        "nonce:command-recovery:sync-turn:neutral:",
        122_000_u64,
        "neutral_encounter_pending",
    );
    let battle_id = battle_id_from_events(&neutral_sync, "neutral_encounter_pending");
    resolve_battle_to_end(
        &fixture,
        player_one,
        &session_id,
        &battle_id,
        "nonce:command-recovery:battle",
    );
    let aftermath_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    let events_after_aftermath = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 200_u32),
    )
    .expect("events after battle aftermath should decode")
    .expect("events after battle aftermath should load");
    assert_eq!(
        event_count_for_subject(
            &events_after_aftermath,
            "battle_aftermath_applied",
            &battle_id,
        ),
        1
    );
    let aftermath_nonce = "nonce:command-recovery:battle:aftermath".to_string();
    let aftermath_status = query_as::<CommandStatusView>(
        &fixture,
        player_one,
        "get_command_status_by_nonce",
        (
            session_id.clone(),
            "sync_battle".to_string(),
            aftermath_nonce.clone(),
        ),
    )
    .expect("aftermath command status should decode")
    .expect("aftermath command status should load");
    assert_eq!(aftermath_status.status, CommandStatus::Applied);
    let aftermath_replay = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_battle",
        (session_id.clone(), battle_id.clone(), aftermath_nonce),
    )
    .expect("aftermath exact nonce replay should decode")
    .expect("aftermath exact nonce replay should succeed");
    assert_eq!(aftermath_replay.command_id, aftermath_status.command_id);
    assert!(aftermath_replay.events.is_empty());
    let aftermath_retry = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "sync_battle",
        (
            session_id.clone(),
            battle_id.clone(),
            "nonce:command-recovery:battle:aftermath-retry".to_string(),
        ),
    )
    .expect("aftermath fresh retry should decode")
    .expect("aftermath fresh retry should succeed");
    assert_eq!(aftermath_retry.status, CommandStatus::Applied);
    assert!(
        aftermath_retry
            .events
            .iter()
            .all(|event| event.event_type != "battle_aftermath_applied")
    );
    let retry_storage = diagnostic_snapshot(&fixture, COMMAND_RECOVERY_ENTITIES);
    for entity in [
        "Battle",
        "CommandEffect",
        "GameEvent",
        "NeutralArmy",
        "WorldObject",
    ] {
        assert_row_count_stable(&aftermath_storage, &retry_storage, entity);
    }
    let events_after_retry = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (session_id, "public".to_string(), 0_u64, 200_u32),
    )
    .expect("events after battle retry should decode")
    .expect("events after battle retry should load");
    assert_eq!(
        event_count_for_subject(&events_after_retry, "battle_aftermath_applied", &battle_id),
        1
    );
}

#[test]
fn pocket_ic_visibility_redaction_keeps_private_payloads_private() {
    let fixture = install_degens_canister_fixture();
    let player_one = candid::Principal::self_authenticating(b"domm-visibility-one");
    let player_two = candid::Principal::self_authenticating(b"domm-visibility-two");
    let session_id =
        start_active_two_player_session(&fixture, player_one, player_two, "visibility-redaction");
    let town_id = "town:west".to_string();
    let participant_one = query_as::<ParticipantView>(
        &fixture,
        player_one,
        "get_my_participant",
        (session_id.clone(),),
    )
    .expect("visibility participant query should decode")
    .expect("visibility participant should load");

    let hidden_town = query_as::<ApiTownView>(
        &fixture,
        player_two,
        "get_town_view",
        (session_id.clone(), town_id.clone()),
    )
    .expect("opponent hidden town query should decode")
    .expect_err("opponent should not read a town outside visibility");
    assert_eq!(hidden_town.code, "not_visible");

    let built = update_as::<CommandResponse>(
        &fixture,
        player_one,
        "submit_build_town_structure",
        (
            session_id.clone(),
            town_id.clone(),
            "building:freehold-training-yard".to_string(),
            "nonce:visibility:build".to_string(),
        ),
    )
    .expect("visibility build should decode")
    .expect("visibility build should succeed");
    assert_eq!(built.status, CommandStatus::Applied);
    assert!(built.events.iter().any(|event| {
        event.audience_key == format!("participant:{}", participant_one.participant_id)
            && event.event_type == "town_building_built"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("freehold-training-yard"))
    }));
    assert!(built.events.iter().any(|event| {
        event.audience_key == "public"
            && event.event_type == "town_building_built"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""redacted":true"#))
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| !payload.contains("freehold-training-yard"))
    }));

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
            "nonce:visibility:recruit".to_string(),
        ),
    )
    .expect("visibility recruit should decode")
    .expect("visibility recruit should succeed");
    assert_eq!(recruited.status, CommandStatus::Applied);
    assert!(recruited.events.iter().any(|event| {
        event.audience_key == format!("participant:{}", participant_one.participant_id)
            && event.event_type == "units_recruited"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("mudhook-levy"))
    }));
    assert!(recruited.events.iter().any(|event| {
        event.audience_key == "public"
            && event.event_type == "units_recruited"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains(r#""redacted":true"#))
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| !payload.contains("mudhook-levy"))
    }));

    let private_page = query_as::<ApiEventPage>(
        &fixture,
        player_one,
        "get_events_after",
        (
            session_id.clone(),
            format!("participant:{}", participant_one.participant_id),
            0_u64,
            50_u32,
        ),
    )
    .expect("private event page should decode")
    .expect("private event page should load");
    assert!(private_page.events.iter().any(|event| {
        event.event_type == "town_building_built"
            && event.audience_key.starts_with("participant:")
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("freehold-training-yard"))
    }));
    assert!(private_page.events.iter().any(|event| {
        event.event_type == "units_recruited"
            && event.audience_key.starts_with("participant:")
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("mudhook-levy"))
    }));

    let forbidden_private_page = query_as::<ApiEventPage>(
        &fixture,
        player_two,
        "get_events_after",
        (
            session_id.clone(),
            format!("participant:{}", participant_one.participant_id),
            0_u64,
            50_u32,
        ),
    )
    .expect("forbidden private event query should decode")
    .expect_err("opponent must not read participant-private events");
    assert_eq!(forbidden_private_page.code, "audience_not_allowed");

    let public_page_for_opponent = query_as::<ApiEventPage>(
        &fixture,
        player_two,
        "get_events_after",
        (session_id.clone(), "public".to_string(), 0_u64, 50_u32),
    )
    .expect("opponent public event page should decode")
    .expect("opponent public event page should load");
    for event_type in ["town_building_built", "units_recruited"] {
        let event = public_page_for_opponent
            .events
            .iter()
            .find(|event| event.event_type == event_type)
            .unwrap_or_else(|| panic!("public feed should include redacted {event_type}"));
        assert_eq!(event.audience_key, "public");
        let payload = event
            .payload
            .as_deref()
            .expect("public redacted event should carry a payload");
        assert!(payload.contains(r#""redacted":true"#));
        assert!(!payload.contains("freehold-training-yard"));
        assert!(!payload.contains("mudhook-levy"));
    }

    let still_hidden_town = query_as::<ApiTownView>(
        &fixture,
        player_two,
        "get_town_view",
        (session_id.clone(), town_id),
    )
    .expect("opponent hidden town after build/recruit should decode")
    .expect_err("opponent should still not read hidden town internals");
    assert_eq!(still_hidden_town.code, "not_visible");

    let champion_id = owned_champion_id(&fixture, player_one, &session_id);
    let (neutral_sync, _) = submit_move_and_sync_until_event(
        &fixture,
        player_one,
        &session_id,
        &champion_id,
        vec![
            MoveCoord::new(9, 24),
            MoveCoord::new(10, 24),
            MoveCoord::new(11, 24),
            MoveCoord::new(12, 24),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:visibility:move:neutral",
        "nonce:visibility:sync-turn:neutral:",
        122_000_u64,
        "neutral_encounter_pending",
    );
    let battle_id = battle_id_from_events(&neutral_sync, "neutral_encounter_pending");
    let own_battle = query_as::<BattleView>(
        &fixture,
        player_one,
        "get_battle_state",
        (session_id.clone(), battle_id.clone()),
    )
    .expect("own neutral battle view should decode")
    .expect("own neutral battle view should load");
    assert_eq!(own_battle.battle_type, "neutral");
    assert!(!own_battle.stacks.is_empty());

    let hidden_battle = query_as::<BattleView>(
        &fixture,
        player_two,
        "get_battle_state",
        (session_id.clone(), battle_id.clone()),
    )
    .expect("opponent neutral battle query should decode")
    .expect_err("uninvolved opponent should not read neutral battle state");
    assert_eq!(hidden_battle.code, "battle_not_visible");

    let public_after_battle = query_as::<ApiEventPage>(
        &fixture,
        player_two,
        "get_events_after",
        (session_id, "public".to_string(), 0_u64, 100_u32),
    )
    .expect("opponent public battle events should decode")
    .expect("opponent public battle events should load");
    let neutral_event = public_after_battle
        .events
        .iter()
        .find(|event| event.event_type == "neutral_encounter_pending")
        .expect("public feed should include the neutral encounter marker");
    let neutral_payload = neutral_event
        .payload
        .as_deref()
        .expect("neutral encounter marker should include a bounded payload");
    assert!(neutral_payload.contains(r#""battle_id":"#));
    assert!(!neutral_payload.contains("quantity"));
    assert!(!neutral_payload.contains("front_hp"));
    assert!(!neutral_payload.contains("damage_min"));
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

    let _income_sync = gate_sync_turn_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        "nonce:gate-j:sync:income:",
        "income_materialized",
        4,
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
    advance_time_without_timers(&fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
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

    let _income_sync = gate_sync_turn_until_event(
        &mut metrics,
        &fixture,
        player_one,
        &session_id,
        "nonce:gate-l:sync:income:",
        "income_materialized",
        4,
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
    gate_resolve_battle_to_end_for_callers(
        &mut metrics,
        &fixture,
        &[player_one, player_two],
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
    advance_time_without_timers(&fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
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
        61_000,
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

    let mut saw_setup_partial_sync = false;
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
        saw_setup_partial_sync |= saw_partial_sync;
    }
    assert!(
        saw_setup_partial_sync,
        "setup movement should observe at least one partial sync slice"
    );

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
        6,
        61_000,
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
    let session_id = create_ready_two_player_session(fixture, player_one, player_two, nonce_stem);
    start_session_once_and_wait_active(
        fixture,
        player_one,
        &session_id,
        &format!("nonce:{nonce_stem}:start"),
    );
    session_id
}

fn create_ready_two_player_session(
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

    session_id
}

fn start_session_once_and_wait_active(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    client_nonce: &str,
) -> LobbyCommandResponse {
    let started = update_as::<LobbyCommandResponse>(
        fixture,
        player,
        "start_session",
        (session_id.to_string(), client_nonce.to_string()),
    )
    .expect("start_session should decode")
    .expect("start_session should succeed");
    assert_eq!(started.status, CommandStatus::Applied);
    match &started.result {
        LobbyCommandResult::Session(session) => {
            assert!(
                matches!(session.state.as_str(), "starting" | "active"),
                "one-call start_session should return starting or active, got {}",
                session.state
            );
        }
        other => panic!("start_session returned unexpected result: {other:?}"),
    }
    wait_for_session_active(fixture, player, session_id);
    started
}

fn wait_for_session_active(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
) -> SessionView {
    let mut last_state = String::new();
    for _ in 0..80 {
        let session =
            query_as::<SessionView>(fixture, player, "get_session", (session_id.to_string(),))
                .expect("started session should decode")
                .expect("started session should load");
        if session.state == "active" {
            return session;
        }
        last_state = session.state;
        advance_time_for_timers(fixture, 5);
    }
    let jobs = diagnostic_system_jobs(fixture, Some(session_id.to_string()), None);
    panic!(
        "one-call start_session timer continuation should activate session, last state {last_state}, jobs {:?}",
        jobs.jobs
            .iter()
            .map(|job| (
                job.job_key.as_str(),
                job.job_kind.as_str(),
                job.status.as_str(),
                job.attempt_count
            ))
            .collect::<Vec<_>>()
    );
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

const COMMAND_RECOVERY_ENTITIES: &[&str] = &[
    "Battle",
    "Champion",
    "ChampionArmyStack",
    "ChampionHire",
    "CommandEffect",
    "DwellingPool",
    "DwellingRecruitment",
    "GameCommand",
    "GameEvent",
    "NeutralArmy",
    "ResourceLedgerEntry",
    "TavernOffer",
    "TownBuilding",
    "TownGarrisonStack",
    "TownRecruitPool",
    "WorldObject",
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

    gate_start_session_once_and_wait_active(
        metrics,
        fixture,
        player_one,
        &session_id,
        &format!("nonce:{nonce_stem}:start"),
    );
    session_id
}

fn gate_start_session_once_and_wait_active(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    client_nonce: &str,
) -> LobbyCommandResponse {
    let started = gate_update_as::<LobbyCommandResponse>(
        metrics,
        fixture,
        player,
        "start_session",
        (session_id.to_string(), client_nonce.to_string()),
    )
    .expect("start_session should succeed");
    metrics.observe_lobby_response(&started);
    assert_eq!(started.status, CommandStatus::Applied);
    match &started.result {
        LobbyCommandResult::Session(session) => {
            assert!(
                matches!(session.state.as_str(), "starting" | "active"),
                "one-call start_session should return starting or active, got {}",
                session.state
            );
        }
        other => panic!("start_session returned unexpected result: {other:?}"),
    }

    let mut last_state = String::new();
    for _ in 0..80 {
        let session = gate_query_as::<SessionView>(
            metrics,
            fixture,
            player,
            "get_session",
            (session_id.to_string(),),
        )
        .expect("started session should load");
        if session.state == "active" {
            return started;
        }
        last_state = session.state;
        advance_time_for_timers(fixture, 5);
    }
    panic!(
        "one-call start_session timer continuation should activate session, last state {last_state}"
    );
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
    let max_sync_calls = path.len().saturating_add(8).max(12);
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
    let max_sync_calls = max_sync_calls.max(12);
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
        if synced.status != CommandStatus::Applied {
            if synced
                .error
                .as_ref()
                .is_some_and(|error| error.code == "turn_not_due")
            {
                advance_time_without_timers(fixture, 61_000);
                continue;
            }
            panic!("sync_session_turn failed while waiting for {expected_event_type}: {synced:?}");
        }
        let partial_sync = synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete");
        saw_partial_sync |= partial_sync;
        if synced
            .events
            .iter()
            .any(|event| event.event_type == expected_event_type)
        {
            return (synced, saw_partial_sync);
        }
        if !partial_sync {
            advance_time_without_timers(fixture, 61_000);
        }
    }

    panic!("sync_session_turn did not emit {expected_event_type} after {max_sync_calls} calls");
}

fn gate_sync_turn_until_event(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    sync_nonce_prefix: &str,
    expected_event_type: &str,
    max_sync_calls: usize,
) -> CommandResponse {
    let mut observed = Vec::new();
    for attempt in 0..max_sync_calls {
        fixture.pic().advance_time(Duration::from_millis(61_000));
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
        observed.push(sync_summary(attempt, &synced));
        if gate_public_event_seen(metrics, fixture, player, session_id, expected_event_type) {
            return synced;
        }
        if synced.status != CommandStatus::Applied {
            if synced
                .error
                .as_ref()
                .is_some_and(|error| error.code == "turn_not_due")
            {
                continue;
            }
            panic!("sync_session_turn failed while waiting for {expected_event_type}: {synced:?}");
        }
        if synced
            .events
            .iter()
            .any(|event| event.event_type == expected_event_type)
        {
            return synced;
        }
    }

    panic!(
        "sync_session_turn did not emit {expected_event_type} after {max_sync_calls} calls: {}",
        observed.join(" | ")
    );
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
            advance_time_without_timers(fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
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
    gate_resolve_battle_to_end_for_callers(
        metrics,
        fixture,
        &[player],
        session_id,
        battle_id,
        nonce_prefix,
    )
}

fn gate_resolve_battle_to_end_for_callers(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    players: &[candid::Principal],
    session_id: &str,
    battle_id: &str,
    nonce_prefix: &str,
) -> (BattleView, bool) {
    assert!(
        !players.is_empty(),
        "battle resolver needs at least one caller"
    );
    let mut saw_battle_sync = false;
    let mut last_views = String::new();
    for step in 0..160 {
        let mut actionable_view: Option<(candid::Principal, BattleView)> = None;
        let mut summaries = Vec::new();

        for player in players.iter().copied() {
            let view = gate_query_as::<BattleView>(
                metrics,
                fixture,
                player,
                "get_battle_state",
                (session_id.to_string(), battle_id.to_string()),
            )
            .expect("battle view should load");
            summaries.push(format!(
                "{}:state={}:active={}:legal={}",
                player.to_text(),
                view.state,
                view.active_stack_id.as_deref().unwrap_or("-"),
                view.legal_actions_for_caller.len()
            ));
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
                .unwrap_or_else(|error| {
                    panic!("post-battle sync_session_turn should decode: {error}")
                });
                metrics.record_update("sync_session_turn", &turn_synced);
                match turn_synced {
                    Ok(response) => {
                        metrics.observe_command_response(&response);
                        assert_eq!(response.status, CommandStatus::Applied);
                    }
                    Err(error) if error.code == "turn_not_due" => {}
                    Err(error) => {
                        panic!(
                            "post-battle sync_session_turn should succeed or be not due: {error:?}"
                        )
                    }
                }
                return (view, true);
            }
            if actionable_view.is_none() && !view.legal_actions_for_caller.is_empty() {
                actionable_view = Some((player, view));
            }
        }

        last_views = summaries.join(" | ");
        if let Some((player, view)) = actionable_view {
            let input = choose_battle_action_for_goal(&view, player == players[0]);
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
            saw_battle_sync = true;
            continue;
        }

        advance_time_without_timers(fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
        let synced = gate_update_as::<CommandResponse>(
            metrics,
            fixture,
            players[0],
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
        saw_battle_sync = true;
    }

    panic!(
        "battle {battle_id} did not resolve within the test budget; sync={saw_battle_sync}; last views: {last_views}"
    );
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

fn assert_row_count_stable(
    before: &DiagnosticStorageSnapshot,
    after: &DiagnosticStorageSnapshot,
    entity: &str,
) {
    assert_eq!(
        row_count(after, entity),
        row_count(before, entity),
        "{entity} row count should stay stable"
    );
}

fn event_count_for_subject(page: &ApiEventPage, event_type: &str, subject_id_text: &str) -> usize {
    page.events
        .iter()
        .filter(|event| {
            event.event_type == event_type
                && event.subject_id_text.as_deref() == Some(subject_id_text)
        })
        .count()
}

fn compact_game_view(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
) -> GameView {
    query_as::<GameView>(
        fixture,
        player,
        "get_game_view",
        (
            session_id.to_string(),
            GameViewRequest {
                viewport: opening_viewport_for_slot(0),
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
    .expect("compact game view should decode")
    .expect("compact game view should load")
}

fn setup_progress(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
) -> SetupProgressView {
    query_as::<SetupProgressView>(
        fixture,
        player,
        "get_setup_progress",
        (session_id.to_string(),),
    )
    .expect("setup progress should decode")
    .expect("setup progress should load")
}

fn visible_objects_page(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    viewport: &Viewport,
    cursor: Option<u32>,
    limit: u32,
) -> ObjectViewPage {
    query_as::<ObjectViewPage>(
        fixture,
        player,
        "get_visible_objects",
        (session_id.to_string(), viewport.clone(), cursor, limit),
    )
    .unwrap_or_else(|error| {
        panic!(
            "visible objects should decode for viewport {:?}, cursor {:?}, limit {}: {error}",
            viewport, cursor, limit
        )
    })
    .expect("visible objects should load")
}

fn visible_object<'a>(page: &'a ObjectViewPage, subject_id_text: &str) -> &'a ObjectView {
    page.objects
        .iter()
        .find(|object| object.subject_id_text == subject_id_text)
        .unwrap_or_else(|| panic!("{subject_id_text} should render in visible objects"))
}

fn diagnostic_system_jobs(
    fixture: &StandaloneCanisterFixture,
    session_id: Option<String>,
    status: Option<String>,
) -> DiagnosticSystemJobPage {
    query_as::<DiagnosticSystemJobPage>(
        fixture,
        candid::Principal::anonymous(),
        "get_diagnostic_system_jobs",
        (session_id, status, 50_u32, Option::<String>::None),
    )
    .expect("diagnostic system jobs should decode")
    .expect("diagnostic system jobs should load")
}

fn force_system_job_running(
    fixture: &StandaloneCanisterFixture,
    job_key: &str,
    lease_expires_at_ms: u64,
) -> DiagnosticSystemJobView {
    update_as::<DiagnosticSystemJobView>(
        fixture,
        candid::Principal::anonymous(),
        "force_diagnostic_system_job_running",
        (job_key.to_string(), lease_expires_at_ms),
    )
    .expect("force diagnostic system job should decode")
    .expect("force diagnostic system job should succeed")
}

fn run_diagnostic_system_job(fixture: &StandaloneCanisterFixture, job_key: &str) -> u32 {
    update_as::<u32>(
        fixture,
        candid::Principal::anonymous(),
        "run_diagnostic_system_job",
        (job_key.to_string(),),
    )
    .expect("run diagnostic system job should decode")
    .expect("run diagnostic system job should succeed")
}

fn replay_player_registration(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    nonce_stem: &str,
    player_suffix: &str,
) {
    let display_suffix = match player_suffix {
        "one" => "One",
        "two" => "Two",
        value => value,
    };
    let response = update_as::<LobbyCommandResponse>(
        fixture,
        player,
        "register_player",
        (
            Some(format!("{nonce_stem}-{player_suffix}")),
            Some(format!("{nonce_stem} {display_suffix}")),
            format!("nonce:{nonce_stem}:register:{player_suffix}"),
        ),
    )
    .expect("registration replay should decode")
    .expect("registration replay should return a command response");
    assert_eq!(response.status, CommandStatus::Applied);
}

fn turn_deadline_job_key(session_id: &str, turn: u32) -> String {
    format!("turn_deadline:{session_id}:{turn}")
}

fn millis_until_due(now_ms: u64, due_at_ms: u64) -> u64 {
    due_at_ms.saturating_sub(now_ms).saturating_add(1_000)
}

fn advance_time_ms(fixture: &StandaloneCanisterFixture, millis: u64) {
    fixture.pic().advance_time(Duration::from_millis(millis));
    fixture.pic().tick();
}

fn advance_time_without_timers(fixture: &StandaloneCanisterFixture, millis: u64) {
    fixture.pic().advance_time(Duration::from_millis(millis));
}

fn advance_time_for_timers(fixture: &StandaloneCanisterFixture, millis: u64) {
    fixture.pic().advance_time(Duration::from_millis(millis));
    fixture.pic().tick_n(5);
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
    submit_move_and_sync_until_event_after(
        fixture,
        player,
        session_id,
        champion_id,
        path,
        move_nonce,
        sync_nonce_prefix,
        now_ms,
        expected_event_type,
        61_000,
    )
}

fn submit_move_and_sync_until_event_after(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    champion_id: &str,
    path: Vec<MoveCoord>,
    move_nonce: &str,
    sync_nonce_prefix: &str,
    now_ms: u64,
    expected_event_type: &str,
    sync_advance_ms: u64,
) -> (CommandResponse, bool) {
    let max_sync_calls = path.len().saturating_add(8).max(12);
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
        sync_advance_ms,
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
    sync_advance_ms: u64,
) -> (CommandResponse, bool) {
    let max_sync_calls = max_sync_calls.max(12);
    let mut saw_partial_sync = false;
    let mut observed = Vec::new();
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
        .unwrap_or_else(|error| {
            panic!(
                "sync_session_turn {}{} should decode: {error}",
                sync_nonce_prefix, attempt
            )
        })
        .unwrap_or_else(|error| {
            panic!(
                "sync_session_turn {}{} should succeed: {error:?}",
                sync_nonce_prefix, attempt
            )
        });
        observed.push(sync_summary(attempt, &synced));
        if synced.status != CommandStatus::Applied {
            if synced
                .error
                .as_ref()
                .is_some_and(|error| error.code == "turn_not_due")
            {
                advance_time_without_timers(fixture, sync_advance_ms);
                continue;
            }
            panic!("sync_session_turn failed while waiting for {expected_event_type}: {synced:?}");
        }
        let partial_sync = synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete");
        saw_partial_sync |= partial_sync;
        if synced
            .events
            .iter()
            .any(|event| event.event_type == expected_event_type)
        {
            return (synced, saw_partial_sync);
        }
        if !partial_sync {
            advance_time_without_timers(fixture, sync_advance_ms);
        }
    }

    panic!(
        "sync_session_turn did not emit {expected_event_type} after {max_sync_calls} calls: {}",
        observed.join(" | ")
    );
}

fn sync_turn_until_event(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    sync_nonce_prefix: &str,
    expected_event_type: &str,
    max_sync_calls: usize,
) -> CommandResponse {
    let mut observed = Vec::new();
    for attempt in 0..max_sync_calls {
        fixture.pic().advance_time(Duration::from_millis(61_000));
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
        observed.push(sync_summary(attempt, &synced));
        if public_event_seen(fixture, player, session_id, expected_event_type) {
            return synced;
        }
        if synced.status != CommandStatus::Applied {
            if synced
                .error
                .as_ref()
                .is_some_and(|error| error.code == "turn_not_due")
            {
                continue;
            }
            panic!("sync_session_turn failed while waiting for {expected_event_type}: {synced:?}");
        }
        if synced
            .events
            .iter()
            .any(|event| event.event_type == expected_event_type)
        {
            return synced;
        }
    }

    panic!(
        "sync_session_turn did not emit {expected_event_type} after {max_sync_calls} calls: {}",
        observed.join(" | ")
    );
}

fn sync_summary(attempt: usize, response: &CommandResponse) -> String {
    let events = response
        .events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let error = response
        .error
        .as_ref()
        .map(|error| error.code.as_str())
        .unwrap_or("-");
    format!(
        "#{attempt}:status={:?}:turn={}:events=[{}]:error={}",
        response.status, response.effective_turn, events, error
    )
}

fn public_event_seen(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    event_type: &str,
) -> bool {
    query_as::<ApiEventPage>(
        fixture,
        player,
        "get_events_after",
        (session_id.to_string(), "public".to_string(), 0_u64, 200_u32),
    )
    .ok()
    .and_then(Result::ok)
    .is_some_and(|page| {
        page.events
            .iter()
            .any(|event| event.event_type == event_type)
    })
}

fn gate_public_event_seen(
    metrics: &mut GateJMetrics,
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    event_type: &str,
) -> bool {
    gate_query_as::<ApiEventPage>(
        metrics,
        fixture,
        player,
        "get_events_after",
        (session_id.to_string(), "public".to_string(), 0_u64, 200_u32),
    )
    .is_ok_and(|page| {
        page.events
            .iter()
            .any(|event| event.event_type == event_type)
    })
}

fn resolve_battle_to_end(
    fixture: &StandaloneCanisterFixture,
    player: candid::Principal,
    session_id: &str,
    battle_id: &str,
    nonce_prefix: &str,
) -> BattleView {
    for step in 0..160 {
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
                Ok(response) if response.status == CommandStatus::Applied => {}
                Ok(response)
                    if response
                        .error
                        .as_ref()
                        .is_some_and(|error| error.code == "turn_not_due") => {}
                Ok(response) => {
                    panic!("post-battle sync_session_turn should apply or be not due: {response:?}")
                }
                Err(error) if error.code == "turn_not_due" => {}
                Err(error) => {
                    panic!("post-battle sync_session_turn should succeed or be not due: {error:?}")
                }
            }
            return view;
        }
        if view.legal_actions_for_caller.is_empty() {
            advance_time_without_timers(fixture, domm_game::BATTLE_ACTION_DEADLINE_MS + 1);
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
    choose_battle_action_for_goal(view, true)
}

fn choose_battle_action_for_goal(view: &BattleView, aggressive: bool) -> BattleActionInput {
    if !aggressive {
        for preferred in ["Defend", "Wait"] {
            if let Some(action) = view
                .legal_actions_for_caller
                .iter()
                .find(|action| action.enabled && action.action == preferred)
            {
                return BattleActionInput {
                    battle_id: view.battle_id.clone(),
                    battle_stack_id: view
                        .active_stack_id
                        .clone()
                        .expect("active battle should have an active stack"),
                    action: action.action.clone(),
                    ability_key: action.ability_key.clone(),
                    target_stack_id: None,
                    destination: None,
                };
            }
        }
    }

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

#[track_caller]
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
    let response = match query_as::<T>(fixture, caller, method, args) {
        Ok(response) => response,
        Err(error) => panic!(
            "{method} should decode from query call for caller {}: {error}",
            caller.to_text()
        ),
    };
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

fn upgrade_degens_canister(fixture: &StandaloneCanisterFixture) {
    let wasm_path = build_degens_canister();
    let wasm = fs::read(&wasm_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", wasm_path.display()));
    fixture
        .pic()
        .retry_install_code_ok(5, Duration::from_secs(5), || {
            fixture
                .pic()
                .upgrade_canister(
                    fixture.canister_id(),
                    wasm.clone(),
                    candid::encode_args(()).expect("empty upgrade args encode"),
                    None,
                )
                .map_err(|error| error.to_string())
        })
        .expect("degens canister upgrade should succeed");
    fixture.pic().tick();
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
