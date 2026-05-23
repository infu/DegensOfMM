use std::collections::{BTreeMap, BTreeSet};

use candid::Principal;
use domm_degens_schema::schema::{
    Battle, BattleStack, Champion, FactionDefinition, GameCommand, GameParticipant, GameSession,
    RulesetDefinition, SpellDefinition, Town, UnitDefinition,
};
use domm_game::{
    BattleActionInput, BattleView, BuildingContent, ChampionClassContent, CommandPhase,
    CommandResponse, CommandStatus, ContentManifest, FIRST_PLAYABLE_RULESET_ID,
    FIRST_PLAYABLE_RULESET_SLUG, FIRST_PLAYABLE_RULESET_VERSION, LobbyCommandResult,
    MapObjectContent, MoveCoord, OPENING_QUEST_KEY, RecruitTarget, ResourceCost, SpellContent,
    UnitContent, Viewport, first_playable_content_manifest,
};
use icydb::{
    traits::EntityValue,
    types::{Id, Timestamp, Ulid},
};

use super::{
    account_lobby_session, battle as battle_service, battle_runtime, champion_magic,
    command_response, economy_expansion, events as event_service, first_playable_setup,
    flush_barrier, game_view as game_view_service, movement as movement_service,
    scenario_progress as scenario_service, session_turn_runtime, system_jobs as system_job_service,
    town as town_service, town_runtime,
};
use crate::repos::{
    battles, champions_artifacts, commands_events_effects, content, economy as economy_repo,
    economy_expansion as economy_expansion_repo, map_visibility_occupancy,
    movement as movement_repo, players, scenario_progress as scenario_repo, sessions,
    system_jobs as system_job_repo, towns,
};

fn bootstrap_service_memory() {
    icydb::__reexports::canic_memory::api::MemoryApi::bootstrap_owner_range(
        "domm-degens-canister",
        20,
        120,
    )
    .expect("service tests should reserve the generated canister memory range");
}

#[test]
fn register_player_fast_path_replays_runtime_nonce() {
    bootstrap_service_memory();

    let player = Principal::self_authenticating(b"service-register-fast-path");
    let registered = account_lobby_session::register_player(
        player,
        Some("service-fast-register".to_string()),
        Some("Service Fast Register".to_string()),
        "nonce:service-fast-register:one".to_string(),
    )
    .expect("fresh registration should not trap");
    assert_eq!(registered.status, CommandStatus::Applied);
    let player_id = match &registered.result {
        LobbyCommandResult::Player(view) => view.player_id.clone(),
        other => panic!("register returned unexpected result: {other:?}"),
    };

    let replay = account_lobby_session::register_player(
        player,
        Some("service-fast-register".to_string()),
        Some("Service Fast Register".to_string()),
        "nonce:service-fast-register:one".to_string(),
    )
    .expect("runtime registration replay should not trap");
    assert_eq!(replay.command_id, registered.command_id);

    let mismatch = account_lobby_session::register_player(
        player,
        Some("service-fast-register-renamed".to_string()),
        Some("Service Fast Register".to_string()),
        "nonce:service-fast-register:one".to_string(),
    )
    .expect("runtime registration mismatch should return a command response");
    assert_eq!(mismatch.status, CommandStatus::Failed);
    assert_eq!(
        mismatch.error.expect("mismatch should carry error").code,
        "duplicate_nonce_payload_mismatch"
    );

    let second_nonce = account_lobby_session::register_player(
        player,
        Some("service-fast-register".to_string()),
        Some("Service Fast Register".to_string()),
        "nonce:service-fast-register:two".to_string(),
    )
    .expect("registered principal with a new nonce should return the existing player");
    let second_player_id = match &second_nonce.result {
        LobbyCommandResult::Player(view) => view.player_id.clone(),
        other => panic!("register second nonce returned unexpected result: {other:?}"),
    };
    assert_eq!(second_player_id, player_id);
}

fn create_ready_two_player_lobby(prefix: &str) -> (Principal, Principal, String) {
    let player_one_seed = format!("{prefix}-player-one");
    let player_two_seed = format!("{prefix}-player-two");
    let player_one = Principal::self_authenticating(player_one_seed.as_bytes());
    let player_two = Principal::self_authenticating(player_two_seed.as_bytes());

    account_lobby_session::register_player(
        player_one,
        Some(format!("{prefix}-one")),
        Some(format!("{prefix} One")),
        format!("nonce:{prefix}:register:one"),
    )
    .expect("player one registration should not trap");
    account_lobby_session::register_player(
        player_two,
        Some(format!("{prefix}-two")),
        Some(format!("{prefix} Two")),
        format!("nonce:{prefix}:register:two"),
    )
    .expect("player two registration should not trap");

    let created = account_lobby_session::create_session(
        player_one,
        format!("{prefix} Match"),
        FIRST_PLAYABLE_RULESET_ID.to_string(),
        19_105,
        format!("nonce:{prefix}:create"),
    )
    .expect("session creation should not trap");
    let session_id = match created.result {
        LobbyCommandResult::Session(session) => session.session_id,
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    account_lobby_session::join_session(
        player_two,
        session_id.clone(),
        "faction:ashen-ledger".to_string(),
        format!("nonce:{prefix}:join"),
    )
    .expect("join should not trap");
    account_lobby_session::mark_ready(
        player_one,
        session_id.clone(),
        format!("nonce:{prefix}:ready:one"),
    )
    .expect("player one ready should not trap");
    account_lobby_session::mark_ready(
        player_two,
        session_id.clone(),
        format!("nonce:{prefix}:ready:two"),
    )
    .expect("player two ready should not trap");

    (player_one, player_two, session_id)
}

fn seeded_content_set(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    values.into_iter().collect()
}

fn isolated_manifest_ruleset(
    prefix: &str,
) -> (
    RulesetDefinition,
    BTreeMap<String, FactionDefinition>,
    ContentManifest,
) {
    let manifest = first_playable_content_manifest();
    let unique = Ulid::generate().to_string();
    let ruleset = content::create_ruleset_definition(
        format!("{prefix}-{unique}"),
        1,
        format!("{prefix} Rules"),
        None,
        Some(format!("{prefix}-hash-{unique}")),
    )
    .expect("isolated content ruleset should seed");
    let mut factions = BTreeMap::new();
    for faction in &manifest.factions {
        let row = content::create_faction_definition(
            ruleset.id(),
            faction.slug.clone(),
            faction.name.clone(),
            faction.trait_key.clone(),
        )
        .expect("isolated manifest faction should seed");
        factions.insert(faction.slug.clone(), row);
    }
    (ruleset, factions, manifest)
}

struct MagicSessionFixture {
    caller: Principal,
    session_id: String,
    session: GameSession,
    champion: Champion,
    hex_spark: SpellDefinition,
    spite_march: SpellDefinition,
}

fn magic_spell_content(
    ruleset_slug: &str,
    slug: &str,
    name: &str,
    mana_cost: u16,
    target_type: &str,
    effect_key: &str,
) -> SpellContent {
    SpellContent {
        id: format!("spell:{slug}"),
        ruleset_id: ruleset_slug.to_string(),
        slug: slug.to_string(),
        name: name.to_string(),
        description: Some("Test misery spell.".to_string()),
        icon_key: Some(format!("icon:spell:{slug}")),
        school: "misery".to_string(),
        level: 1,
        mana_cost,
        target_type: target_type.to_string(),
        effect_key: effect_key.to_string(),
        duration_rounds: 0,
    }
}

fn create_active_magic_session(
    prefix: &str,
    skill_points: u16,
    skill_keys: Vec<String>,
    mana_max: u16,
) -> MagicSessionFixture {
    let unique = Ulid::generate().to_string();
    let principal_seed = format!("{prefix}-{unique}");
    let caller = Principal::self_authenticating(principal_seed.as_bytes());
    let player = players::create_player_account(
        icydb::types::Principal::from(caller),
        Some(format!("magic-{}", &unique[..20])),
        Some(format!("{prefix} Magic")),
    )
    .expect("magic player should seed");
    let ruleset_slug = format!("{prefix}-rules-{unique}");
    let ruleset = content::create_ruleset_definition(
        ruleset_slug.clone(),
        1,
        format!("{prefix} Rules"),
        None,
        Some(format!("{prefix}-hash-{unique}")),
    )
    .expect("magic ruleset should seed");
    let faction = content::create_faction_definition(
        ruleset.id(),
        format!("{prefix}-faction-{unique}"),
        format!("{prefix} Faction"),
        "misery".to_string(),
    )
    .expect("magic faction should seed");
    let class_slug = format!("magic-{}", &unique[..16]);
    let class = content::create_champion_class_definition(
        ruleset.id(),
        Some(faction.id()),
        ChampionClassContent {
            id: format!("class:{class_slug}"),
            ruleset_id: ruleset_slug.clone(),
            faction_slug: None,
            slug: class_slug.clone(),
            name: format!("{prefix} Class"),
            description: None,
            portrait_key: None,
            base_movement: 24,
            base_vision: 4,
        },
    )
    .expect("magic class should seed");
    let hex_spark = content::create_spell_definition(
        ruleset.id(),
        magic_spell_content(
            &ruleset_slug,
            "hex-spark",
            "Hex Spark",
            3,
            "enemy_battle_stack",
            "spell:hex_spark_damage_15",
        ),
    )
    .expect("hex-spark should seed");
    let spite_march = content::create_spell_definition(
        ruleset.id(),
        magic_spell_content(
            &ruleset_slug,
            "spite-march",
            "Spite March",
            2,
            "self_champion",
            "spell:spite_march_movement_30",
        ),
    )
    .expect("spite-march should seed");
    let mut session = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        format!("{prefix} Session"),
        19_105,
        16,
        16,
        Timestamp::from_millis(9_000_000_000_000),
    )
    .expect("magic session should seed");
    session.state = "active".to_string();
    session.current_turn = 7;
    session.turn_started_at = Timestamp::from_millis(1_000);
    session.turn_deadline_at = Timestamp::from_millis(9_000_000_000_000);
    let mut session =
        sessions::update_session(session).expect("active magic session should persist");
    let participant = sessions::create_participant(
        session.id(),
        player.id(),
        faction.id(),
        0,
        "red".to_string(),
    )
    .expect("magic participant should seed");
    let champion = champions_artifacts::insert_champion_with_id(
        Id::from_key(Ulid::generate()),
        session.id(),
        participant.id(),
        class.id(),
        format!("{prefix} Champion"),
        class_slug,
        "active".to_string(),
        4,
        4,
        0,
        0,
        1,
        0,
        1,
        1,
        1,
        1,
        mana_max,
        mana_max,
        session.current_turn,
        skill_points,
        skill_keys,
        24,
        24,
        session.current_turn,
        4,
        0,
    )
    .expect("magic champion should seed");
    let runtime = session_turn_runtime::prepare_active_turn_runtime(&mut session)
        .expect("active magic runtime should prepare")
        .expect("active magic runtime should be new");
    let session = sessions::update_session(session).expect("prepared magic session should persist");
    session_turn_runtime::insert_runtime(runtime);

    MagicSessionFixture {
        caller,
        session_id: session.id().to_string(),
        session,
        champion,
        hex_spark,
        spite_march,
    }
}

fn assert_failed_response_code(response: &CommandResponse, code: &str) {
    assert_eq!(response.status, CommandStatus::Failed);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some(code)
    );
}

fn assert_status_not_found_by_nonce(
    caller: Principal,
    session_id: &str,
    command_type: &str,
    client_nonce: &str,
) {
    let err = event_service::get_command_status_by_nonce(
        caller,
        session_id.to_string(),
        command_type.to_string(),
        client_nonce.to_string(),
    )
    .expect_err("command status should not exist for transient pre-command failure");
    assert_eq!(err.code, "command_status_not_found");
}

fn assert_status_by_nonce(
    caller: Principal,
    session_id: &str,
    command_type: &str,
    client_nonce: &str,
    expected_command_id: &str,
    expected_status: CommandStatus,
    expected_error_code: Option<&str>,
) -> domm_game::CommandStatusView {
    let status = event_service::get_command_status_by_nonce(
        caller,
        session_id.to_string(),
        command_type.to_string(),
        client_nonce.to_string(),
    )
    .expect("command status by nonce should exist");
    assert_eq!(status.command_id, expected_command_id);
    assert_eq!(status.status, expected_status);
    assert_eq!(status.error_code.as_deref(), expected_error_code);
    status
}

fn assert_status_by_id_matches(
    caller: Principal,
    session_id: &str,
    command_id: &str,
    expected: &domm_game::CommandStatusView,
) {
    let by_id =
        event_service::get_command_status(caller, session_id.to_string(), command_id.to_string())
            .expect("command status by id should exist");
    assert_eq!(&by_id, expected);
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn restore_session_turn_runtime_after_upgrade() {
    session_turn_runtime::persist_snapshot_for_upgrade()
        .expect("session turn runtime snapshot should persist for upgrade");
    session_turn_runtime::clear_all_for_tests();
    session_turn_runtime::restore_snapshot_after_upgrade()
        .expect("session turn runtime snapshot should restore after upgrade");
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn restore_battle_runtime_after_upgrade() {
    battle_runtime::persist_snapshot_for_upgrade()
        .expect("battle runtime snapshot should persist for upgrade");
    battle_runtime::clear_all_for_tests();
    battle_runtime::restore_snapshot_after_upgrade()
        .expect("battle runtime snapshot should restore after upgrade");
}

fn create_durable_spell_row(fixture: &MagicSessionFixture, spell: &SpellDefinition) {
    champions_artifacts::create_champion_spell(
        fixture.session.id(),
        fixture.champion.id(),
        spell.id(),
        &spell.slug,
        fixture.session.current_turn,
        Id::<GameCommand>::from_key(Ulid::generate()),
    )
    .expect("durable champion spell row should seed");
}

fn fill_runtime_spellbook_to_cap(fixture: &MagicSessionFixture) {
    for index in 0..domm_game::CHAMPION_SPELLBOOK_CAP {
        session_turn_runtime::mirror_champion_spell_snapshot(
            &fixture.session_id,
            fixture.session.current_turn,
            session_turn_runtime::RuntimeChampionSpell {
                champion_id: fixture.champion.id().key(),
                spell_id: Ulid::generate(),
                spell_slug: Some(format!("known-{index}")),
                learned_turn: fixture.session.current_turn,
                last_command_id: None,
                needs_flush: false,
            },
        );
    }
}

struct TownRecruitSessionFixture {
    caller: Principal,
    session_id: String,
    town: Town,
    champion: Champion,
    unit: UnitDefinition,
}

fn town_recruit_unit_content(ruleset_slug: &str, unit_slug: &str) -> UnitContent {
    UnitContent {
        id: format!("unit:{unit_slug}"),
        ruleset_id: ruleset_slug.to_string(),
        faction_slug: None,
        slug: unit_slug.to_string(),
        name: "Test Levy".to_string(),
        description: None,
        sprite_key: None,
        icon_key: None,
        animation_key: None,
        tier: 1,
        attack: 1,
        defense: 1,
        damage_min: 1,
        damage_max: 2,
        max_hp: 9,
        speed: 4,
        initiative: 4,
        ranged: false,
        flying: false,
        shots: 0,
        cost: ResourceCost {
            gold: 5,
            ..ResourceCost::zero()
        },
        weekly_growth: 8,
        ability_keys: Vec::new(),
    }
}

fn create_active_town_recruit_session(
    prefix: &str,
    champion_x: u16,
    champion_y: u16,
) -> TownRecruitSessionFixture {
    let unique = Ulid::generate().to_string();
    let caller = Principal::self_authenticating(format!("{prefix}-{unique}").as_bytes());
    let player = players::create_player_account(
        icydb::types::Principal::from(caller),
        Some(format!("town-{}", &unique[..20])),
        Some(format!("{prefix} Town")),
    )
    .expect("town recruit player should seed");
    let ruleset_slug = format!("{prefix}-rules-{unique}");
    let ruleset = content::create_ruleset_definition(
        ruleset_slug.clone(),
        1,
        format!("{prefix} Rules"),
        None,
        Some(format!("{prefix}-hash-{unique}")),
    )
    .expect("town recruit ruleset should seed");
    let faction = content::create_faction_definition(
        ruleset.id(),
        format!("{prefix}-faction-{unique}"),
        format!("{prefix} Faction"),
        "freehold".to_string(),
    )
    .expect("town recruit faction should seed");
    let class_slug = format!("town-class-{}", &unique[..12]);
    let class = content::create_champion_class_definition(
        ruleset.id(),
        Some(faction.id()),
        ChampionClassContent {
            id: format!("class:{class_slug}"),
            ruleset_id: ruleset_slug.clone(),
            faction_slug: Some(faction.slug.clone()),
            slug: class_slug.clone(),
            name: format!("{prefix} Captain"),
            description: None,
            portrait_key: None,
            base_movement: 24,
            base_vision: 4,
        },
    )
    .expect("town recruit champion class should seed");
    let unit_slug = format!("levy-{}", &unique[..12]);
    let unit = content::create_unit_definition(
        ruleset.id(),
        Some(faction.id()),
        town_recruit_unit_content(&ruleset_slug, &unit_slug),
    )
    .expect("town recruit unit should seed");

    let mut session = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        format!("{prefix} Session"),
        19_105,
        16,
        16,
        Timestamp::from_millis(9_000_000_000_000),
    )
    .expect("town recruit session should seed");
    session.state = "active".to_string();
    session.current_turn = 7;
    session.turn_started_at = Timestamp::from_millis(1_000);
    session.turn_deadline_at = Timestamp::from_millis(9_000_000_000_000);
    let mut session =
        sessions::update_session(session).expect("active town recruit session should persist");
    let participant = sessions::create_participant(
        session.id(),
        player.id(),
        faction.id(),
        0,
        "red".to_string(),
    )
    .expect("town recruit participant should seed");
    let champion = champions_artifacts::insert_champion_with_id(
        Id::from_key(Ulid::generate()),
        session.id(),
        participant.id(),
        class.id(),
        format!("{prefix} Champion"),
        class_slug,
        "active".to_string(),
        champion_x,
        champion_y,
        0,
        0,
        1,
        0,
        1,
        1,
        1,
        1,
        10,
        10,
        session.current_turn,
        0,
        Vec::new(),
        24,
        24,
        session.current_turn,
        4,
        0,
    )
    .expect("town recruit champion should seed");
    sessions::ensure_participant_champion_id(participant, champion.id())
        .expect("town recruit participant champion id should persist");
    let town = towns::create_town(
        session.id(),
        Some(Id::from_key(champion.participant_id)),
        faction.id(),
        format!("{prefix} Town"),
        4,
        4,
        0,
        0,
        "active".to_string(),
        1,
        0,
        0,
        0,
        0,
        0,
    )
    .expect("town recruit town should seed");
    towns::create_town_recruit_pool(session.id(), town.id(), unit.id(), unit.slug.clone(), 10, 1)
        .expect("town recruit pool should seed");
    let runtime = session_turn_runtime::prepare_active_turn_runtime(&mut session)
        .expect("active town recruit runtime should prepare")
        .expect("active town recruit runtime should be new");
    let session =
        sessions::update_session(session).expect("prepared town recruit session should persist");
    session_turn_runtime::insert_runtime(runtime);

    TownRecruitSessionFixture {
        caller,
        session_id: session.id().to_string(),
        town,
        champion,
        unit,
    }
}

struct EconomySessionFixture {
    caller: Principal,
    session_id: String,
    participant_id: Id<GameParticipant>,
    town: Town,
    champion: Champion,
    unit: UnitDefinition,
    base_building_slug: String,
    locked_building_slug: String,
    dwelling_object_id: String,
    offer_key: String,
}

fn economy_building_content(
    ruleset_slug: &str,
    slug: &str,
    name: &str,
    cost: ResourceCost,
    requires_building_slugs: Vec<String>,
) -> BuildingContent {
    BuildingContent {
        id: format!("building:{slug}"),
        ruleset_id: ruleset_slug.to_string(),
        faction_slug: None,
        slug: slug.to_string(),
        name: name.to_string(),
        description: None,
        icon_key: None,
        building_type: "economy-test".to_string(),
        cost,
        requires_building_slugs,
        unlocks_unit_slug: None,
        effect_key: None,
    }
}

fn economy_dwelling_object_content(ruleset_slug: &str, slug: &str) -> MapObjectContent {
    MapObjectContent {
        id: format!("object:{slug}"),
        ruleset_id: ruleset_slug.to_string(),
        slug: slug.to_string(),
        name: "Test Dwelling".to_string(),
        description: None,
        sprite_key: None,
        icon_key: None,
        object_type: "dwelling".to_string(),
        footprint_w: 1,
        footprint_h: 1,
        blocking: false,
        interaction_key: "external_dwelling".to_string(),
        refresh_rule: "weekly".to_string(),
    }
}

fn create_active_economy_session(prefix: &str, dwelling_available: u32) -> EconomySessionFixture {
    let unique = Ulid::generate().to_string();
    let caller = Principal::self_authenticating(format!("{prefix}-{unique}").as_bytes());
    let player = players::create_player_account(
        icydb::types::Principal::from(caller),
        Some(format!("economy-{}", &unique[..20])),
        Some(format!("{prefix} Economy")),
    )
    .expect("economy player should seed");
    let ruleset_slug = format!("{prefix}-rules-{unique}");
    let ruleset = content::create_ruleset_definition(
        ruleset_slug.clone(),
        1,
        format!("{prefix} Rules"),
        None,
        Some(format!("{prefix}-hash-{unique}")),
    )
    .expect("economy ruleset should seed");
    let faction = content::create_faction_definition(
        ruleset.id(),
        format!("{prefix}-faction-{unique}"),
        format!("{prefix} Faction"),
        "freehold".to_string(),
    )
    .expect("economy faction should seed");
    let class_slug = format!("economy-class-{}", &unique[..12]);
    let class = content::create_champion_class_definition(
        ruleset.id(),
        Some(faction.id()),
        ChampionClassContent {
            id: format!("class:{class_slug}"),
            ruleset_id: ruleset_slug.clone(),
            faction_slug: Some(faction.slug.clone()),
            slug: class_slug.clone(),
            name: format!("{prefix} Captain"),
            description: None,
            portrait_key: None,
            base_movement: 24,
            base_vision: 4,
        },
    )
    .expect("economy champion class should seed");
    let unique_tail = &unique[18..26];
    let unit_slug = format!("levy-{unique_tail}");
    let unit = content::create_unit_definition(
        ruleset.id(),
        Some(faction.id()),
        town_recruit_unit_content(&ruleset_slug, &unit_slug),
    )
    .expect("economy unit should seed");
    let base_building_slug = format!("yard-{unique_tail}");
    content::create_building_definition(
        ruleset.id(),
        Some(faction.id()),
        economy_building_content(
            &ruleset_slug,
            &base_building_slug,
            "Market Yard",
            ResourceCost {
                gold: 7,
                wood: 3,
                ..ResourceCost::zero()
            },
            Vec::new(),
        ),
    )
    .expect("economy base building should seed");
    let locked_building_slug = format!("hall-{unique_tail}");
    content::create_building_definition(
        ruleset.id(),
        Some(faction.id()),
        economy_building_content(
            &ruleset_slug,
            &locked_building_slug,
            "Locked Hall",
            ResourceCost {
                gold: 5,
                ..ResourceCost::zero()
            },
            vec![base_building_slug.clone()],
        ),
    )
    .expect("economy locked building should seed");
    let object_def = content::create_map_object_definition(
        ruleset.id(),
        economy_dwelling_object_content(&ruleset_slug, &format!("dwelling-{}", &unique[..12])),
    )
    .expect("economy dwelling object definition should seed");

    let mut session = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        format!("{prefix} Session"),
        19_105,
        16,
        16,
        Timestamp::from_millis(9_000_000_000_000),
    )
    .expect("economy session should seed");
    session.state = "active".to_string();
    session.current_turn = 7;
    session.turn_started_at = Timestamp::from_millis(1_000);
    session.turn_deadline_at = Timestamp::from_millis(9_000_000_000_000);
    let mut session = sessions::update_session(session).expect("active economy session persists");
    let participant = sessions::create_participant(
        session.id(),
        player.id(),
        faction.id(),
        0,
        "red".to_string(),
    )
    .expect("economy participant should seed");
    let champion = champions_artifacts::insert_champion_with_id(
        Id::from_key(Ulid::generate()),
        session.id(),
        participant.id(),
        class.id(),
        format!("{prefix} Champion"),
        class_slug,
        "active".to_string(),
        4,
        4,
        0,
        0,
        1,
        0,
        1,
        1,
        1,
        1,
        10,
        10,
        session.current_turn,
        0,
        Vec::new(),
        24,
        24,
        session.current_turn,
        4,
        0,
    )
    .expect("economy champion should seed");
    let participant = sessions::ensure_participant_champion_id(participant, champion.id())
        .expect("economy participant champion id should persist");
    let town = towns::create_town(
        session.id(),
        Some(participant.id()),
        faction.id(),
        format!("{prefix} Town"),
        4,
        4,
        0,
        0,
        "active".to_string(),
        1,
        0,
        0,
        0,
        0,
        0,
    )
    .expect("economy town should seed");
    let week_number = domm_game::week_for_turn(session.current_turn);
    towns::create_town_recruit_pool(session.id(), town.id(), unit.id(), unit.slug.clone(), 8, 1)
        .expect("economy town recruit pool should seed");
    let offer_key = domm_game::tavern_offer_key(&town.id().to_string(), week_number, 0);
    economy_expansion_repo::create_tavern_offer(
        session.id(),
        town.id(),
        participant.id(),
        week_number,
        0,
        offer_key.clone(),
        class.id(),
        class.slug.clone(),
        format!("{prefix} Hire"),
        100,
    )
    .expect("economy tavern offer should seed");
    economy_expansion_repo::create_tavern_offer(
        session.id(),
        town.id(),
        participant.id(),
        week_number,
        1,
        domm_game::tavern_offer_key(&town.id().to_string(), week_number, 1),
        class.id(),
        class.slug.clone(),
        format!("{prefix} Reserve Hire"),
        150,
    )
    .expect("economy reserve tavern offer should seed");
    let dwelling_object = map_visibility_occupancy::create_world_object(
        session.id(),
        object_def.id(),
        Some(participant.id()),
        None,
        6,
        4,
        0,
        0,
        "captured".to_string(),
        "dwelling".to_string(),
        session.current_turn,
        session.current_turn,
        session.current_turn,
        None,
    )
    .expect("economy dwelling object should seed");
    let dwelling_pool = economy_expansion_repo::create_dwelling_pool(
        session.id(),
        dwelling_object.id(),
        Some(participant.id()),
        unit.id(),
        unit.slug.clone(),
        dwelling_available,
        week_number,
        0,
        true,
    )
    .expect("economy dwelling pool should seed");
    economy_expansion::mirror_runtime_dwelling_pool(&dwelling_pool);

    let runtime = session_turn_runtime::prepare_active_turn_runtime(&mut session)
        .expect("active economy runtime should prepare")
        .expect("active economy runtime should be new");
    let session = sessions::update_session(session).expect("prepared economy session persists");
    session_turn_runtime::insert_runtime(runtime);

    EconomySessionFixture {
        caller,
        session_id: session.id().to_string(),
        participant_id: participant.id(),
        town,
        champion,
        unit,
        base_building_slug,
        locked_building_slug,
        dwelling_object_id: dwelling_object.id().to_string(),
        offer_key,
    }
}

fn create_active_economy_negative_session(prefix: &str) -> EconomySessionFixture {
    create_active_economy_session(prefix, 0)
}

fn create_active_city_economy_session(prefix: &str) -> EconomySessionFixture {
    create_active_economy_session(prefix, 3)
}

struct ScenarioProgressFixture {
    caller_one: Principal,
    caller_two: Principal,
    session_id: String,
    session: GameSession,
    participant_one: GameParticipant,
    participant_two: GameParticipant,
}

fn scenario_object_content(ruleset_slug: &str, slug: &str) -> MapObjectContent {
    MapObjectContent {
        id: format!("object:{slug}"),
        ruleset_id: ruleset_slug.to_string(),
        slug: slug.to_string(),
        name: "Scenario Objective".to_string(),
        description: None,
        sprite_key: None,
        icon_key: None,
        object_type: "central_objective".to_string(),
        footprint_w: 1,
        footprint_h: 1,
        blocking: true,
        interaction_key: "score_central_objective".to_string(),
        refresh_rule: "owner_score".to_string(),
    }
}

fn create_active_scenario_progress_session(
    prefix: &str,
    seeded_turn: u32,
    active_turn: u32,
    max_turns: u32,
    opening_quest_progress: u32,
) -> ScenarioProgressFixture {
    let unique = Ulid::generate().to_string();
    let caller_one = Principal::self_authenticating(format!("{prefix}-one-{unique}").as_bytes());
    let caller_two = Principal::self_authenticating(format!("{prefix}-two-{unique}").as_bytes());
    let player_one = players::create_player_account(
        icydb::types::Principal::from(caller_one),
        Some(format!("scenario-one-{}", &unique[..12])),
        Some(format!("{prefix} Scenario One")),
    )
    .expect("scenario player one should seed");
    let player_two = players::create_player_account(
        icydb::types::Principal::from(caller_two),
        Some(format!("scenario-two-{}", &unique[..12])),
        Some(format!("{prefix} Scenario Two")),
    )
    .expect("scenario player two should seed");
    let ruleset_slug = format!("{prefix}-rules-{unique}");
    let ruleset = content::create_ruleset_definition(
        ruleset_slug.clone(),
        1,
        format!("{prefix} Rules"),
        None,
        Some(format!("{prefix}-hash-{unique}")),
    )
    .expect("scenario ruleset should seed");
    let faction = content::create_faction_definition(
        ruleset.id(),
        format!("{prefix}-faction-{unique}"),
        format!("{prefix} Faction"),
        "scenario".to_string(),
    )
    .expect("scenario faction should seed");
    let objective_def = content::create_map_object_definition(
        ruleset.id(),
        scenario_object_content(&ruleset_slug, &format!("objective-{}", &unique[..12])),
    )
    .expect("scenario objective definition should seed");
    let mut session = sessions::create_game_session(
        ruleset.id(),
        player_one.id(),
        format!("{prefix} Session"),
        19_105,
        48,
        48,
        Timestamp::from_millis(9_000_000_000_000),
    )
    .expect("scenario session should seed");
    session.state = "active".to_string();
    session.current_turn = seeded_turn;
    session.max_turns = max_turns;
    session.turn_started_at = Timestamp::from_millis(1_000);
    session.turn_deadline_at = Timestamp::from_millis(9_000_000_000_000);
    let mut session = sessions::update_session(session).expect("scenario session should persist");
    let participant_one = sessions::create_participant(
        session.id(),
        player_one.id(),
        faction.id(),
        0,
        "red".to_string(),
    )
    .expect("scenario participant one should seed");
    let participant_two = sessions::create_participant(
        session.id(),
        player_two.id(),
        faction.id(),
        1,
        "blue".to_string(),
    )
    .expect("scenario participant two should seed");

    for seed in &domm_game::first_playable_scenario().central_objectives {
        map_visibility_occupancy::create_world_object(
            session.id(),
            objective_def.id(),
            None,
            None,
            seed.x,
            seed.y,
            seed.x / u16::from(domm_game::FIRST_PLAYABLE_CHUNK_SIZE),
            seed.y / u16::from(domm_game::FIRST_PLAYABLE_CHUNK_SIZE),
            "available".to_string(),
            "central_objective".to_string(),
            0,
            0,
            0,
            None,
        )
        .expect("scenario central objective object should seed");
    }

    scenario_service::ensure_seeded_scenario_progress(
        &session,
        &[participant_one.clone(), participant_two.clone()],
    )
    .expect("scenario progress rows should seed");
    let mut quest = scenario_repo::find_quest_by_participant_key(
        session.id(),
        participant_one.id(),
        OPENING_QUEST_KEY,
    )
    .expect("opening quest lookup should not fail")
    .expect("opening quest should seed for participant one");
    quest.progress_value = opening_quest_progress;
    scenario_repo::update_quest_state(quest).expect("opening quest progress should update");

    session.current_turn = active_turn;
    let mut session =
        sessions::update_session(session).expect("active scenario turn should persist");
    let runtime = session_turn_runtime::prepare_active_turn_runtime(&mut session)
        .expect("active scenario runtime should prepare")
        .expect("active scenario runtime should be new");
    let session = sessions::update_session(session).expect("prepared scenario session persists");
    session_turn_runtime::insert_runtime(runtime);

    ScenarioProgressFixture {
        caller_one,
        caller_two,
        session_id: session.id().to_string(),
        session,
        participant_one,
        participant_two,
    }
}

fn complete_runtime_opening_quest(fixture: &ScenarioProgressFixture) {
    let mut quest = session_turn_runtime::quest_snapshot(
        &fixture.session_id,
        fixture.session.current_turn,
        &fixture.participant_one.id().to_string(),
        OPENING_QUEST_KEY,
    )
    .expect("accepted runtime quest snapshot should exist");
    quest.progress_value = quest.required_value;
    assert!(
        session_turn_runtime::mirror_quest_snapshot(
            &fixture.session_id,
            fixture.session.current_turn,
            quest,
        ),
        "opening quest completion should mirror into runtime"
    );
}

fn capture_runtime_central_objectives(fixture: &ScenarioProgressFixture) {
    for seed in &domm_game::first_playable_scenario().central_objectives {
        let mut object = map_visibility_occupancy::find_world_object_by_session_xy(
            fixture.session.id(),
            seed.x,
            seed.y,
        )
        .expect("central objective lookup should not fail")
        .expect("central objective should exist");
        object.owner_participant_id = Some(fixture.participant_one.id().key());
        object.state = "captured".to_string();
        object.captured_turn = fixture.session.current_turn;
        let object = map_visibility_occupancy::update_world_object(object)
            .expect("central objective capture should persist");
        session_turn_runtime::mirror_world_object_update(&object);
    }
}

struct RuntimeBattleDeadlineFixture {
    caller: Principal,
    session: GameSession,
    participant: GameParticipant,
    input: BattleActionInput,
    nonce: String,
    deadline_ms: u64,
}

fn create_runtime_battle_deadline_fixture(prefix: &str) -> RuntimeBattleDeadlineFixture {
    let unique = Ulid::generate().to_string();
    let caller = Principal::self_authenticating(format!("{prefix}-{unique}").as_bytes());
    let player = players::create_player_account(
        icydb::types::Principal::from(caller),
        Some(format!("battle-deadline-{}", &unique[..12])),
        Some(format!("{prefix} Battle Deadline")),
    )
    .expect("battle deadline player should seed");
    let ruleset = content::create_ruleset_definition(
        format!("{prefix}-rules-{unique}"),
        1,
        format!("{prefix} Rules"),
        None,
        Some(format!("{prefix}-hash-{unique}")),
    )
    .expect("battle deadline ruleset should seed");
    let faction = content::create_faction_definition(
        ruleset.id(),
        format!("{prefix}-faction-{unique}"),
        format!("{prefix} Faction"),
        "battle".to_string(),
    )
    .expect("battle deadline faction should seed");
    let mut session = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        format!("{prefix} Session"),
        19_105,
        16,
        16,
        Timestamp::from_millis(9_000_000_000_000),
    )
    .expect("battle deadline session should seed");
    session.state = "active".to_string();
    session.current_turn = 5;
    session.turn_started_at = Timestamp::from_millis(1_000);
    session.turn_deadline_at = Timestamp::from_millis(9_000_000_000_000);
    let session = sessions::update_session(session).expect("battle deadline session persists");
    let participant = sessions::create_participant(
        session.id(),
        player.id(),
        faction.id(),
        0,
        "red".to_string(),
    )
    .expect("battle deadline participant should seed");

    let deadline_ms = 10_000_u64;
    let durable_battle = battles::create_battle(
        session.id(),
        "resolved".to_string(),
        "test".to_string(),
        None,
        None,
        None,
        None,
        domm_game::BATTLE_SIDE_ATTACKER.to_string(),
        domm_game::BATTLE_GRID_WIDTH,
        domm_game::BATTLE_GRID_HEIGHT,
        domm_game::BATTLE_MAX_ROUNDS,
        42,
        session.current_turn,
        Some(Timestamp::from_millis(
            deadline_ms
                .try_into()
                .expect("battle deadline millis should fit timestamp"),
        )),
        Id::<GameCommand>::from_key(Ulid::generate()),
    )
    .expect("battle deadline durable header should seed");
    let battle_id = durable_battle.id().to_string();
    let attacker_stack_id = format!("battle-stack:{battle_id}:attacker:0");
    let defender_stack_id = format!("battle-stack:{battle_id}:defender:0");
    let state = domm_game::BattleState {
        session_seed: session.seed.to_string(),
        battles: vec![domm_game::BattleRecord {
            battle_id: battle_id.clone(),
            session_id: session.id().to_string(),
            state: "active".to_string(),
            battle_type: "test".to_string(),
            attacker_champion_id: None,
            defender_champion_id: None,
            defender_town_id: None,
            defender_neutral_army_id: None,
            current_round: 1,
            active_side: domm_game::BATTLE_SIDE_ATTACKER.to_string(),
            active_stack_id: Some(attacker_stack_id.clone()),
            grid_width: domm_game::BATTLE_GRID_WIDTH,
            grid_height: domm_game::BATTLE_GRID_HEIGHT,
            max_rounds: domm_game::BATTLE_MAX_ROUNDS,
            turn_seed: 42,
            winner_participant_id: None,
            created_turn: session.current_turn,
            action_deadline_at: Some(deadline_ms),
            resolved_at: None,
            cleanup_after_turn: 0,
            last_command_id: None,
        }],
        stacks: vec![
            runtime_battle_stack(
                &battle_id,
                &attacker_stack_id,
                Some(participant.id().to_string()),
                domm_game::BATTLE_SIDE_ATTACKER,
                0,
                1,
                4,
            ),
            runtime_battle_stack(
                &battle_id,
                &defender_stack_id,
                None,
                domm_game::BATTLE_SIDE_DEFENDER,
                1,
                10,
                4,
            ),
        ],
        obstacles: Vec::new(),
        occupancy: vec![
            domm_game::BattleOccupancyRecord {
                battle_occupancy_id: format!("battle-occupancy:{attacker_stack_id}"),
                battle_id: battle_id.clone(),
                battle_stack_id: attacker_stack_id.clone(),
                battle_x: 1,
                battle_y: 4,
                last_command_id: None,
            },
            domm_game::BattleOccupancyRecord {
                battle_occupancy_id: format!("battle-occupancy:{defender_stack_id}"),
                battle_id: battle_id.clone(),
                battle_stack_id: defender_stack_id,
                battle_x: 10,
                battle_y: 4,
                last_command_id: None,
            },
        ],
        commands: Vec::new(),
        events: Vec::new(),
    };
    let runtime =
        battle_runtime::build_runtime_from_state(&session, state).expect("runtime should build");
    battle_runtime::insert_runtime(runtime);

    RuntimeBattleDeadlineFixture {
        caller,
        session,
        participant,
        input: BattleActionInput {
            battle_id,
            battle_stack_id: attacker_stack_id,
            action: "Defend".to_string(),
            ability_key: None,
            target_stack_id: None,
            destination: None,
        },
        nonce: format!("nonce:{prefix}:battle-action"),
        deadline_ms,
    }
}

fn runtime_battle_stack(
    battle_id: &str,
    stack_id: &str,
    owner_participant_id: Option<String>,
    side: &str,
    slot_index: u8,
    battle_x: u8,
    battle_y: u8,
) -> domm_game::BattleStackRecord {
    domm_game::BattleStackRecord {
        battle_stack_id: stack_id.to_string(),
        battle_id: battle_id.to_string(),
        unit_id: format!("unit:{side}:{slot_index}"),
        owner_participant_id,
        side: side.to_string(),
        slot_index,
        origin_kind: "test".to_string(),
        origin_stack_id_text: None,
        origin_slot_index: slot_index,
        champion_might: 0,
        champion_guard: 0,
        attack: 4,
        defense: 4,
        damage_min: 1,
        damage_max: 2,
        max_hp: 10,
        speed: 4,
        initiative: 8_u8.saturating_sub(slot_index),
        ranged: false,
        flying: false,
        quantity: 10,
        front_hp: 10,
        shots_remaining: 0,
        battle_x,
        battle_y,
        readiness: 0,
        acted_round: 0,
        retaliated_round: 0,
        defended_round: 0,
        waited_round: 0,
        cast_round: 0,
        status: "active".to_string(),
        last_command_id: None,
        status_keys: Vec::new(),
    }
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn failed_command_transient_rejections_do_not_create_receipts() {
    bootstrap_service_memory();

    let payload_fixture = create_active_magic_session("fr-payload", 1, Vec::new(), 4);
    let payload_nonce = "nonce:failed-receipts:payload-too-large".to_string();
    let huge_skill_key = "x".repeat(domm_game::MAX_COMMAND_PAYLOAD_JSON_BYTES + 1);
    let payload_failure = champion_magic::select_champion_level_up(
        payload_fixture.caller,
        payload_fixture.session_id.clone(),
        payload_fixture.champion.id().to_string(),
        huge_skill_key,
        payload_nonce.clone(),
    )
    .expect("payload-too-large command should return a failed response");
    assert_failed_response_code(&payload_failure, "payload_too_large");
    assert_status_not_found_by_nonce(
        payload_fixture.caller,
        &payload_fixture.session_id,
        "select_champion_level_up",
        &payload_nonce,
    );
    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_STRONG_READ)
        .expect("strong-read barrier should run after payload-too-large failure");
    assert_status_not_found_by_nonce(
        payload_fixture.caller,
        &payload_fixture.session_id,
        "select_champion_level_up",
        &payload_nonce,
    );

    let turn_fixture = create_active_magic_session("fr-turn", 0, Vec::new(), 4);
    let turn_nonce = "nonce:failed-receipts:sync-turn-not-due".to_string();
    let turn_failure = movement_service::sync_session_turn(
        turn_fixture.caller,
        turn_fixture.session_id.clone(),
        1_000,
        turn_nonce.clone(),
    )
    .expect("turn-not-due sync should return a failed response");
    assert_failed_response_code(&turn_failure, "turn_not_due");
    assert_status_not_found_by_nonce(
        turn_fixture.caller,
        &turn_fixture.session_id,
        "sync_session_turn",
        &turn_nonce,
    );
    restore_session_turn_runtime_after_upgrade();
    assert_status_not_found_by_nonce(
        turn_fixture.caller,
        &turn_fixture.session_id,
        "sync_session_turn",
        &turn_nonce,
    );

    let pending_fixture = create_active_magic_session("fr-work", 1, Vec::new(), 4);
    system_job_repo::create_system_job(system_job_repo::SystemJobDraft {
        job_key: format!(
            "turn_resolution:{}:{}:failed-receipts",
            pending_fixture.session.id(),
            pending_fixture.session.current_turn
        ),
        job_kind: "turn_resolution".to_string(),
        session_id: pending_fixture.session.id(),
        battle_id: None,
        turn_number: Some(pending_fixture.session.current_turn),
        due_at: Timestamp::now(),
        command_id: None,
        cursor_json: None,
    })
    .expect("turn-resolution job should seed backend-work-pending guard");
    let pending_nonce = "nonce:failed-receipts:backend-work-pending".to_string();
    let pending_error = champion_magic::select_champion_level_up(
        pending_fixture.caller,
        pending_fixture.session_id.clone(),
        pending_fixture.champion.id().to_string(),
        "dirty_tactics".to_string(),
        pending_nonce.clone(),
    )
    .expect_err("backend-work-pending guard should reject before command creation");
    assert_eq!(pending_error.code, "backend_work_pending");
    assert_status_not_found_by_nonce(
        pending_fixture.caller,
        &pending_fixture.session_id,
        "select_champion_level_up",
        &pending_nonce,
    );
    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("upgrade barrier should not create backend-work-pending status");
    assert_status_not_found_by_nonce(
        pending_fixture.caller,
        &pending_fixture.session_id,
        "select_champion_level_up",
        &pending_nonce,
    );

    let gameover_fixture = create_active_magic_session("fr-over", 1, Vec::new(), 4);
    session_turn_runtime::remove_runtime(
        &gameover_fixture.session_id,
        gameover_fixture.session.current_turn,
    )
    .expect("post-gameover fixture runtime should be removable");
    let mut gameover_session = gameover_fixture.session.clone();
    gameover_session.state = "gameover".to_string();
    sessions::update_session(gameover_session).expect("gameover session should persist");
    let gameover_nonce = "nonce:failed-receipts:post-gameover".to_string();
    let gameover_error = champion_magic::select_champion_level_up(
        gameover_fixture.caller,
        gameover_fixture.session_id.clone(),
        gameover_fixture.champion.id().to_string(),
        "dirty_tactics".to_string(),
        gameover_nonce.clone(),
    )
    .expect_err("post-gameover command should reject before command creation");
    assert_eq!(gameover_error.code, "session_not_active");
    assert_status_not_found_by_nonce(
        gameover_fixture.caller,
        &gameover_fixture.session_id,
        "select_champion_level_up",
        &gameover_nonce,
    );
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn failed_command_receipts_survive_upgrade_flush_and_eviction() {
    bootstrap_service_memory();

    let stale_fixture = create_active_magic_session("fr-stale", 0, Vec::new(), 4);
    let stale_participant = sessions::load_participant(Id::<GameParticipant>::from_key(
        stale_fixture.champion.participant_id,
    ))
    .expect("stale-turn participant lookup should not fail")
    .expect("stale-turn participant should exist");
    let stale_nonce = "nonce:failed-receipts:stale-turn".to_string();
    let stale_command_type = "submit_move_intent";
    let stale_payload = r#"{"client_turn":6,"server_turn":7}"#.to_string();
    let stale_payload_hash = command_response::payload_hash(
        stale_command_type,
        &stale_participant.id().to_string(),
        &stale_nonce,
        &stale_payload,
    );
    let mut stale_command = commands_events_effects::create_game_command(
        stale_fixture.session.id(),
        "player".to_string(),
        stale_participant.id().to_string(),
        None,
        Some(stale_participant.id()),
        Some(stale_fixture.champion.id()),
        stale_fixture.session.current_turn.saturating_sub(1),
        command_response::nonce_u64(stale_command_type, &stale_nonce),
        stale_command_type.to_string(),
        stale_payload_hash,
        stale_payload,
    )
    .expect("stale-turn command row should seed");
    stale_command.status = "failed".to_string();
    stale_command.phase = "failed".to_string();
    stale_command.error_code = Some("stale_turn".to_string());
    stale_command.error_message = Some("client used an old turn number".to_string());
    stale_command.failed_at = Some(Timestamp::now());
    let stale_command = commands_events_effects::update_game_command(stale_command)
        .expect("stale-turn failed command should persist");
    let stale_status = assert_status_by_nonce(
        stale_fixture.caller,
        &stale_fixture.session_id,
        stale_command_type,
        &stale_nonce,
        &stale_command.id().to_string(),
        CommandStatus::Failed,
        Some("stale_turn"),
    );
    assert_status_by_id_matches(
        stale_fixture.caller,
        &stale_fixture.session_id,
        &stale_command.id().to_string(),
        &stale_status,
    );
    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("upgrade barrier should preserve durable stale-turn status");
    let stale_status_after_upgrade = assert_status_by_nonce(
        stale_fixture.caller,
        &stale_fixture.session_id,
        stale_command_type,
        &stale_nonce,
        &stale_command.id().to_string(),
        CommandStatus::Failed,
        Some("stale_turn"),
    );
    assert_eq!(stale_status_after_upgrade, stale_status);

    let mismatch_fixture = create_active_magic_session("fr-mismatch", 2, Vec::new(), 4);
    let mismatch_nonce = "nonce:failed-receipts:skill-nonce-mismatch".to_string();
    let selected = champion_magic::select_champion_level_up(
        mismatch_fixture.caller,
        mismatch_fixture.session_id.clone(),
        mismatch_fixture.champion.id().to_string(),
        "dirty_tactics".to_string(),
        mismatch_nonce.clone(),
    )
    .expect("first level-up command should apply");
    assert_eq!(selected.status, CommandStatus::Applied);
    let mismatch = champion_magic::select_champion_level_up(
        mismatch_fixture.caller,
        mismatch_fixture.session_id.clone(),
        mismatch_fixture.champion.id().to_string(),
        "grim_logistics".to_string(),
        mismatch_nonce.clone(),
    )
    .expect("nonce mismatch should return a failed response");
    assert_failed_response_code(&mismatch, "duplicate_nonce_payload_mismatch");
    let selected_status = assert_status_by_nonce(
        mismatch_fixture.caller,
        &mismatch_fixture.session_id,
        "select_champion_level_up",
        &mismatch_nonce,
        &selected.command_id,
        CommandStatus::Applied,
        None,
    );
    restore_session_turn_runtime_after_upgrade();
    assert_eq!(
        assert_status_by_nonce(
            mismatch_fixture.caller,
            &mismatch_fixture.session_id,
            "select_champion_level_up",
            &mismatch_nonce,
            &selected.command_id,
            CommandStatus::Applied,
            None,
        ),
        selected_status
    );
    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("upgrade barrier should flush selected command receipt");
    session_turn_runtime::remove_runtime(
        &mismatch_fixture.session_id,
        mismatch_fixture.session.current_turn,
    )
    .expect("flushed mismatch fixture runtime should be removable");
    let selected_status_after_flush = assert_status_by_nonce(
        mismatch_fixture.caller,
        &mismatch_fixture.session_id,
        "select_champion_level_up",
        &mismatch_nonce,
        &selected.command_id,
        CommandStatus::Applied,
        None,
    );
    assert_status_by_id_matches(
        mismatch_fixture.caller,
        &mismatch_fixture.session_id,
        &selected.command_id,
        &selected_status_after_flush,
    );

    let recruit_fixture = create_active_town_recruit_session("fr-recruit", 5, 4);
    let recruit_target = RecruitTarget::Champion {
        champion_id: recruit_fixture.champion.id().to_string(),
        slot_index: None,
    };
    let recruit_nonce = "nonce:failed-receipts:recruit-target".to_string();
    let recruit_failure = town_service::submit_recruit_units(
        recruit_fixture.caller,
        recruit_fixture.session_id.clone(),
        recruit_fixture.town.id().to_string(),
        recruit_fixture.unit.slug.clone(),
        4,
        recruit_target,
        recruit_nonce.clone(),
    )
    .expect("disabled recruit target should return a failed response");
    assert_failed_response_code(&recruit_failure, "champion_not_at_town");
    let recruit_status = assert_status_by_nonce(
        recruit_fixture.caller,
        &recruit_fixture.session_id,
        "submit_recruit_units",
        &recruit_nonce,
        &recruit_failure.command_id,
        CommandStatus::Failed,
        Some("champion_not_at_town"),
    );
    restore_session_turn_runtime_after_upgrade();
    assert_eq!(
        assert_status_by_nonce(
            recruit_fixture.caller,
            &recruit_fixture.session_id,
            "submit_recruit_units",
            &recruit_nonce,
            &recruit_failure.command_id,
            CommandStatus::Failed,
            Some("champion_not_at_town"),
        ),
        recruit_status
    );
    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("upgrade barrier should flush recruit failure receipt");
    let recruit_session =
        sessions::load_session(Id::<GameSession>::from_key(recruit_fixture.town.session_id))
            .expect("recruit session lookup should not fail")
            .expect("recruit session should exist");
    session_turn_runtime::remove_runtime(&recruit_fixture.session_id, recruit_session.current_turn)
        .expect("flushed recruit runtime should be removable");
    let recruit_status_after_flush = assert_status_by_nonce(
        recruit_fixture.caller,
        &recruit_fixture.session_id,
        "submit_recruit_units",
        &recruit_nonce,
        &recruit_failure.command_id,
        CommandStatus::Failed,
        Some("champion_not_at_town"),
    );
    assert_status_by_id_matches(
        recruit_fixture.caller,
        &recruit_fixture.session_id,
        &recruit_failure.command_id,
        &recruit_status_after_flush,
    );

    battle_runtime::clear_all_for_tests();
    let battle_fixture = create_runtime_battle_deadline_fixture("fr-battle");
    let mut unsupported_action = battle_fixture.input.clone();
    unsupported_action.action = "LaunchFireworks".to_string();
    let battle_nonce = "nonce:failed-receipts:battle-action-unsupported".to_string();
    let battle_failure = battle_service::submit_battle_action(
        battle_fixture.caller,
        battle_fixture.session.id().to_string(),
        unsupported_action.clone(),
        battle_nonce.clone(),
        battle_fixture.deadline_ms.saturating_sub(1),
    )
    .expect("unsupported battle action should return a failed response");
    assert_failed_response_code(&battle_failure, "battle_action_not_supported");
    let battle_status = assert_status_by_nonce(
        battle_fixture.caller,
        &battle_fixture.session.id().to_string(),
        "submit_battle_action",
        &battle_nonce,
        &battle_failure.command_id,
        CommandStatus::Failed,
        Some("battle_action_not_supported"),
    );
    restore_battle_runtime_after_upgrade();
    assert_eq!(
        assert_status_by_nonce(
            battle_fixture.caller,
            &battle_fixture.session.id().to_string(),
            "submit_battle_action",
            &battle_nonce,
            &battle_failure.command_id,
            CommandStatus::Failed,
            Some("battle_action_not_supported"),
        ),
        battle_status
    );
    let runtime = battle_runtime::with_runtime(&unsupported_action.battle_id, Clone::clone)
        .expect("unsupported-action battle runtime should restore before archive flush");
    battle_runtime::archive_runtime_events(&runtime);
    battle_runtime::archive_runtime_command_receipts(&runtime);
    assert!(
        battle_runtime::remove_runtime(&unsupported_action.battle_id).is_some(),
        "unsupported-action runtime should be removable for durable status check"
    );
    battle_runtime::flush_runtime_archives_for_barrier()
        .expect("battle runtime failure archive should flush");
    let battle_status_after_flush = assert_status_by_nonce(
        battle_fixture.caller,
        &battle_fixture.session.id().to_string(),
        "submit_battle_action",
        &battle_nonce,
        &battle_failure.command_id,
        CommandStatus::Failed,
        Some("battle_action_not_supported"),
    );
    assert_eq!(battle_status_after_flush, battle_status);
    assert_status_by_id_matches(
        battle_fixture.caller,
        &battle_fixture.session.id().to_string(),
        &battle_failure.command_id,
        &battle_status_after_flush,
    );
}

#[test]
fn battle_action_replay_after_deadline_returns_runtime_receipt_without_durable_duplicate() {
    bootstrap_service_memory();
    let fixture = create_runtime_battle_deadline_fixture("battle-deadline-replay");
    let client_nonce = command_response::nonce_u64("submit_battle_action", &fixture.nonce);

    let first = battle_service::submit_battle_action(
        fixture.caller,
        fixture.session.id().to_string(),
        fixture.input.clone(),
        fixture.nonce.clone(),
        fixture.deadline_ms.saturating_sub(1),
    )
    .expect("runtime battle action should submit before deadline");
    assert_eq!(first.status, CommandStatus::Applied);
    assert!(
        commands_events_effects::find_game_command_by_idempotency(
            fixture.session.id(),
            "player",
            &fixture.participant.id().to_string(),
            client_nonce,
        )
        .expect("durable command lookup before deadline replay should not fail")
        .is_none(),
        "runtime battle action should stay heap-owned before flush"
    );

    let late = battle_service::submit_battle_action(
        fixture.caller,
        fixture.session.id().to_string(),
        fixture.input,
        fixture.nonce.clone(),
        fixture.deadline_ms.saturating_add(60_000),
    )
    .expect("same-nonce runtime battle action replay should win before deadline timeout handoff");
    assert_eq!(late.command_id, first.command_id);
    assert_eq!(late.status, CommandStatus::Applied);
    assert_eq!(late.result, first.result);
    assert_eq!(late.events, first.events);
    assert!(
        commands_events_effects::find_game_command_by_idempotency(
            fixture.session.id(),
            "player",
            &fixture.participant.id().to_string(),
            client_nonce,
        )
        .expect("durable command lookup after deadline replay should not fail")
        .is_none(),
        "late replay must not create a duplicate durable battle command"
    );

    let status = event_service::get_command_status_by_nonce(
        fixture.caller,
        fixture.session.id().to_string(),
        "submit_battle_action".to_string(),
        fixture.nonce,
    )
    .expect("runtime battle action status should remain available after late replay");
    assert_eq!(status.command_id, first.command_id);
    assert_eq!(status.status, CommandStatus::Applied);
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn battle_archive_flush_clears_backlog_and_preserves_replay_contract() {
    bootstrap_service_memory();
    battle_runtime::clear_all_for_tests();
    let fixture = create_runtime_battle_deadline_fixture("battle-archive-flush");
    let client_nonce = command_response::nonce_u64("submit_battle_action", &fixture.nonce);

    let first = battle_service::submit_battle_action(
        fixture.caller,
        fixture.session.id().to_string(),
        fixture.input.clone(),
        fixture.nonce.clone(),
        fixture.deadline_ms.saturating_sub(1),
    )
    .expect("runtime battle action should submit before archive flush");
    assert_eq!(first.status, CommandStatus::Applied);

    let runtime = battle_runtime::with_runtime(&fixture.input.battle_id, Clone::clone)
        .expect("battle runtime should remain active before archive");
    assert_eq!(runtime.command_receipts.len(), 1);
    let archived_event_keys = runtime
        .active_events
        .iter()
        .map(|event| event.event.event_key.clone())
        .collect::<BTreeSet<_>>();
    let archived_event_types = runtime
        .active_events
        .iter()
        .map(|event| event.event.event_type.clone())
        .collect::<BTreeSet<_>>();
    assert!(
        !archived_event_keys.is_empty(),
        "runtime battle action should create archiveable events"
    );

    battle_runtime::archive_runtime_events(&runtime);
    battle_runtime::archive_runtime_command_receipts(&runtime);
    battle_runtime::archive_runtime_events(&runtime);
    battle_runtime::archive_runtime_command_receipts(&runtime);
    assert!(
        battle_runtime::remove_runtime(&fixture.input.battle_id).is_some(),
        "runtime removal should archive the battle once more without duplicates"
    );

    let pending_before = battle_runtime::runtime_archive_projection_pending_entries();
    assert_eq!(
        pending_before,
        archived_event_keys.len() + 1,
        "archive pending count should dedupe duplicate event keys and command receipts"
    );

    let flushed = battle_runtime::flush_runtime_archives_for_barrier()
        .expect("battle archive flush should succeed");
    assert_eq!(flushed, pending_before);
    assert_eq!(
        battle_runtime::runtime_archive_projection_pending_entries(),
        0
    );
    assert_eq!(
        battle_runtime::flush_runtime_archives_for_barrier()
            .expect("second battle archive flush should stay idempotent"),
        0
    );

    let durable_command = commands_events_effects::find_game_command_by_idempotency(
        fixture.session.id(),
        "player",
        &fixture.participant.id().to_string(),
        client_nonce,
    )
    .expect("durable flushed command lookup should not fail")
    .expect("flushed runtime battle command should be durable");
    assert_eq!(durable_command.id().to_string(), first.command_id);
    let result_json = durable_command
        .result_json
        .as_deref()
        .expect("flushed runtime battle command should preserve result JSON");
    assert!(result_json.contains(r#""command_kind":"submit_battle_action""#));
    assert!(result_json.contains(r#""current_round":"#));
    assert!(!result_json.contains("runtime_flushed"));

    let mut durable_event_keys = BTreeSet::new();
    for event_type in archived_event_types {
        let page =
            commands_events_effects::events_by_type(fixture.session.id(), &event_type, 100, None)
                .expect("durable event page should load");
        for event in page.items {
            if archived_event_keys.contains(&event.event_key) {
                assert!(
                    durable_event_keys.insert(event.event_key.clone()),
                    "duplicate durable event key flushed: {}",
                    event.event_key
                );
            }
        }
    }
    assert_eq!(durable_event_keys, archived_event_keys);

    let replay = battle_service::submit_battle_action(
        fixture.caller,
        fixture.session.id().to_string(),
        fixture.input,
        fixture.nonce.clone(),
        fixture.deadline_ms.saturating_sub(1),
    )
    .expect("durable replay after battle runtime eviction should return");
    assert_eq!(replay.command_id, first.command_id);
    assert_eq!(replay.status, CommandStatus::Applied);
    assert_eq!(replay.result, first.result);

    let status_by_id = event_service::get_command_status(
        fixture.caller,
        fixture.session.id().to_string(),
        first.command_id,
    )
    .expect("flushed battle command status by id should load");
    assert_eq!(status_by_id.status, CommandStatus::Applied);
    let status_by_nonce = event_service::get_command_status_by_nonce(
        fixture.caller,
        fixture.session.id().to_string(),
        "submit_battle_action".to_string(),
        fixture.nonce,
    )
    .expect("flushed battle command status by nonce should load");
    assert_eq!(status_by_nonce.command_id, replay.command_id);
    assert_eq!(status_by_nonce.status, CommandStatus::Applied);
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn battle_runtime_receipt_result_json_survives_upgrade_flush_and_eviction() {
    bootstrap_service_memory();
    battle_runtime::clear_all_for_tests();
    let fixture = create_runtime_battle_deadline_fixture("battle-receipt-parity");
    let session_id = fixture.session.id().to_string();
    let sync_nonce = "nonce:battle-receipt-parity:sync-battle".to_string();
    let action_nonce = fixture.nonce.clone();
    let end_nonce = "nonce:battle-receipt-parity:end-battle".to_string();

    let sync_first = battle_service::sync_battle(
        fixture.caller,
        session_id.clone(),
        fixture.input.battle_id.clone(),
        fixture.deadline_ms.saturating_sub(1),
        sync_nonce.clone(),
    )
    .expect("runtime sync_battle should apply");
    assert_eq!(sync_first.status, CommandStatus::Applied);
    let sync_status_before = event_service::get_command_status_by_nonce(
        fixture.caller,
        session_id.clone(),
        "sync_battle".to_string(),
        sync_nonce.clone(),
    )
    .expect("runtime sync status by nonce should load before flush");
    assert_result_json_kind(&sync_status_before, "sync_battle");

    let action_first = battle_service::submit_battle_action(
        fixture.caller,
        session_id.clone(),
        fixture.input.clone(),
        action_nonce.clone(),
        fixture.deadline_ms.saturating_sub(1),
    )
    .expect("runtime submit_battle_action should apply");
    assert_eq!(action_first.status, CommandStatus::Applied);
    let action_status_before = event_service::get_command_status_by_nonce(
        fixture.caller,
        session_id.clone(),
        "submit_battle_action".to_string(),
        action_nonce.clone(),
    )
    .expect("runtime battle action status by nonce should load before flush");
    assert_result_json_kind(&action_status_before, "submit_battle_action");

    let end_first = battle_service::end_battle_turn(
        fixture.caller,
        session_id.clone(),
        fixture.input.battle_id.clone(),
        end_nonce.clone(),
    )
    .expect("runtime end_battle_turn should apply");
    assert_eq!(end_first.status, CommandStatus::Applied);
    let end_status_before = event_service::get_command_status_by_nonce(
        fixture.caller,
        session_id.clone(),
        "end_battle_turn".to_string(),
        end_nonce.clone(),
    )
    .expect("runtime end-battle status by nonce should load before flush");
    assert_result_json_kind(&end_status_before, "end_battle_turn");

    assert_replay_response_parity(
        &battle_service::sync_battle(
            fixture.caller,
            session_id.clone(),
            fixture.input.battle_id.clone(),
            fixture.deadline_ms.saturating_sub(1),
            sync_nonce.clone(),
        )
        .expect("runtime sync replay should return before upgrade"),
        &sync_first,
    );
    assert_replay_response_parity(
        &battle_service::submit_battle_action(
            fixture.caller,
            session_id.clone(),
            fixture.input.clone(),
            action_nonce.clone(),
            fixture.deadline_ms.saturating_sub(1),
        )
        .expect("runtime battle action replay should return before upgrade"),
        &action_first,
    );
    assert_replay_response_parity(
        &battle_service::end_battle_turn(
            fixture.caller,
            session_id.clone(),
            fixture.input.battle_id.clone(),
            end_nonce.clone(),
        )
        .expect("runtime end-battle replay should return before upgrade"),
        &end_first,
    );

    battle_runtime::persist_snapshot_for_upgrade()
        .expect("active battle runtime snapshot should persist for upgrade");
    battle_runtime::clear_all_for_tests();
    battle_runtime::restore_snapshot_after_upgrade()
        .expect("active battle runtime snapshot should restore after upgrade");
    assert_replay_response_parity(
        &battle_service::sync_battle(
            fixture.caller,
            session_id.clone(),
            fixture.input.battle_id.clone(),
            fixture.deadline_ms.saturating_sub(1),
            sync_nonce.clone(),
        )
        .expect("runtime sync replay should return after upgrade restore"),
        &sync_first,
    );
    assert_eq!(
        event_service::get_command_status_by_nonce(
            fixture.caller,
            session_id.clone(),
            "sync_battle".to_string(),
            sync_nonce.clone(),
        )
        .expect("runtime sync status should load after upgrade restore"),
        sync_status_before
    );
    assert_replay_response_parity(
        &battle_service::submit_battle_action(
            fixture.caller,
            session_id.clone(),
            fixture.input.clone(),
            action_nonce.clone(),
            fixture.deadline_ms.saturating_sub(1),
        )
        .expect("runtime battle action replay should return after upgrade restore"),
        &action_first,
    );
    assert_eq!(
        event_service::get_command_status_by_nonce(
            fixture.caller,
            session_id.clone(),
            "submit_battle_action".to_string(),
            action_nonce.clone(),
        )
        .expect("runtime battle action status should load after upgrade restore"),
        action_status_before
    );
    assert_replay_response_parity(
        &battle_service::end_battle_turn(
            fixture.caller,
            session_id.clone(),
            fixture.input.battle_id.clone(),
            end_nonce.clone(),
        )
        .expect("runtime end-battle replay should return after upgrade restore"),
        &end_first,
    );
    assert_eq!(
        event_service::get_command_status_by_nonce(
            fixture.caller,
            session_id.clone(),
            "end_battle_turn".to_string(),
            end_nonce.clone(),
        )
        .expect("runtime end-battle status should load after upgrade restore"),
        end_status_before
    );

    let runtime = battle_runtime::with_runtime(&fixture.input.battle_id, Clone::clone)
        .expect("restored battle runtime should be active before archive flush");
    assert_eq!(runtime.command_receipts.len(), 3);
    battle_runtime::archive_runtime_events(&runtime);
    battle_runtime::archive_runtime_command_receipts(&runtime);
    assert!(
        battle_runtime::remove_runtime(&fixture.input.battle_id).is_some(),
        "runtime should be removable for durable replay parity"
    );
    assert!(
        battle_runtime::runtime_archive_projection_pending_entries() >= 3,
        "archive should contain at least the three runtime battle receipts"
    );
    battle_runtime::flush_runtime_archives_for_barrier()
        .expect("battle runtime receipt archive should flush");
    assert_eq!(
        battle_runtime::runtime_archive_projection_pending_entries(),
        0
    );

    assert_replay_response_parity(
        &battle_service::sync_battle(
            fixture.caller,
            session_id.clone(),
            fixture.input.battle_id.clone(),
            fixture.deadline_ms.saturating_sub(1),
            sync_nonce.clone(),
        )
        .expect("durable sync replay should return after runtime eviction"),
        &sync_first,
    );
    assert_status_parity_after_flush(
        fixture.caller,
        &session_id,
        "sync_battle",
        &sync_nonce,
        &sync_first.command_id,
        &sync_status_before,
    );

    assert_replay_response_parity(
        &battle_service::submit_battle_action(
            fixture.caller,
            session_id.clone(),
            fixture.input.clone(),
            action_nonce.clone(),
            fixture.deadline_ms.saturating_sub(1),
        )
        .expect("durable battle action replay should return after runtime eviction"),
        &action_first,
    );
    assert_status_parity_after_flush(
        fixture.caller,
        &session_id,
        "submit_battle_action",
        &action_nonce,
        &action_first.command_id,
        &action_status_before,
    );

    assert_replay_response_parity(
        &battle_service::end_battle_turn(
            fixture.caller,
            session_id.clone(),
            fixture.input.battle_id,
            end_nonce.clone(),
        )
        .expect("durable end-battle replay should return after runtime eviction"),
        &end_first,
    );
    assert_status_parity_after_flush(
        fixture.caller,
        &session_id,
        "end_battle_turn",
        &end_nonce,
        &end_first.command_id,
        &end_status_before,
    );
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn battle_sync_timeout_backlog_drains_inline_with_stable_replay() {
    bootstrap_service_memory();
    battle_runtime::clear_all_for_tests();
    let fixture = create_runtime_battle_deadline_fixture("battle-sync-backlog");
    let session_id = fixture.session.id().to_string();
    let sync_nonce = "nonce:battle-sync-backlog:sync-battle".to_string();
    let far_past_deadlines = fixture.deadline_ms.saturating_add(700_000);

    let first = battle_service::sync_battle(
        fixture.caller,
        session_id.clone(),
        fixture.input.battle_id.clone(),
        far_past_deadlines,
        sync_nonce.clone(),
    )
    .expect("runtime sync_battle should process the timeout backlog inline");
    assert_eq!(first.status, CommandStatus::Applied);
    let first_outcome = battle_sync_outcome(&first);
    assert!(first_outcome.timeout_actions_applied >= 1);
    assert!(
        !first_outcome.battle_sync_incomplete,
        "sync_battle should drain due timeouts in one call"
    );
    assert_eq!(
        first
            .events
            .iter()
            .filter(|event| event.event_type == "battle_timeout_auto_defend")
            .count(),
        usize::try_from(first_outcome.timeout_actions_applied)
            .expect("timeout action count should fit usize")
    );

    let status_by_nonce = event_service::get_command_status_by_nonce(
        fixture.caller,
        session_id.clone(),
        "sync_battle".to_string(),
        sync_nonce.clone(),
    )
    .expect("runtime sync status by nonce should load");
    assert_eq!(status_by_nonce.command_id, first.command_id);
    assert_eq!(status_by_nonce.status, CommandStatus::Applied);
    let result_json = status_by_nonce
        .result_json
        .as_deref()
        .expect("runtime sync should expose result JSON");
    assert!(result_json.contains(r#""command_kind":"sync_battle""#));
    assert!(result_json.contains(r#""battle_sync_incomplete":false"#));
    let status_by_id = event_service::get_command_status(
        fixture.caller,
        session_id.clone(),
        first.command_id.clone(),
    )
    .expect("runtime sync status by id should load");
    assert_eq!(status_by_id, status_by_nonce);

    let replay = battle_service::sync_battle(
        fixture.caller,
        session_id.clone(),
        fixture.input.battle_id.clone(),
        far_past_deadlines,
        sync_nonce.clone(),
    )
    .expect("same-nonce sync replay should return the original receipt");
    assert_replay_response_parity(&replay, &first);
    assert_eq!(replay.events, first.events);

    let next = battle_service::sync_battle(
        fixture.caller,
        session_id,
        fixture.input.battle_id,
        far_past_deadlines,
        "nonce:battle-sync-backlog:sync-battle-next".to_string(),
    )
    .expect("fresh sync after inline response should find no timeout backlog");
    assert_eq!(next.status, CommandStatus::Applied);
    let next_outcome = battle_sync_outcome(&next);
    assert_eq!(next_outcome.timeout_actions_applied, 0);
    assert!(!next_outcome.battle_sync_incomplete);
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn active_battle_runtime_deadlines_survive_upgrade_and_inline_progress() {
    bootstrap_service_memory();
    battle_runtime::clear_all_for_tests();

    let action_fixture = create_runtime_battle_deadline_fixture("battle-upgrade-action-timeout");
    let action_session_id = action_fixture.session.id().to_string();
    let action = battle_service::submit_battle_action(
        action_fixture.caller,
        action_session_id.clone(),
        action_fixture.input.clone(),
        action_fixture.nonce.clone(),
        action_fixture.deadline_ms.saturating_sub(1),
    )
    .expect("runtime player battle action should apply before upgrade");
    assert_eq!(action.status, CommandStatus::Applied);
    let action_status_before = event_service::get_command_status_by_nonce(
        action_fixture.caller,
        action_session_id.clone(),
        "submit_battle_action".to_string(),
        action_fixture.nonce.clone(),
    )
    .expect("runtime action status should load before upgrade");
    let (timeout_deadline, _timeout_key) = runtime_timer_state(&action_fixture.input.battle_id)
        .expect("runtime deadline state should exist after player action");
    assert!(timeout_deadline.is_some());

    simulate_battle_runtime_upgrade_restore();
    assert_replay_response_parity(
        &battle_service::submit_battle_action(
            action_fixture.caller,
            action_session_id.clone(),
            action_fixture.input.clone(),
            action_fixture.nonce.clone(),
            action_fixture.deadline_ms.saturating_sub(1),
        )
        .expect("player action replay should survive battle runtime restore"),
        &action,
    );
    assert_eq!(
        event_service::get_command_status_by_nonce(
            action_fixture.caller,
            action_session_id.clone(),
            "submit_battle_action".to_string(),
            action_fixture.nonce.clone(),
        )
        .expect("action status should survive battle runtime restore"),
        action_status_before
    );
    assert_eq!(
        runtime_timer_state(&action_fixture.input.battle_id)
            .expect("restored runtime deadline state should exist")
            .0,
        timeout_deadline
    );
    assert_eq!(
        event_service::get_command_status_by_nonce(
            action_fixture.caller,
            action_session_id,
            "submit_battle_action".to_string(),
            action_fixture.nonce,
        )
        .expect("action status should remain after timeout timer progress"),
        action_status_before
    );

    let round_fixture = create_runtime_battle_deadline_fixture("battle-upgrade-round-wakeup");
    let round_session_id = round_fixture.session.id().to_string();
    let end_nonce = "nonce:battle-upgrade-round-wakeup:end-battle".to_string();
    let end_turn = battle_service::end_battle_turn(
        round_fixture.caller,
        round_session_id.clone(),
        round_fixture.input.battle_id.clone(),
        end_nonce.clone(),
    )
    .expect("runtime end_battle_turn should apply before upgrade");
    assert_eq!(end_turn.status, CommandStatus::Applied);
    let end_status_before = event_service::get_command_status_by_nonce(
        round_fixture.caller,
        round_session_id.clone(),
        "end_battle_turn".to_string(),
        end_nonce.clone(),
    )
    .expect("runtime end-battle status should load before upgrade");
    let (_, _, round_key_before) = runtime_round_state(&round_fixture.input.battle_id)
        .expect("runtime round state should exist after end_battle_turn");
    assert_eq!(round_key_before, None);
    assert_event_type_visible(
        round_fixture.caller,
        &round_session_id,
        "battle_round_auto_defend",
    );

    simulate_battle_runtime_upgrade_restore();
    assert_replay_response_parity(
        &battle_service::end_battle_turn(
            round_fixture.caller,
            round_session_id.clone(),
            round_fixture.input.battle_id.clone(),
            end_nonce.clone(),
        )
        .expect("end_battle_turn replay should survive battle runtime restore"),
        &end_turn,
    );
    assert_eq!(
        event_service::get_command_status_by_nonce(
            round_fixture.caller,
            round_session_id.clone(),
            "end_battle_turn".to_string(),
            end_nonce,
        )
        .expect("end-battle status should survive battle runtime restore"),
        end_status_before
    );
    let (current_round, _runtime_event_count, round_job_key_after) =
        runtime_round_state(&round_fixture.input.battle_id)
            .expect("runtime round state should exist after inline progress");
    assert!(
        current_round > 1 || round_job_key_after.is_none(),
        "round readiness should advance inline or clear stale wakeup state"
    );
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn end_turn_all_ready_resolves_turn_and_maintenance_inline_after_upgrade_flush() {
    bootstrap_service_memory();
    let fixture = create_active_scenario_progress_session("inline-turn", 1, 1, 30, 0);
    let session_id = fixture.session.id().to_string();
    let turn_number = fixture.session.current_turn;
    let turn_job_key = format!("turn_resolution:{}:{turn_number}", fixture.session.id());

    let ready_one = movement_service::end_turn(
        fixture.caller_one,
        session_id.clone(),
        "nonce:inline-turn:end-turn:one".to_string(),
    )
    .expect("participant one should end turn");
    assert_eq!(ready_one.status, CommandStatus::Applied);

    restore_session_turn_runtime_after_upgrade();
    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("projection flush should tolerate a one-ready turn before inline resolution");

    let ready_two = movement_service::end_turn(
        fixture.caller_two,
        session_id.clone(),
        "nonce:inline-turn:end-turn:two".to_string(),
    )
    .expect("participant two should end turn and resolve the turn inline");
    assert_eq!(ready_two.status, CommandStatus::Applied);

    assert!(
        system_job_repo::find_system_job_by_key(&turn_job_key)
            .expect("turn-resolution job lookup should not fail")
            .is_none(),
        "all-ready end_turn must not schedule a turn-resolution job"
    );
    system_job_service::repair_and_schedule_after_install_or_upgrade()
        .expect("system job repair no-op should not fail");
    let processed = system_job_service::run_due_jobs_until_idle(8)
        .expect("due-job dispatcher should run without due inline gameplay jobs");
    assert_eq!(processed, 0);

    let advanced_session = sessions::load_session(fixture.session.id())
        .expect("advanced session lookup should not fail")
        .expect("advanced session should exist");
    assert_eq!(advanced_session.current_turn, turn_number + 1);
    assert_eq!(advanced_session.state, "active");

    let next_turn_deadline_key = format!(
        "turn_deadline:{}:{}",
        fixture.session.id(),
        advanced_session.current_turn
    );
    assert!(
        system_job_repo::find_system_job_by_key(&next_turn_deadline_key)
            .expect("next-turn deadline lookup should not fail")
            .is_none(),
        "inline turn advance must not schedule a next-turn deadline job"
    );

    for job_kind in ["scenario_objectives", "world_events", "advanced_victory"] {
        let job_key = format!(
            "{job_kind}:{}:{}",
            fixture.session.id(),
            advanced_session.current_turn
        );
        assert!(
            system_job_repo::find_system_job_by_key(&job_key)
                .expect("scenario maintenance job lookup should not fail")
                .is_none(),
            "scenario maintenance should run inline without system job {job_key}"
        );
    }

    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_STRONG_READ)
        .expect("final projection flush after inline turn execution should succeed");
    let projection = session_turn_runtime::projection_diagnostic_snapshot();
    for kernel in projection
        .kernels
        .iter()
        .filter(|kernel| kernel.session_id == session_id)
    {
        assert_eq!(kernel.dirty_queue_len, 0);
        assert_eq!(kernel.lag_generations, 0);
        assert_eq!(kernel.pending_entries, 0);
    }
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn timer_race_interleavings_reject_late_commands_and_deduplicate_timeout_callbacks() {
    bootstrap_service_memory();

    let movement_fixture = create_active_town_recruit_session("timer-race-movement", 4, 4);
    let movement_session = sessions::load_session(Id::<GameSession>::from_key(
        Ulid::from_str(&movement_fixture.session_id).expect("movement session id should be ulid"),
    ))
    .expect("movement session lookup should not fail")
    .expect("movement session should exist");
    map_visibility_occupancy::create_map_chunk(
        movement_session.id(),
        0,
        0,
        16,
        16,
        vec![0; 16 * 16],
        vec![1; 16 * 16],
        vec![0; 16 * 16],
    )
    .expect("movement race test map chunk should seed");
    let turn_number = movement_session.current_turn;
    let turn_job_key = format!("turn_resolution:{}:{turn_number}", movement_session.id());
    system_job_repo::create_system_job(system_job_repo::SystemJobDraft {
        job_key: turn_job_key.clone(),
        job_kind: "turn_resolution".to_string(),
        session_id: movement_session.id(),
        battle_id: None,
        turn_number: Some(turn_number),
        due_at: Timestamp::now(),
        command_id: None,
        cursor_json: None,
    })
    .expect("turn-resolution race job should seed");

    let blocked_nonce = "nonce:timer-race-movement:late-submit".to_string();
    let blocked = movement_service::submit_move_intent(
        movement_fixture.caller,
        movement_fixture.session_id.clone(),
        movement_fixture.champion.id().to_string(),
        vec![MoveCoord::new(
            movement_fixture.champion.x + 1,
            movement_fixture.champion.y,
        )],
        blocked_nonce.clone(),
        1_000,
    )
    .expect_err("accepted turn-resolution timer should block same-turn movement");
    assert_eq!(blocked.code, "backend_work_pending");
    assert_status_not_found_by_nonce(
        movement_fixture.caller,
        &movement_fixture.session_id,
        "submit_move_intent",
        &blocked_nonce,
    );

    let processed = system_job_service::run_due_job_by_key(&turn_job_key)
        .expect("turn-resolution race job should dispatch");
    assert_eq!(processed, 1);
    let completed_job = system_job_repo::find_system_job_by_key(&turn_job_key)
        .expect("completed race job lookup should not fail")
        .expect("completed race job should exist");
    assert_eq!(completed_job.status, system_job_repo::STATUS_COMPLETED);
    let advanced = sessions::load_session(movement_session.id())
        .expect("advanced movement session lookup should not fail")
        .expect("advanced movement session should exist");
    assert_eq!(advanced.current_turn, turn_number + 1);

    battle_runtime::clear_all_for_tests();
    let battle_fixture = create_runtime_battle_deadline_fixture("tr-battle");
    let battle_session_id = battle_fixture.session.id().to_string();
    let far_past_deadline = battle_fixture.deadline_ms.saturating_add(700_000);
    let synced = battle_service::sync_battle(
        battle_fixture.caller,
        battle_session_id.clone(),
        battle_fixture.input.battle_id.clone(),
        far_past_deadline,
        "nonce:timer-race-battle-sync-timeout:sync".to_string(),
    )
    .expect("sync_battle should process an expired runtime timeout");
    assert_eq!(synced.status, CommandStatus::Applied);
    let timeout_events_after_sync = public_event_type_count(
        battle_fixture.caller,
        &battle_session_id,
        "battle_timeout_auto_defend",
    );
    assert_eq!(timeout_events_after_sync, 1);

    battle_service::process_runtime_battle_timeout_timer_for_tests(
        &battle_session_id,
        &battle_fixture.input.battle_id,
        battle_fixture
            .deadline_ms
            .try_into()
            .expect("battle deadline should fit i64"),
    )
    .expect("stale timeout timer callback should be a no-op after sync_battle");
    assert_eq!(
        public_event_type_count(
            battle_fixture.caller,
            &battle_session_id,
            "battle_timeout_auto_defend",
        ),
        timeout_events_after_sync,
        "stale timeout callback should not duplicate the timeout event already emitted by sync_battle"
    );
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn resolved_battle_finalization_flushes_runtime_and_durable_survivors() {
    bootstrap_service_memory();
    battle_runtime::clear_all_for_tests();
    let fixture = create_resolved_town_runtime_battle_fixture("battle-finalization");

    let finalized = battle_service::sync_battle(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.battle_id.clone(),
        77_000,
        "nonce:battle-finalization:sync-battle".to_string(),
    )
    .expect("resolved runtime battle sync should finalize");
    assert_eq!(finalized.status, CommandStatus::Applied);
    assert!(
        finalized
            .changed_subjects
            .iter()
            .any(|subject| subject.subject_kind == "battle" && subject.operation == "aftermath")
    );

    assert!(
        !battle_runtime::contains_runtime(&fixture.battle_id),
        "resolved battle runtime should be removed after finalization"
    );
    assert_eq!(
        battle_runtime::runtime_archive_projection_pending_entries(),
        0,
        "runtime eviction barrier should leave no archived battle backlog"
    );

    let battle = battles::load_battle(fixture.battle)
        .expect("final battle row lookup should not fail")
        .expect("final battle row should exist");
    assert_eq!(battle.state, "resolved");
    assert_eq!(
        battle.winner_participant_id,
        Some(fixture.participant_id.key())
    );

    let attacker_stack = battles::load_battle_stack(fixture.attacker_stack)
        .expect("attacker stack lookup should not fail")
        .expect("attacker stack should persist");
    assert_eq!(attacker_stack.quantity, fixture.attacker_survivor_quantity);
    assert_eq!(attacker_stack.front_hp, fixture.attacker_survivor_front_hp);
    assert_eq!(attacker_stack.status, "active");
    let defender_stack = battles::load_battle_stack(fixture.defender_stack)
        .expect("defender stack lookup should not fail")
        .expect("defender stack should persist");
    assert_eq!(defender_stack.quantity, 0);
    assert_eq!(defender_stack.status, "defeated");

    let view = battle_service::get_battle_state(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.battle_id.clone(),
        77_000,
    )
    .expect("durable-only resolved battle view should load after runtime eviction");
    assert_eq!(view.state, "resolved");
    assert!(view.legal_actions_for_caller.is_empty());
    let view_attacker = view
        .stacks
        .iter()
        .find(|stack| stack.battle_stack_id == fixture.attacker_stack.to_string())
        .expect("resolved durable view should include attacker stack");
    assert_eq!(view_attacker.quantity, fixture.attacker_survivor_quantity);
    assert_eq!(view_attacker.front_hp, fixture.attacker_survivor_front_hp);
    let view_defender = view
        .stacks
        .iter()
        .find(|stack| stack.battle_stack_id == fixture.defender_stack.to_string())
        .expect("resolved durable view should include defender stack");
    assert_eq!(view_defender.status, "defeated");

    let town = towns::load_town(fixture.town.id())
        .expect("town lookup after finalization should not fail")
        .expect("town should exist after finalization");
    assert_eq!(
        town.owner_participant_id,
        Some(fixture.participant_id.key())
    );
    let garrison = towns::list_town_garrison(town.id(), domm_game::MAX_LIST_LIMIT)
        .expect("town garrison should load after finalization");
    assert_eq!(garrison.len(), 1);
    assert_eq!(garrison[0].unit_slug, fixture.unit.slug);
    assert_eq!(garrison[0].quantity, fixture.attacker_survivor_quantity);
    assert_eq!(garrison[0].front_hp, fixture.attacker_survivor_front_hp);

    let champion = champions_artifacts::load_champion(fixture.champion.id())
        .expect("champion lookup after finalization should not fail")
        .expect("champion should exist after finalization");
    assert_eq!(champion.status, "active");
    assert_eq!((champion.x, champion.y), (town.x, town.y));
    assert_eq!(champion.in_battle_id, None);

    let events = event_service::get_events_after(
        fixture.caller,
        fixture.session_id,
        "public".to_string(),
        0,
        100,
    )
    .expect("final event feed should load");
    for expected in ["town_captured", "battle_aftermath_applied"] {
        assert!(
            events
                .events
                .iter()
                .any(|event| event.event_type == expected),
            "missing final event type {expected}"
        );
    }
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
struct ResolvedTownRuntimeBattleFixture {
    caller: Principal,
    session_id: String,
    participant_id: Id<GameParticipant>,
    town: Town,
    champion: Champion,
    unit: UnitDefinition,
    battle: Id<Battle>,
    battle_id: String,
    attacker_stack: Id<BattleStack>,
    defender_stack: Id<BattleStack>,
    attacker_survivor_quantity: u32,
    attacker_survivor_front_hp: u16,
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn create_resolved_town_runtime_battle_fixture(prefix: &str) -> ResolvedTownRuntimeBattleFixture {
    let fixture = create_active_town_recruit_session(prefix, 1, 1);
    let mut session = sessions::load_session(Id::<GameSession>::from_key(
        Ulid::from_str(&fixture.session_id).expect("fixture session id should parse"),
    ))
    .expect("session lookup should not fail")
    .expect("session should exist");
    let participant_id = Id::<GameParticipant>::from_key(fixture.champion.participant_id);
    let seed_command = Id::<GameCommand>::from_key(Ulid::generate());
    let battle = battles::create_battle(
        session.id(),
        "active".to_string(),
        "town".to_string(),
        Some(fixture.champion.id()),
        None,
        Some(fixture.town.id()),
        None,
        domm_game::BATTLE_SIDE_ATTACKER.to_string(),
        domm_game::BATTLE_GRID_WIDTH,
        domm_game::BATTLE_GRID_HEIGHT,
        domm_game::BATTLE_MAX_ROUNDS,
        7331,
        session.current_turn,
        None,
        seed_command,
    )
    .expect("town battle row should seed");
    let attacker_stack = battles::create_battle_stack(
        battle.id(),
        fixture.unit.id(),
        Some(participant_id),
        domm_game::BATTLE_SIDE_ATTACKER.to_string(),
        0,
        "champion_army".to_string(),
        None,
        0,
        8,
        5,
        2,
        4,
        12,
        5,
        9,
        false,
        false,
        9,
        12,
        0,
        2,
        4,
        seed_command,
    )
    .expect("attacker battle stack should seed");
    let defender_stack = battles::create_battle_stack(
        battle.id(),
        fixture.unit.id(),
        None,
        domm_game::BATTLE_SIDE_DEFENDER.to_string(),
        0,
        "town_garrison".to_string(),
        None,
        0,
        4,
        4,
        1,
        2,
        9,
        3,
        4,
        false,
        false,
        2,
        9,
        0,
        9,
        4,
        seed_command,
    )
    .expect("defender battle stack should seed");
    let attacker_occupancy = battles::create_battle_occupancy(
        battle.id(),
        attacker_stack.id(),
        attacker_stack.battle_x,
        attacker_stack.battle_y,
        seed_command,
    )
    .expect("attacker battle occupancy should seed");
    battles::create_battle_occupancy(
        battle.id(),
        defender_stack.id(),
        defender_stack.battle_x,
        defender_stack.battle_y,
        seed_command,
    )
    .expect("defender battle occupancy should seed");

    let attacker_survivor_quantity = 6;
    let attacker_survivor_front_hp = 7;
    let battle_id = battle.id().to_string();
    let state = domm_game::BattleState {
        session_seed: session.seed.to_string(),
        battles: vec![domm_game::BattleRecord {
            battle_id: battle_id.clone(),
            session_id: session.id().to_string(),
            state: "resolved".to_string(),
            battle_type: "town".to_string(),
            attacker_champion_id: Some(fixture.champion.id().to_string()),
            defender_champion_id: None,
            defender_town_id: Some(fixture.town.id().to_string()),
            defender_neutral_army_id: None,
            current_round: 3,
            active_side: domm_game::BATTLE_SIDE_ATTACKER.to_string(),
            active_stack_id: None,
            grid_width: domm_game::BATTLE_GRID_WIDTH,
            grid_height: domm_game::BATTLE_GRID_HEIGHT,
            max_rounds: domm_game::BATTLE_MAX_ROUNDS,
            turn_seed: 7331,
            winner_participant_id: Some(participant_id.to_string()),
            created_turn: session.current_turn,
            action_deadline_at: None,
            resolved_at: Some(77_000),
            cleanup_after_turn: session.current_turn.saturating_add(1),
            last_command_id: None,
        }],
        stacks: vec![
            battle_stack_record_from_row(
                &attacker_stack,
                attacker_survivor_quantity,
                attacker_survivor_front_hp,
                "active",
            ),
            battle_stack_record_from_row(&defender_stack, 0, 0, "defeated"),
        ],
        obstacles: Vec::new(),
        occupancy: vec![domm_game::BattleOccupancyRecord {
            battle_occupancy_id: attacker_occupancy.id().to_string(),
            battle_id: battle_id.clone(),
            battle_stack_id: attacker_stack.id().to_string(),
            battle_x: attacker_stack.battle_x,
            battle_y: attacker_stack.battle_y,
            last_command_id: None,
        }],
        commands: Vec::new(),
        events: Vec::new(),
    };
    let runtime =
        battle_runtime::build_runtime_from_state(&session, state).expect("runtime should build");
    battle_runtime::insert_runtime(runtime);
    session = sessions::load_session(session.id())
        .expect("session reload should not fail")
        .expect("session should still exist");

    ResolvedTownRuntimeBattleFixture {
        caller: fixture.caller,
        session_id: session.id().to_string(),
        participant_id,
        town: fixture.town,
        champion: fixture.champion,
        unit: fixture.unit,
        battle: battle.id(),
        battle_id,
        attacker_stack: attacker_stack.id(),
        defender_stack: defender_stack.id(),
        attacker_survivor_quantity,
        attacker_survivor_front_hp,
    }
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn battle_stack_record_from_row(
    stack: &BattleStack,
    quantity: u32,
    front_hp: u16,
    status: &str,
) -> domm_game::BattleStackRecord {
    domm_game::BattleStackRecord {
        battle_stack_id: stack.id().to_string(),
        battle_id: Id::<Battle>::from_key(stack.battle_id).to_string(),
        unit_id: Id::<UnitDefinition>::from_key(stack.unit_id).to_string(),
        owner_participant_id: stack
            .owner_participant_id
            .map(|id| Id::<GameParticipant>::from_key(id).to_string()),
        side: stack.side.clone(),
        slot_index: stack.slot_index,
        origin_kind: stack.origin_kind.clone(),
        origin_stack_id_text: stack.origin_stack_id_text.clone(),
        origin_slot_index: stack.origin_slot_index,
        champion_might: 0,
        champion_guard: 0,
        attack: stack.attack,
        defense: stack.defense,
        damage_min: stack.damage_min,
        damage_max: stack.damage_max,
        max_hp: stack.max_hp,
        speed: stack.speed,
        initiative: stack.initiative,
        ranged: stack.ranged,
        flying: stack.flying,
        quantity,
        front_hp,
        shots_remaining: stack.shots_remaining,
        battle_x: stack.battle_x,
        battle_y: stack.battle_y,
        readiness: stack.readiness,
        acted_round: stack.acted_round,
        retaliated_round: stack.retaliated_round,
        defended_round: stack.defended_round,
        waited_round: stack.waited_round,
        cast_round: stack.cast_round,
        status: status.to_string(),
        last_command_id: None,
        status_keys: stack.status_keys.clone(),
    }
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn simulate_battle_runtime_upgrade_restore() {
    battle_runtime::persist_snapshot_for_upgrade()
        .expect("battle runtime snapshot should persist for upgrade");
    battle_runtime::clear_all_for_tests();
    battle_runtime::restore_snapshot_after_upgrade()
        .expect("battle runtime snapshot should restore after upgrade");
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn runtime_timer_state(battle_id: &str) -> Option<(Option<u64>, Option<String>)> {
    battle_runtime::with_runtime(battle_id, |runtime| {
        (
            runtime.deadline.action_deadline_at_ms,
            runtime.deadline.timeout_job_key.clone(),
        )
    })
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn runtime_round_state(battle_id: &str) -> Option<(u16, usize, Option<String>)> {
    battle_runtime::with_runtime(battle_id, |runtime| {
        let battle = runtime
            .state
            .battle(battle_id)
            .expect("runtime battle should exist");
        (
            battle.current_round,
            runtime.active_events.len(),
            runtime.deadline.round_job_key.clone(),
        )
    })
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn assert_event_type_visible(caller: Principal, session_id: &str, event_type: &str) {
    let events = event_service::get_events_after(
        caller,
        session_id.to_string(),
        "public".to_string(),
        0,
        100,
    )
    .expect("runtime event feed should load");
    assert!(
        events
            .events
            .iter()
            .any(|event| event.event_type == event_type),
        "expected public event type {event_type}, got {:?}",
        events
            .events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>()
    );
}

fn assert_public_event_sequences_unique(caller: Principal, session_id: &str) {
    let events = event_service::get_events_after(
        caller,
        session_id.to_string(),
        "public".to_string(),
        0,
        200,
    )
    .expect("public event feed should load");
    let mut seen = BTreeSet::new();
    for event in events.events {
        assert!(
            seen.insert(event.event_seq),
            "duplicate public event sequence {} in session {}",
            event.event_seq,
            session_id
        );
    }
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn public_event_type_count(caller: Principal, session_id: &str, event_type: &str) -> usize {
    event_service::get_events_after(caller, session_id.to_string(), "public".to_string(), 0, 100)
        .expect("public event feed should load")
        .events
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .count()
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn battle_sync_outcome(response: &CommandResponse) -> &domm_game::BattleSyncOutcome {
    match &response.result {
        domm_game::CommandResult::BattleSync(outcome) => outcome,
        other => panic!("expected battle sync outcome, got {other:?}"),
    }
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn assert_replay_response_parity(actual: &CommandResponse, expected: &CommandResponse) {
    assert_eq!(actual.command_id, expected.command_id);
    assert_eq!(actual.command_type, expected.command_type);
    assert_eq!(actual.status, expected.status);
    assert_eq!(actual.phase, expected.phase);
    assert_eq!(actual.retryable, expected.retryable);
    assert_eq!(actual.result, expected.result);
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn assert_result_json_kind(status: &domm_game::CommandStatusView, command_kind: &str) {
    let result_json = status
        .result_json
        .as_deref()
        .expect("runtime battle status should expose result JSON");
    assert!(
        result_json.contains(&format!(r#""command_kind":"{command_kind}""#)),
        "unexpected result_json for {command_kind}: {result_json}"
    );
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
fn assert_status_parity_after_flush(
    caller: Principal,
    session_id: &str,
    command_type: &str,
    client_nonce: &str,
    command_id: &str,
    expected: &domm_game::CommandStatusView,
) {
    let by_nonce = event_service::get_command_status_by_nonce(
        caller,
        session_id.to_string(),
        command_type.to_string(),
        client_nonce.to_string(),
    )
    .expect("durable battle status by nonce should load after flush");
    assert_eq!(&by_nonce, expected);

    let by_id =
        event_service::get_command_status(caller, session_id.to_string(), command_id.to_string())
            .expect("durable battle status by id should load after flush");
    assert_eq!(by_id, by_nonce);
}

#[test]
fn scenario_progress_quest_edges_redaction_and_flush_are_stable() {
    bootstrap_service_memory();
    let fixture = create_active_scenario_progress_session("scenario-quest-edges", 1, 1, 30, 0);

    let initial = scenario_service::preview_quest(
        fixture.caller_one,
        fixture.session_id.clone(),
        OPENING_QUEST_KEY.to_string(),
    )
    .expect("opening quest preview should load");
    assert!(initial.can_accept);
    assert!(!initial.can_claim);
    assert_eq!(initial.quest.progress_value, 0);
    let redacted = domm_game::redact_quest_for_viewer(
        initial.quest.clone(),
        &fixture.participant_two.id().to_string(),
    );
    assert!(redacted.redacted);
    assert_eq!(redacted.progress_value, 0);
    assert_eq!(redacted.reward_gold, None);

    let early_claim_nonce = "nonce:scenario:quest:early-claim".to_string();
    let early_claim = scenario_service::claim_quest_reward(
        fixture.caller_one,
        fixture.session_id.clone(),
        OPENING_QUEST_KEY.to_string(),
        early_claim_nonce.clone(),
    )
    .expect("claim before accept should return a command response");
    assert_failed_response_code(&early_claim, "quest_not_accepted");
    let early_status = event_service::get_command_status_by_nonce(
        fixture.caller_one,
        fixture.session_id.clone(),
        "claim_quest_reward".to_string(),
        early_claim_nonce,
    )
    .expect("failed early claim status should be visible by nonce");
    assert_eq!(early_status.status, CommandStatus::Failed);

    let accepted = scenario_service::accept_quest(
        fixture.caller_one,
        fixture.session_id.clone(),
        OPENING_QUEST_KEY.to_string(),
        "nonce:scenario:quest:accept".to_string(),
    )
    .expect("quest accept should return");
    assert_eq!(accepted.status, CommandStatus::Applied);

    let incomplete_claim = scenario_service::claim_quest_reward(
        fixture.caller_one,
        fixture.session_id.clone(),
        OPENING_QUEST_KEY.to_string(),
        "nonce:scenario:quest:incomplete-claim".to_string(),
    )
    .expect("claim before completion should return a command response");
    assert_failed_response_code(&incomplete_claim, "quest_incomplete");

    complete_runtime_opening_quest(&fixture);
    let claimed_nonce = "nonce:scenario:quest:claim".to_string();
    let claimed = scenario_service::claim_quest_reward(
        fixture.caller_one,
        fixture.session_id.clone(),
        OPENING_QUEST_KEY.to_string(),
        claimed_nonce.clone(),
    )
    .expect("completed quest claim should return");
    assert_eq!(claimed.status, CommandStatus::Applied);
    let claimed_replay = scenario_service::claim_quest_reward(
        fixture.caller_one,
        fixture.session_id.clone(),
        OPENING_QUEST_KEY.to_string(),
        claimed_nonce.clone(),
    )
    .expect("completed quest claim replay should return");
    assert_eq!(claimed_replay.command_id, claimed.command_id);

    let runtime_participant = session_turn_runtime::participant_snapshot(
        &fixture.session_id,
        &fixture.participant_one.id().to_string(),
    )
    .expect("quest reward should update runtime participant");
    assert_eq!(runtime_participant.gold, fixture.participant_one.gold + 500);

    assert_public_event_sequences_unique(fixture.caller_one, &fixture.session_id);
    restore_session_turn_runtime_after_upgrade();
    let restored_preview = scenario_service::preview_quest(
        fixture.caller_one,
        fixture.session_id.clone(),
        OPENING_QUEST_KEY.to_string(),
    )
    .expect("claimed quest preview should survive runtime upgrade restore");
    assert_eq!(restored_preview.quest.status, "claimed");
    let restored_status_by_nonce = event_service::get_command_status_by_nonce(
        fixture.caller_one,
        fixture.session_id.clone(),
        "claim_quest_reward".to_string(),
        claimed_nonce.clone(),
    )
    .expect("quest claim status should survive runtime upgrade restore");
    assert_eq!(restored_status_by_nonce.command_id, claimed.command_id);

    let flushed = flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("scenario quest upgrade flush should succeed");
    assert!(flushed > 0);
    assert_public_event_sequences_unique(fixture.caller_one, &fixture.session_id);
    let durable_quest = scenario_repo::find_quest_by_participant_key(
        fixture.session.id(),
        fixture.participant_one.id(),
        OPENING_QUEST_KEY,
    )
    .expect("durable quest lookup should not fail")
    .expect("durable quest should exist");
    assert_eq!(durable_quest.status, "claimed");
    assert_eq!(durable_quest.progress_value, durable_quest.required_value);
    let durable_participant = sessions::load_participant(fixture.participant_one.id())
        .expect("durable participant lookup should not fail")
        .expect("durable participant should exist");
    assert_eq!(durable_participant.gold, fixture.participant_one.gold + 500);
    let claimed_status = event_service::get_command_status(
        fixture.caller_one,
        fixture.session_id.clone(),
        claimed.command_id,
    )
    .expect("flushed quest command status should be readable by id");
    assert_eq!(claimed_status.status, CommandStatus::Applied);
    let claimed_status_by_nonce = event_service::get_command_status_by_nonce(
        fixture.caller_one,
        fixture.session_id.clone(),
        "claim_quest_reward".to_string(),
        claimed_nonce,
    )
    .expect("flushed quest command status should be readable by nonce");
    assert_eq!(claimed_status_by_nonce.status, CommandStatus::Applied);
}

#[test]
fn visible_objects_merge_runtime_world_object_overlay_with_durable_neighbors() {
    bootstrap_service_memory();
    let fixture = create_active_scenario_progress_session("render-world-merge", 1, 1, 30, 0);
    let unique = Ulid::generate().to_string();
    let ruleset_id = Id::<RulesetDefinition>::from_key(fixture.session.ruleset_id);
    let ruleset = content::load_ruleset(ruleset_id)
        .expect("ruleset lookup should not fail")
        .expect("ruleset should exist");
    let object_def = content::create_map_object_definition(
        ruleset_id,
        scenario_object_content(&ruleset.slug, &format!("merge-object-{}", &unique[..12])),
    )
    .expect("merge object definition should seed");
    let dirty_object = map_visibility_occupancy::create_world_object(
        fixture.session.id(),
        object_def.id(),
        None,
        None,
        10,
        10,
        0,
        0,
        "available".to_string(),
        "central_objective".to_string(),
        0,
        0,
        0,
        Some(r#"{"object_slug":"dirty-runtime-shrine"}"#.to_string()),
    )
    .expect("dirty world object should seed");
    let neighbor_object = map_visibility_occupancy::create_world_object(
        fixture.session.id(),
        object_def.id(),
        None,
        None,
        11,
        10,
        0,
        0,
        "available".to_string(),
        "central_objective".to_string(),
        0,
        0,
        0,
        Some(r#"{"object_slug":"durable-neighbor-shrine"}"#.to_string()),
    )
    .expect("neighbor world object should seed");
    for object in [&dirty_object, &neighbor_object] {
        map_visibility_occupancy::create_known_object(
            fixture.session.id(),
            fixture.participant_one.id(),
            "world_object".to_string(),
            object.id().to_string(),
            object.x,
            object.y,
            object.chunk_x,
            object.chunk_y,
            "visible".to_string(),
            fixture.session.current_turn,
            None,
        )
        .expect("known world object should seed");
    }

    let mut runtime = session_turn_runtime::SessionTurnRuntime::new(
        fixture.session_id.clone(),
        fixture.session.current_turn,
        1_000,
        9_000_000_000_000,
        u64::from(fixture.session.turn_duration_ms),
    );
    runtime.session = Some(fixture.session.clone());
    session_turn_runtime::insert_runtime(runtime);
    let mut dirty_runtime_object = dirty_object.clone();
    dirty_runtime_object.owner_participant_id = Some(fixture.participant_one.id().key());
    dirty_runtime_object.state = "captured".to_string();
    dirty_runtime_object.captured_turn = fixture.session.current_turn;
    session_turn_runtime::mirror_world_object_update(&dirty_runtime_object);
    let durable_dirty_object = map_visibility_occupancy::load_world_object(dirty_object.id())
        .expect("durable dirty world object lookup should not fail")
        .expect("durable dirty world object should still exist");
    assert_eq!(durable_dirty_object.state, "available");

    let object_page = game_view_service::get_visible_objects(
        fixture.caller_one,
        fixture.session_id.clone(),
        Viewport::new(8, 8, 8, 8),
        None,
        32,
    )
    .expect("visible objects should merge runtime overlays with durable rows");
    let dirty_view = object_page
        .objects
        .iter()
        .find(|object| {
            object.subject_kind == "world_object"
                && object.subject_id_text == dirty_object.id().to_string()
        })
        .expect("dirty runtime object should render from runtime overlay");
    assert_eq!((dirty_view.x, dirty_view.y), (10, 10));
    assert_eq!(
        dirty_view.owner_participant_id.as_deref(),
        Some(fixture.participant_one.id().to_string().as_str())
    );
    assert!(
        dirty_view.details_json.contains(r#""state":"captured""#),
        "dirty runtime object should expose runtime state: {}",
        dirty_view.details_json
    );
    let neighbor_view = object_page
        .objects
        .iter()
        .find(|object| {
            object.subject_kind == "world_object"
                && object.subject_id_text == neighbor_object.id().to_string()
        })
        .expect("durable-only neighbor should remain visible next to dirty runtime object");
    assert_eq!((neighbor_view.x, neighbor_view.y), (11, 10));
    assert_eq!(neighbor_view.owner_participant_id, None);
    assert!(
        neighbor_view
            .details_json
            .contains(r#""state":"available""#),
        "durable-only neighbor should expose durable state: {}",
        neighbor_view.details_json
    );
}

#[test]
fn projection_runtime_views_match_after_partial_full_flush_and_eviction() {
    bootstrap_service_memory();
    let fixture = create_active_scenario_progress_session("projection-view-parity", 1, 1, 30, 0);
    flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_STRONG_READ)
        .expect("initial scenario projection flush should leave a clean baseline");

    let unique = Ulid::generate().to_string();
    let ruleset_id = Id::<RulesetDefinition>::from_key(fixture.session.ruleset_id);
    let ruleset = content::load_ruleset(ruleset_id)
        .expect("ruleset lookup should not fail")
        .expect("ruleset should exist");
    let object_def = content::create_map_object_definition(
        ruleset_id,
        scenario_object_content(&ruleset.slug, &format!("parity-object-{}", &unique[..12])),
    )
    .expect("parity object definition should seed");
    let west_object = map_visibility_occupancy::create_world_object(
        fixture.session.id(),
        object_def.id(),
        None,
        None,
        12,
        10,
        0,
        0,
        "available".to_string(),
        "central_objective".to_string(),
        0,
        0,
        0,
        Some(r#"{"object_slug":"parity-west"}"#.to_string()),
    )
    .expect("west parity object should seed");
    let east_object = map_visibility_occupancy::create_world_object(
        fixture.session.id(),
        object_def.id(),
        None,
        None,
        13,
        10,
        0,
        0,
        "available".to_string(),
        "central_objective".to_string(),
        0,
        0,
        0,
        Some(r#"{"object_slug":"parity-east"}"#.to_string()),
    )
    .expect("east parity object should seed");
    for object in [&west_object, &east_object] {
        map_visibility_occupancy::create_known_object(
            fixture.session.id(),
            fixture.participant_one.id(),
            "world_object".to_string(),
            object.id().to_string(),
            object.x,
            object.y,
            object.chunk_x,
            object.chunk_y,
            "visible".to_string(),
            fixture.session.current_turn,
            None,
        )
        .expect("parity known object should seed");
    }

    for object in [&west_object, &east_object] {
        let mut runtime_object = object.clone();
        runtime_object.owner_participant_id = Some(fixture.participant_one.id().key());
        runtime_object.state = "captured".to_string();
        runtime_object.captured_turn = fixture.session.current_turn;
        session_turn_runtime::mirror_world_object_update(&runtime_object);
    }

    let subject_ids = BTreeSet::from([west_object.id().to_string(), east_object.id().to_string()]);
    let before_flush =
        visible_object_projection_subset(fixture.caller_one, &fixture.session_id, &subject_ids);
    assert!(before_flush.iter().all(|object| {
        object.owner_participant_id.as_deref()
            == Some(fixture.participant_one.id().to_string().as_str())
            && object.details_json.contains(r#""state":"captured""#)
    }));

    let dirty_before = session_turn_runtime::projection_diagnostic_snapshot();
    let dirty_kernel = dirty_before
        .kernels
        .iter()
        .find(|kernel| kernel.session_id == fixture.session_id)
        .expect("dirty projection kernel should exist");
    assert!(
        dirty_kernel.dirty_queue_len >= 2,
        "two mirrored objects should create at least two dirty entries"
    );

    let partial = session_turn_runtime::flush_runtime_projection_queue(
        session_turn_runtime::ProjectionFlushLimits {
            max_rows: 1,
            max_instructions: u64::MAX,
            max_stable_pages_delta: u64::MAX,
        },
    )
    .expect("bounded partial projection flush should succeed");
    assert_eq!(partial.entries_processed, 1);
    assert!(partial.truncated);
    assert!(
        partial.queue_len_after > 0,
        "partial flush should leave at least one dirty runtime entry"
    );
    let after_partial =
        visible_object_projection_subset(fixture.caller_one, &fixture.session_id, &subject_ids);
    assert_eq!(after_partial, before_flush);

    let full = session_turn_runtime::flush_runtime_projection_queue(
        session_turn_runtime::ProjectionFlushLimits::unbounded(),
    )
    .expect("full projection flush should succeed");
    assert_eq!(full.queue_len_after, 0);
    let after_full =
        visible_object_projection_subset(fixture.caller_one, &fixture.session_id, &subject_ids);
    assert_eq!(after_full, before_flush);

    session_turn_runtime::remove_runtime(&fixture.session_id, fixture.session.current_turn)
        .expect("flushed parity runtime should be removable");
    let after_eviction =
        visible_object_projection_subset(fixture.caller_one, &fixture.session_id, &subject_ids);
    assert_eq!(after_eviction, before_flush);

    let clean_after = session_turn_runtime::projection_diagnostic_snapshot();
    for kernel in clean_after
        .kernels
        .iter()
        .filter(|kernel| kernel.session_id == fixture.session_id)
    {
        assert_eq!(kernel.dirty_queue_len, 0);
        assert_eq!(kernel.lag_generations, 0);
        assert_eq!(kernel.pending_entries, 0);
    }
}

fn visible_object_projection_subset(
    caller: Principal,
    session_id: &str,
    subject_ids: &BTreeSet<String>,
) -> Vec<domm_game::ObjectView> {
    let page = game_view_service::get_visible_objects(
        caller,
        session_id.to_string(),
        Viewport::new(8, 8, 8, 8),
        None,
        64,
    )
    .expect("visible object parity page should load");
    let mut objects = page
        .objects
        .into_iter()
        .filter(|object| {
            object.subject_kind == "world_object" && subject_ids.contains(&object.subject_id_text)
        })
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| left.subject_id_text.cmp(&right.subject_id_text));
    assert_eq!(
        objects.len(),
        subject_ids.len(),
        "visible object parity page did not include all expected subjects"
    );
    objects
}

#[test]
fn scenario_progress_objectives_events_rules_and_max_turn_edges_are_stable() {
    bootstrap_service_memory();
    let fixture = create_active_scenario_progress_session("scenario-rule-edges", 7, 8, 8, 1);

    let week_one = scenario_repo::page_world_events_by_window(fixture.session.id(), "week:1")
        .expect("week one world event page should load");
    assert_eq!(week_one.items.len(), 1);
    let world_events = scenario_service::sync_world_events(
        fixture.caller_one,
        fixture.session_id.clone(),
        "nonce:scenario:world-events:week-two".to_string(),
    )
    .expect("world event sync should return");
    assert_eq!(world_events.status, CommandStatus::Applied);
    let world_events_replay = scenario_service::sync_world_events(
        fixture.caller_one,
        fixture.session_id.clone(),
        "nonce:scenario:world-events:week-two".to_string(),
    )
    .expect("world event sync replay should return");
    assert_eq!(world_events_replay.command_id, world_events.command_id);
    let world_events_fresh = scenario_service::sync_world_events(
        fixture.caller_one,
        fixture.session_id.clone(),
        "nonce:scenario:world-events:week-two:fresh".to_string(),
    )
    .expect("fresh world event sync should reuse the deterministic row");
    assert_eq!(world_events_fresh.status, CommandStatus::Applied);
    let expired_week_one =
        scenario_repo::page_world_events_by_window(fixture.session.id(), "week:1")
            .expect("expired week one world event page should load");
    assert_eq!(expired_week_one.items.len(), 1);
    assert_eq!(expired_week_one.items[0].status, "expired");
    let week_two = scenario_repo::page_world_events_by_window(fixture.session.id(), "week:2")
        .expect("week two world event page should load");
    assert_eq!(week_two.items.len(), 1);
    assert_eq!(week_two.items[0].status, "active");
    let active_world_events =
        scenario_service::get_world_events(fixture.caller_two, fixture.session_id.clone())
            .expect("active world events should load");
    assert_eq!(active_world_events.events.len(), 1);
    assert_eq!(active_world_events.events[0].event_window, "week:2");

    capture_runtime_central_objectives(&fixture);
    let synced_objectives = scenario_service::sync_objectives(
        fixture.caller_one,
        fixture.session_id.clone(),
        "nonce:scenario:objectives:sync".to_string(),
    )
    .expect("objective sync should return");
    assert_eq!(synced_objectives.status, CommandStatus::Applied);
    let objective_view =
        scenario_service::get_objective_progress(fixture.caller_two, fixture.session_id.clone())
            .expect("objective progress should be readable by the opponent");
    assert_eq!(objective_view.objectives.len(), 2);
    let participant_one_id_text = fixture.participant_one.id().to_string();
    assert!(objective_view.objectives.iter().all(|objective| {
        objective.status == "complete"
            && objective.progress_value == objective.required_value
            && objective.owner_participant_id.as_deref() == Some(participant_one_id_text.as_str())
    }));

    let victory = scenario_service::sync_advanced_victory(
        fixture.caller_one,
        fixture.session_id.clone(),
        "nonce:scenario:victory:sync".to_string(),
    )
    .expect("advanced victory sync should return");
    assert_eq!(victory.status, CommandStatus::Applied);
    let rules =
        scenario_service::get_scenario_rules(fixture.caller_one, fixture.session_id.clone())
            .expect("scenario rules should load");
    let central = rules
        .rules
        .iter()
        .find(|rule| rule.rule_key == "rule:central-objectives")
        .expect("central objective rule should exist");
    assert_eq!(central.current_value, 2);
    assert_eq!(central.victory_state, "complete");
    let max_turn = rules
        .rules
        .iter()
        .find(|rule| rule.rule_key == "rule:max-turn")
        .expect("max turn rule should exist");
    assert_eq!(max_turn.current_value, fixture.session.current_turn);
    assert_eq!(max_turn.victory_state, "max_turn_reached");
    let disabled = rules
        .rules
        .iter()
        .find(|rule| rule.rule_key == "rule:artifact-victory")
        .expect("disabled artifact rule should exist");
    assert_eq!(disabled.status, "disabled");
    assert_eq!(disabled.current_value, 0);
    assert_eq!(disabled.victory_state, "disabled");
    assert_eq!(
        disabled.disabled_reason.as_deref(),
        Some("checkpoint_24_schema_only")
    );

    assert_public_event_sequences_unique(fixture.caller_one, &fixture.session_id);
    restore_session_turn_runtime_after_upgrade();
    let restored_objectives =
        scenario_service::get_objective_progress(fixture.caller_two, fixture.session_id.clone())
            .expect("objective progress should survive runtime upgrade restore");
    assert_eq!(restored_objectives, objective_view);
    let restored_world_events =
        scenario_service::get_world_events(fixture.caller_two, fixture.session_id.clone())
            .expect("world events should survive runtime upgrade restore");
    assert_eq!(restored_world_events.events, active_world_events.events);
    let restored_rules =
        scenario_service::get_scenario_rules(fixture.caller_one, fixture.session_id.clone())
            .expect("scenario rules should survive runtime upgrade restore");
    assert_eq!(restored_rules.rules, rules.rules);

    let flushed = flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("scenario rule upgrade flush should succeed");
    assert!(flushed > 0);
    assert_public_event_sequences_unique(fixture.caller_one, &fixture.session_id);
    for key in ["objective:north", "objective:south"] {
        let row = scenario_repo::find_objective_by_key(fixture.session.id(), key)
            .expect("durable objective lookup should not fail")
            .expect("durable objective row should exist");
        assert_eq!(row.status, "complete");
        assert_eq!(row.progress_value, row.required_value);
        assert_eq!(row.participant_id, Some(fixture.participant_one.id().key()));
    }
    let durable_central =
        scenario_repo::find_scenario_rule_by_key(fixture.session.id(), "rule:central-objectives")
            .expect("durable central rule lookup should not fail")
            .expect("durable central rule should exist");
    assert_eq!(durable_central.current_value, 2);
    assert_eq!(durable_central.victory_state, "complete");
    let durable_max =
        scenario_repo::find_scenario_rule_by_key(fixture.session.id(), "rule:max-turn")
            .expect("durable max-turn rule lookup should not fail")
            .expect("durable max-turn rule should exist");
    assert_eq!(durable_max.current_value, fixture.session.current_turn);
    assert_eq!(durable_max.victory_state, "max_turn_reached");
    let durable_disabled =
        scenario_repo::find_scenario_rule_by_key(fixture.session.id(), "rule:artifact-victory")
            .expect("durable disabled rule lookup should not fail")
            .expect("durable disabled rule should exist");
    assert_eq!(durable_disabled.status, "disabled");
    assert_eq!(durable_disabled.current_value, 0);
    assert_eq!(durable_disabled.victory_state, "disabled");
}

#[test]
fn town_champion_recruit_target_allows_champion_at_town_and_creates_stack() {
    bootstrap_service_memory();
    let fixture = create_active_town_recruit_session("town-champion-recruit", 4, 4);
    let target = RecruitTarget::Champion {
        champion_id: fixture.champion.id().to_string(),
        slot_index: None,
    };

    let preview = town_service::preview_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.unit.slug.clone(),
        4,
        target.clone(),
    )
    .expect("champion target preview should run");
    assert!(preview.allowed);
    assert_eq!(preview.disabled_reason, None);
    assert_eq!(preview.target_slot_index, Some(0));
    assert_eq!(preview.total_cost.gold, 20);

    let response = town_service::submit_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.unit.slug.clone(),
        4,
        target,
        "nonce:town-champion-recruit".to_string(),
    )
    .expect("champion target recruit should return a command response");
    assert_eq!(response.status, CommandStatus::Applied);
    assert_eq!(response.phase, CommandPhase::Complete);
    assert!(response.changed_subjects.iter().any(|subject| {
        subject.subject_kind == "champion_army_stack" && subject.operation == "upsert"
    }));

    let stacks = champions_artifacts::list_champion_army_stacks(
        fixture.champion.id(),
        u32::from(domm_game::MAX_ARMY_SLOTS),
    )
    .expect("champion stacks should list");
    let stacks =
        economy_expansion::overlay_runtime_champion_army_stacks(fixture.champion.id(), stacks);
    let stack = stacks
        .iter()
        .find(|stack| stack.slot_index == 0)
        .expect("champion stack should be projected");
    assert_eq!(stack.unit_id, fixture.unit.id().key());
    assert_eq!(stack.quantity, 4);

    let pool = town_runtime::recruit_pool(&fixture.town, fixture.unit.id())
        .expect("pool lookup should run")
        .expect("pool should still exist");
    assert_eq!(pool.available, 6);
}

#[test]
fn town_champion_recruit_target_rejects_champion_away_before_spend() {
    bootstrap_service_memory();
    let fixture = create_active_town_recruit_session("town-champion-away", 5, 4);
    let target = RecruitTarget::Champion {
        champion_id: fixture.champion.id().to_string(),
        slot_index: None,
    };

    let preview = town_service::preview_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.unit.slug.clone(),
        4,
        target.clone(),
    )
    .expect("champion-away preview should return a disabled preview");
    assert!(!preview.allowed);
    assert_eq!(
        preview.disabled_reason.as_deref(),
        Some("champion_not_at_town")
    );

    let response = town_service::submit_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.unit.slug.clone(),
        4,
        target,
        "nonce:town-champion-away".to_string(),
    )
    .expect("champion-away recruit should return a command response");
    assert_failed_response_code(&response, "champion_not_at_town");

    let pool = town_runtime::recruit_pool(&fixture.town, fixture.unit.id())
        .expect("pool lookup should run")
        .expect("pool should still exist");
    assert_eq!(pool.available, 10);
    let stacks = champions_artifacts::list_champion_army_stacks(
        fixture.champion.id(),
        u32::from(domm_game::MAX_ARMY_SLOTS),
    )
    .expect("champion stacks should list");
    let stacks =
        economy_expansion::overlay_runtime_champion_army_stacks(fixture.champion.id(), stacks);
    assert!(stacks.is_empty());
}

#[test]
fn city_economy_integration_keeps_runtime_views_replays_and_flush_consistent() {
    bootstrap_service_memory();
    let fixture = create_active_city_economy_session("city-economy-integration");
    let town_id = fixture.town.id().to_string();
    let champion_id = fixture.champion.id().to_string();

    let initial_town =
        town_service::get_town_view(fixture.caller, fixture.session_id.clone(), town_id.clone())
            .expect("initial town view should load");
    let initial_pool = initial_town
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == fixture.unit.slug)
        .expect("initial town recruit pool should be visible");
    assert_eq!(initial_pool.available, 8);

    let offers = economy_expansion::get_tavern_offers(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
    )
    .expect("tavern offers should load");
    let offer = offers
        .offers
        .iter()
        .find(|offer| offer.offer_key == fixture.offer_key)
        .expect("seeded tavern offer should be returned");
    assert_eq!(offer.status, "available");

    let initial_dwelling = economy_expansion::get_dwelling_pool(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
    )
    .expect("initial dwelling pool should load");
    assert_eq!(initial_dwelling.available, 3);

    let build_preview = town_service::preview_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.base_building_slug.clone(),
    )
    .expect("building preview should load");
    assert!(build_preview.allowed);
    assert_eq!(build_preview.cost.gold, 7);
    assert_eq!(build_preview.cost.wood, 3);

    let build_nonce = "nonce:city-economy-integration:build".to_string();
    let build = town_service::submit_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.base_building_slug.clone(),
        build_nonce.clone(),
    )
    .expect("building submit should return");
    assert_eq!(build.status, CommandStatus::Applied);
    let build_replay = town_service::submit_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.base_building_slug.clone(),
        build_nonce,
    )
    .expect("building replay should return");
    assert_eq!(build_replay.status, CommandStatus::Applied);
    assert_eq!(build_replay.command_id, build.command_id);

    let after_build =
        town_service::get_town_view(fixture.caller, fixture.session_id.clone(), town_id.clone())
            .expect("town view after build should load");
    assert!(
        after_build
            .buildings
            .iter()
            .any(|building| { building.building_slug == fixture.base_building_slug })
    );

    let recruit_preview = town_service::preview_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.unit.slug.clone(),
        2,
        RecruitTarget::TownGarrison { slot_index: None },
    )
    .expect("town recruit preview should load");
    assert!(recruit_preview.allowed);
    assert_eq!(recruit_preview.available, 8);
    assert_eq!(recruit_preview.total_cost.gold, 10);

    let recruit_nonce = "nonce:city-economy-integration:town-recruit".to_string();
    let recruited = town_service::submit_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.unit.slug.clone(),
        2,
        RecruitTarget::TownGarrison { slot_index: None },
        recruit_nonce.clone(),
    )
    .expect("town recruit submit should return");
    assert_eq!(recruited.status, CommandStatus::Applied);
    let recruited_replay = town_service::submit_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.unit.slug.clone(),
        2,
        RecruitTarget::TownGarrison { slot_index: None },
        recruit_nonce,
    )
    .expect("town recruit replay should return");
    assert_eq!(recruited_replay.status, CommandStatus::Applied);
    assert_eq!(recruited_replay.command_id, recruited.command_id);

    let after_recruit =
        town_service::get_town_view(fixture.caller, fixture.session_id.clone(), town_id.clone())
            .expect("town view after recruit should load");
    let runtime_pool = after_recruit
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == fixture.unit.slug)
        .expect("runtime recruit pool should remain visible");
    assert_eq!(runtime_pool.available, 6);
    let garrison = after_recruit
        .garrison_stacks
        .iter()
        .find(|stack| stack.unit_slug == fixture.unit.slug)
        .expect("runtime garrison stack should be visible");
    assert_eq!(garrison.quantity, 2);

    let hire_preview = economy_expansion::preview_hire_champion(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.offer_key.clone(),
    )
    .expect("hire preview should load");
    assert!(hire_preview.allowed);
    assert_eq!(hire_preview.cost.gold, 100);

    let hire_nonce = "nonce:city-economy-integration:hire".to_string();
    let hired = economy_expansion::hire_tavern_champion(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.offer_key.clone(),
        hire_nonce.clone(),
    )
    .expect("tavern hire should return");
    assert_eq!(hired.status, CommandStatus::Applied);
    let hired_replay = economy_expansion::hire_tavern_champion(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.offer_key.clone(),
        hire_nonce,
    )
    .expect("tavern hire replay should return");
    assert_eq!(hired_replay.status, CommandStatus::Applied);
    assert_eq!(hired_replay.command_id, hired.command_id);
    let offers_after_hire = economy_expansion::get_tavern_offers(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
    )
    .expect("tavern offers after hire should load");
    let hired_offer = offers_after_hire
        .offers
        .iter()
        .find(|offer| offer.offer_key == fixture.offer_key)
        .expect("hired offer should still be visible");
    assert_eq!(hired_offer.status, "hired");
    assert!(hired_offer.hired_champion_id.is_some());

    let market_preview = economy_expansion::preview_market_trade(
        fixture.caller,
        fixture.session_id.clone(),
        "stone".to_string(),
        "crystal".to_string(),
        10,
    )
    .expect("market preview should load");
    assert!(market_preview.allowed);
    assert_eq!(market_preview.amount_out, 1);

    let market_nonce = "nonce:city-economy-integration:market".to_string();
    let market = economy_expansion::submit_market_trade(
        fixture.caller,
        fixture.session_id.clone(),
        "stone".to_string(),
        "crystal".to_string(),
        10,
        market_nonce.clone(),
    )
    .expect("market submit should return");
    assert_eq!(market.status, CommandStatus::Applied);
    let market_replay = economy_expansion::submit_market_trade(
        fixture.caller,
        fixture.session_id.clone(),
        "stone".to_string(),
        "crystal".to_string(),
        10,
        market_nonce,
    )
    .expect("market replay should return");
    assert_eq!(market_replay.status, CommandStatus::Applied);
    assert_eq!(market_replay.command_id, market.command_id);

    let dwelling_preview = economy_expansion::preview_dwelling_recruit(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
        fixture.unit.slug.clone(),
        1,
        champion_id.clone(),
    )
    .expect("dwelling recruit preview should load");
    assert!(dwelling_preview.allowed);
    assert_eq!(dwelling_preview.available, 3);
    assert_eq!(dwelling_preview.total_cost.gold, 5);

    let dwelling_nonce = "nonce:city-economy-integration:dwelling".to_string();
    let dwelling = economy_expansion::submit_dwelling_recruit(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
        fixture.unit.slug.clone(),
        1,
        champion_id.clone(),
        dwelling_nonce.clone(),
    )
    .expect("dwelling recruit submit should return");
    assert_eq!(dwelling.status, CommandStatus::Applied);
    let dwelling_replay = economy_expansion::submit_dwelling_recruit(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
        fixture.unit.slug.clone(),
        1,
        champion_id,
        dwelling_nonce,
    )
    .expect("dwelling recruit replay should return");
    assert_eq!(dwelling_replay.status, CommandStatus::Applied);
    assert_eq!(dwelling_replay.command_id, dwelling.command_id);

    let after_dwelling = economy_expansion::get_dwelling_pool(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
    )
    .expect("dwelling pool after recruit should load");
    assert_eq!(after_dwelling.available, 2);
    let stacks = champions_artifacts::list_champion_army_stacks(
        fixture.champion.id(),
        u32::from(domm_game::MAX_ARMY_SLOTS),
    )
    .expect("champion stacks should list after dwelling recruit");
    let stacks =
        economy_expansion::overlay_runtime_champion_army_stacks(fixture.champion.id(), stacks);
    let champion_stack = stacks
        .iter()
        .find(|stack| stack.unit_id == fixture.unit.id().key())
        .expect("dwelling recruit should update the champion army");
    assert_eq!(champion_stack.quantity, 1);

    assert_public_event_sequences_unique(fixture.caller, &fixture.session_id);
    restore_session_turn_runtime_after_upgrade();
    let restored_town =
        town_service::get_town_view(fixture.caller, fixture.session_id.clone(), town_id.clone())
            .expect("town view should survive runtime upgrade restore");
    assert!(
        restored_town
            .buildings
            .iter()
            .any(|building| { building.building_slug == fixture.base_building_slug })
    );
    assert!(
        restored_town
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == fixture.unit.slug && stack.quantity == 2)
    );
    assert!(
        restored_town
            .recruit_pools
            .iter()
            .any(|pool| pool.unit_slug == fixture.unit.slug && pool.available == 6)
    );
    let restored_offers = economy_expansion::get_tavern_offers(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
    )
    .expect("tavern offers should survive runtime upgrade restore");
    assert!(
        restored_offers
            .offers
            .iter()
            .any(|offer| offer.offer_key == fixture.offer_key && offer.status == "hired")
    );
    let restored_dwelling = economy_expansion::get_dwelling_pool(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
    )
    .expect("dwelling pool should survive runtime upgrade restore");
    assert_eq!(restored_dwelling.available, 2);
    let restored_status = event_service::get_command_status(
        fixture.caller,
        fixture.session_id.clone(),
        dwelling.command_id.clone(),
    )
    .expect("dwelling command status should survive runtime upgrade restore");
    assert_eq!(restored_status.status, CommandStatus::Applied);

    let flushed = flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("upgrade flush barrier should succeed");
    assert!(flushed > 0);
    let flushed_session =
        sessions::load_session(Id::<GameSession>::from_key(fixture.town.session_id))
            .expect("flushed city/economy session lookup should not fail")
            .expect("flushed city/economy session should exist");
    session_turn_runtime::remove_runtime(&fixture.session_id, flushed_session.current_turn)
        .expect("flushed city/economy runtime should be removable");
    assert!(
        !session_turn_runtime::contains_runtime(&fixture.session_id, flushed_session.current_turn),
        "city/economy durable fallback checks should run without session turn runtime"
    );
    assert_public_event_sequences_unique(fixture.caller, &fixture.session_id);

    let flushed_town =
        town_service::get_town_view(fixture.caller, fixture.session_id.clone(), town_id)
            .expect("town view after flush should load");
    assert!(
        flushed_town
            .buildings
            .iter()
            .any(|building| { building.building_slug == fixture.base_building_slug })
    );
    assert!(
        flushed_town
            .garrison_stacks
            .iter()
            .any(|stack| stack.unit_slug == fixture.unit.slug && stack.quantity == 2)
    );
    assert!(
        flushed_town
            .recruit_pools
            .iter()
            .any(|pool| pool.unit_slug == fixture.unit.slug && pool.available == 6)
    );

    let participant = sessions::load_participant(fixture.participant_id)
        .expect("participant lookup should not fail")
        .expect("participant should exist after city/economy flush");
    assert_eq!(participant.gold, 9_878);
    assert_eq!(participant.wood, 7);
    assert_eq!(participant.stone, 0);
    assert_eq!(participant.crystal, 4);

    let durable_stack = champions_artifacts::find_champion_army_stack(fixture.champion.id(), 0)
        .expect("durable champion stack lookup should not fail")
        .expect("champion stack should flush to durable storage");
    assert_eq!(durable_stack.quantity, 1);

    for response in [&build, &recruited, &hired, &market, &dwelling] {
        let status = event_service::get_command_status(
            fixture.caller,
            fixture.session_id.clone(),
            response.command_id.clone(),
        )
        .expect("flushed city/economy command status should load");
        assert_eq!(status.status, CommandStatus::Applied);
    }
}

#[test]
fn economy_market_rejects_invalid_trades_and_insufficient_resources() {
    bootstrap_service_memory();
    let fixture = create_active_economy_negative_session("economy-negative-market");

    let zero = economy_expansion::preview_market_trade(
        fixture.caller,
        fixture.session_id.clone(),
        "wood".to_string(),
        "crystal".to_string(),
        0,
    )
    .expect_err("zero-amount market trades should fail before command creation");
    assert_eq!(zero.code, "invalid_market_trade");

    let non_divisible = economy_expansion::preview_market_trade(
        fixture.caller,
        fixture.session_id.clone(),
        "wood".to_string(),
        "crystal".to_string(),
        11,
    )
    .expect_err("non-divisible market trades should fail before command creation");
    assert_eq!(non_divisible.code, "invalid_market_trade");

    let invalid_pair = economy_expansion::preview_market_trade(
        fixture.caller,
        fixture.session_id.clone(),
        "wood".to_string(),
        "aether".to_string(),
        10,
    )
    .expect_err("invalid market pairs should fail before command creation");
    assert_eq!(invalid_pair.code, "invalid_market_trade");

    let insufficient = economy_expansion::preview_market_trade(
        fixture.caller,
        fixture.session_id.clone(),
        "wood".to_string(),
        "crystal".to_string(),
        20,
    )
    .expect("valid but unaffordable market trade should preview");
    assert!(!insufficient.allowed);
    assert_eq!(
        insufficient.disabled_reason.as_deref(),
        Some("insufficient_resources")
    );

    let response = economy_expansion::submit_market_trade(
        fixture.caller,
        fixture.session_id,
        "wood".to_string(),
        "crystal".to_string(),
        20,
        "nonce:economy-negative-market:insufficient".to_string(),
    )
    .expect("unaffordable market trade should return a command response");
    assert_failed_response_code(&response, "insufficient_resources");
}

#[test]
fn economy_tavern_offer_cannot_be_hired_again_with_fresh_nonce() {
    bootstrap_service_memory();
    let fixture = create_active_economy_negative_session("economy-negative-tavern");

    let preview = economy_expansion::preview_hire_champion(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.offer_key.clone(),
    )
    .expect("available tavern offer should preview");
    assert!(preview.allowed);
    assert_eq!(preview.disabled_reason, None);

    let first = economy_expansion::hire_tavern_champion(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.offer_key.clone(),
        "nonce:economy-negative-tavern:first".to_string(),
    )
    .expect("first tavern hire should return a command response");
    assert_eq!(first.status, CommandStatus::Applied);
    assert_eq!(first.phase, CommandPhase::Complete);

    let replay = economy_expansion::hire_tavern_champion(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.offer_key.clone(),
        "nonce:economy-negative-tavern:first".to_string(),
    )
    .expect("same tavern hire nonce should replay");
    assert_eq!(replay.status, CommandStatus::Applied);
    assert_eq!(replay.command_id, first.command_id);

    let fresh_nonce = economy_expansion::hire_tavern_champion(
        fixture.caller,
        fixture.session_id,
        fixture.town.id().to_string(),
        fixture.offer_key,
        "nonce:economy-negative-tavern:second".to_string(),
    )
    .expect("fresh nonce against hired offer should return a command response");
    assert_failed_response_code(&fresh_nonce, "offer_not_available");
}

#[test]
fn economy_dwelling_recruit_rejects_empty_and_over_max_pools() {
    bootstrap_service_memory();
    let fixture = create_active_economy_negative_session("economy-negative-dwelling");

    let empty = economy_expansion::preview_dwelling_recruit(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
        fixture.unit.slug.clone(),
        1,
        fixture.champion.id().to_string(),
    )
    .expect("empty dwelling pool should return a disabled preview");
    assert!(!empty.allowed);
    assert_eq!(
        empty.disabled_reason.as_deref(),
        Some("dwelling_pool_empty")
    );
    assert_eq!(empty.available, 0);

    let empty_response = economy_expansion::submit_dwelling_recruit(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
        fixture.unit.slug.clone(),
        1,
        fixture.champion.id().to_string(),
        "nonce:economy-negative-dwelling:empty".to_string(),
    )
    .expect("empty dwelling recruit should return a command response");
    assert_failed_response_code(&empty_response, "dwelling_pool_empty");

    let over_max_quantity = domm_game::DWELLING_RECRUIT_MAX_QUANTITY + 1;
    let over_max = economy_expansion::preview_dwelling_recruit(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.dwelling_object_id.clone(),
        fixture.unit.slug.clone(),
        over_max_quantity,
        fixture.champion.id().to_string(),
    )
    .expect("over-max dwelling quantity should return a disabled preview");
    assert!(!over_max.allowed);
    assert_eq!(
        over_max.disabled_reason.as_deref(),
        Some("invalid_quantity")
    );

    let over_max_response = economy_expansion::submit_dwelling_recruit(
        fixture.caller,
        fixture.session_id,
        fixture.dwelling_object_id,
        fixture.unit.slug,
        over_max_quantity,
        fixture.champion.id().to_string(),
        "nonce:economy-negative-dwelling:over-max".to_string(),
    )
    .expect("over-max dwelling recruit should return a command response");
    assert_failed_response_code(&over_max_response, "invalid_quantity");
}

#[test]
fn economy_town_recruit_rejects_invalid_quantity_and_over_pool_without_spend() {
    bootstrap_service_memory();
    let fixture = create_active_economy_negative_session("economy-negative-town-recruit");
    let town_id = fixture.town.id().to_string();
    let target = RecruitTarget::TownGarrison { slot_index: None };

    let invalid = town_service::preview_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.unit.slug.clone(),
        0,
        target.clone(),
    )
    .expect("zero quantity town recruit preview should return");
    assert!(!invalid.allowed);
    assert_eq!(invalid.disabled_reason.as_deref(), Some("invalid_quantity"));

    let over_pool = town_service::preview_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.unit.slug.clone(),
        9,
        target.clone(),
    )
    .expect("over-pool town recruit preview should return");
    assert!(!over_pool.allowed);
    assert_eq!(
        over_pool.disabled_reason.as_deref(),
        Some("recruit_pool_empty")
    );
    assert_eq!(over_pool.available, 8);

    let response = town_service::submit_recruit_units(
        fixture.caller,
        fixture.session_id.clone(),
        town_id.clone(),
        fixture.unit.slug.clone(),
        9,
        target,
        "nonce:economy-negative-town-recruit:over-pool".to_string(),
    )
    .expect("over-pool town recruit should return a command response");
    assert_failed_response_code(&response, "recruit_pool_empty");

    let town_view = town_service::get_town_view(fixture.caller, fixture.session_id, town_id)
        .expect("town view after failed recruit should load");
    let pool = town_view
        .recruit_pools
        .iter()
        .find(|pool| pool.unit_slug == fixture.unit.slug)
        .expect("town recruit pool should remain visible");
    assert_eq!(pool.available, 8);
    assert!(
        town_view.garrison_stacks.is_empty(),
        "failed over-pool recruit must not create garrison stacks"
    );
}

#[test]
fn economy_building_failures_and_resource_ledgers_match_balances() {
    bootstrap_service_memory();
    let fixture = create_active_economy_negative_session("economy-negative-building");
    let missing_code = format!("missing_prerequisite:{}", fixture.base_building_slug);

    let missing_preview = town_service::preview_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.locked_building_slug.clone(),
    )
    .expect("locked building should preview");
    assert!(!missing_preview.allowed);
    assert_eq!(
        missing_preview.disabled_reason.as_deref(),
        Some(missing_code.as_str())
    );

    let missing_submit = town_service::submit_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.locked_building_slug.clone(),
        "nonce:economy-negative-building:missing".to_string(),
    )
    .expect("locked building should return a command response");
    assert_failed_response_code(&missing_submit, &missing_code);

    let build = town_service::submit_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.base_building_slug.clone(),
        "nonce:economy-negative-building:build".to_string(),
    )
    .expect("base building should return a command response");
    assert_eq!(build.status, CommandStatus::Applied);
    assert_eq!(build.phase, CommandPhase::Complete);

    let duplicate_preview = town_service::preview_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.base_building_slug.clone(),
    )
    .expect("duplicate building preview should return");
    assert!(!duplicate_preview.allowed);
    assert_eq!(
        duplicate_preview.disabled_reason.as_deref(),
        Some("already_built")
    );

    let duplicate_submit = town_service::submit_build_town_structure(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.town.id().to_string(),
        fixture.base_building_slug.clone(),
        "nonce:economy-negative-building:duplicate".to_string(),
    )
    .expect("duplicate building should return a command response");
    assert_failed_response_code(&duplicate_submit, "already_built");

    let outcome = session_turn_runtime::flush_runtime_projection_queue(
        session_turn_runtime::ProjectionFlushLimits::unbounded(),
    )
    .expect("economy projection flush should succeed");
    assert_eq!(outcome.queue_len_after, 0);
    assert!(!outcome.truncated);

    let participant = sessions::load_participant(fixture.participant_id)
        .expect("participant lookup should not fail")
        .expect("participant should exist after flush");
    assert_eq!(participant.gold, 9_993);
    assert_eq!(participant.wood, 7);

    let build_command_id = Id::<GameCommand>::from_key(
        Ulid::from_str(&build.command_id).expect("build command id should be a Ulid"),
    );
    let gold_ledger = economy_repo::find_resource_ledger_entry(
        build_command_id,
        &format!("build:{}:gold", fixture.base_building_slug),
    )
    .expect("gold ledger lookup should not fail")
    .expect("gold ledger should flush");
    assert_eq!(gold_ledger.delta, -7);
    assert_eq!(gold_ledger.balance_after, participant.gold);

    let wood_ledger = economy_repo::find_resource_ledger_entry(
        build_command_id,
        &format!("build:{}:wood", fixture.base_building_slug),
    )
    .expect("wood ledger lookup should not fail")
    .expect("wood ledger should flush");
    assert_eq!(wood_ledger.delta, -3);
    assert_eq!(wood_ledger.balance_after, u64::from(participant.wood));
}

#[test]
fn service_magic_first_playable_setup_warms_preferred_spell_cache() {
    bootstrap_service_memory();
    content::clear_cached_spell_for_tests();

    first_playable_setup::ensure_first_playable_content_rows()
        .expect("first-playable content rows should seed");
    let ruleset = content::find_ruleset_by_slug_version(
        FIRST_PLAYABLE_RULESET_SLUG,
        FIRST_PLAYABLE_RULESET_VERSION,
    )
    .expect("first-playable ruleset lookup should not fail")
    .expect("first-playable ruleset should exist");
    let cached = content::cached_spell_slug_for_tests();

    assert_eq!(cached.as_deref(), Some("spite-march"));
    assert!(
        content::find_spell_by_ruleset_slug(ruleset.id(), "spite-march")
            .expect("preferred spell lookup should not fail")
            .is_some()
    );
}

#[test]
fn service_content_first_playable_seed_covers_every_manifest_slug() {
    bootstrap_service_memory();
    content::clear_cached_spell_for_tests();

    let manifest = first_playable_content_manifest();
    let rows = first_playable_setup::ensure_first_playable_content_rows()
        .expect("first-playable content rows should seed");
    let ruleset = content::find_ruleset_by_slug_version(
        FIRST_PLAYABLE_RULESET_SLUG,
        FIRST_PLAYABLE_RULESET_VERSION,
    )
    .expect("first-playable ruleset lookup should not fail")
    .expect("first-playable ruleset should exist");
    let terrain = content::page_terrain_by_ruleset(ruleset.id())
        .expect("first-playable terrain rows should page");
    let spells = content::page_spells_by_ruleset(ruleset.id())
        .expect("first-playable spell rows should page");

    assert_eq!(
        seeded_content_set(rows.factions.keys().cloned()),
        seeded_content_set(manifest.factions.iter().map(|row| row.slug.clone()))
    );
    assert_eq!(
        seeded_content_set(rows.champion_classes.keys().cloned()),
        seeded_content_set(manifest.champion_classes.iter().map(|row| row.slug.clone()))
    );
    assert_eq!(
        seeded_content_set(terrain.iter().map(|row| row.terrain_key.clone())),
        seeded_content_set(manifest.terrain.iter().map(|row| row.terrain_key.clone()))
    );
    assert_eq!(
        seeded_content_set(rows.units.keys().cloned()),
        seeded_content_set(manifest.units.iter().map(|row| row.slug.clone()))
    );
    assert_eq!(
        seeded_content_set(rows.buildings.keys().cloned()),
        seeded_content_set(manifest.buildings.iter().map(|row| row.slug.clone()))
    );
    assert_eq!(
        seeded_content_set(spells.iter().map(|row| row.slug.clone())),
        seeded_content_set(manifest.spells.iter().map(|row| row.slug.clone()))
    );
    assert_eq!(
        seeded_content_set(rows.artifacts.keys().cloned()),
        seeded_content_set(manifest.artifacts.iter().map(|row| row.slug.clone()))
    );
    assert_eq!(
        seeded_content_set(rows.map_objects.keys().cloned()),
        seeded_content_set(manifest.map_objects.iter().map(|row| row.slug.clone()))
    );

    for spell in &manifest.spells {
        assert!(
            content::find_spell_by_ruleset_slug(ruleset.id(), &spell.slug)
                .expect("seeded first-playable spell lookup should not fail")
                .is_some(),
            "missing first-playable spell {}",
            spell.slug
        );
    }
    assert_eq!(
        content::cached_spell_slug_for_tests().as_deref(),
        Some("spite-march")
    );
}

#[test]
fn service_content_seed_repairs_partial_spell_sentinel_and_warms_preferred_cache() {
    bootstrap_service_memory();
    content::clear_cached_spell_for_tests();

    let (ruleset, factions, manifest) = isolated_manifest_ruleset("service-content-partial-spell");
    let preferred = manifest
        .spell("spite-march")
        .expect("preferred first-playable spell should exist")
        .clone();
    content::create_spell_definition(ruleset.id(), preferred)
        .expect("preferred spell sentinel should seed");
    content::clear_cached_spell_for_tests();
    assert!(
        content::find_spell_by_ruleset_slug(ruleset.id(), "hex-spark")
            .expect("pre-repair missing spell lookup should not fail")
            .is_none()
    );
    content::clear_cached_spell_for_tests();

    first_playable_setup::seed_content_definition_batches_for_tests(
        ruleset.id(),
        &manifest,
        &factions,
    )
    .expect("content seeding should repair missing manifest siblings");
    first_playable_setup::seed_content_definition_batches_for_tests(
        ruleset.id(),
        &manifest,
        &factions,
    )
    .expect("content seeding should stay idempotent after repair");

    let spells =
        content::page_spells_by_ruleset(ruleset.id()).expect("repaired spell rows should page");
    assert_eq!(
        seeded_content_set(spells.iter().map(|row| row.slug.clone())),
        seeded_content_set(manifest.spells.iter().map(|row| row.slug.clone()))
    );
    for spell in &manifest.spells {
        assert!(
            content::find_spell_by_ruleset_slug(ruleset.id(), &spell.slug)
                .expect("repaired spell lookup should not fail")
                .is_some(),
            "missing repaired spell {}",
            spell.slug
        );
    }
    assert_eq!(
        content::cached_spell_slug_for_tests().as_deref(),
        Some("spite-march")
    );
}

#[test]
fn champion_magic_sour_sorcery_marks_empty_runtime_spellbook_only_for_sour() {
    bootstrap_service_memory();

    let dirty_fixture = create_active_magic_session("service-magic-dirty", 1, Vec::new(), 4);
    let dirty = champion_magic::select_champion_level_up(
        dirty_fixture.caller,
        dirty_fixture.session_id.clone(),
        dirty_fixture.champion.id().to_string(),
        "dirty_tactics".to_string(),
        "nonce:service-magic-dirty:skill".to_string(),
    )
    .expect("dirty tactics selection should not trap");
    assert_eq!(dirty.status, CommandStatus::Applied);
    assert!(
        session_turn_runtime::runtime_champion_spell_slugs_if_complete(
            &dirty_fixture.session_id,
            dirty_fixture.session.current_turn,
            dirty_fixture.champion.id(),
        )
        .is_none(),
        "non-sour skills must not mark an empty runtime spellbook complete"
    );

    let sour_fixture = create_active_magic_session("service-magic-sour", 1, Vec::new(), 4);
    let sour = champion_magic::select_champion_level_up(
        sour_fixture.caller,
        sour_fixture.session_id.clone(),
        sour_fixture.champion.id().to_string(),
        "sour_sorcery".to_string(),
        "nonce:service-magic-sour:skill".to_string(),
    )
    .expect("sour sorcery selection should not trap");
    assert_eq!(sour.status, CommandStatus::Applied);
    assert_eq!(
        session_turn_runtime::runtime_champion_spell_slugs_if_complete(
            &sour_fixture.session_id,
            sour_fixture.session.current_turn,
            sour_fixture.champion.id(),
        ),
        Some(Vec::new()),
        "sour_sorcery should mark the empty active spellbook complete"
    );
}

#[test]
fn champion_magic_uses_runtime_spellbook_and_durable_cast_fallback() {
    bootstrap_service_memory();

    let runtime_fixture = create_active_magic_session("service-magic-runtime", 1, Vec::new(), 4);
    champion_magic::select_champion_level_up(
        runtime_fixture.caller,
        runtime_fixture.session_id.clone(),
        runtime_fixture.champion.id().to_string(),
        "sour_sorcery".to_string(),
        "nonce:service-magic-runtime:skill".to_string(),
    )
    .expect("runtime sour selection should not trap");

    let test_command = Id::<GameCommand>::from_key(Ulid::generate());
    champions_artifacts::create_champion_spell(
        runtime_fixture.session.id(),
        runtime_fixture.champion.id(),
        runtime_fixture.hex_spark.id(),
        "hex-spark",
        runtime_fixture.session.current_turn,
        test_command,
    )
    .expect("test durable hex-spark row should persist");

    let progression_before_learn = champion_magic::preview_champion_progression(
        runtime_fixture.caller,
        runtime_fixture.session_id.clone(),
        runtime_fixture.champion.id().to_string(),
    )
    .expect("runtime progression should load");
    assert!(
        progression_before_learn.learned_spell_slugs.is_empty(),
        "complete runtime spellbook should avoid paging durable spell rows"
    );

    let learned = champion_magic::learn_champion_spell(
        runtime_fixture.caller,
        runtime_fixture.session_id.clone(),
        runtime_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-runtime:learn".to_string(),
    )
    .expect("runtime spell learn should not trap");
    assert_eq!(learned.status, CommandStatus::Applied);
    assert!(
        champions_artifacts::find_champion_spell(
            runtime_fixture.champion.id(),
            runtime_fixture.spite_march.id(),
        )
        .expect("durable runtime-learned spell lookup should not fail")
        .is_none(),
        "active runtime spell learning should not immediately page/write durable spell rows"
    );
    let progression_after_learn = champion_magic::preview_champion_progression(
        runtime_fixture.caller,
        runtime_fixture.session_id.clone(),
        runtime_fixture.champion.id().to_string(),
    )
    .expect("runtime progression after learn should load");
    assert_eq!(
        progression_after_learn.learned_spell_slugs,
        vec!["spite-march".to_string()]
    );

    let runtime_cast = champion_magic::cast_adventure_spell(
        runtime_fixture.caller,
        runtime_fixture.session_id.clone(),
        runtime_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-runtime:cast".to_string(),
    )
    .expect("runtime-learned adventure spell should cast before durable flush");
    assert_eq!(runtime_cast.status, CommandStatus::Applied);

    let durable_fixture = create_active_magic_session(
        "service-magic-durable",
        0,
        vec!["sour_sorcery".to_string()],
        4,
    );
    champions_artifacts::create_champion_spell(
        durable_fixture.session.id(),
        durable_fixture.champion.id(),
        durable_fixture.spite_march.id(),
        "spite-march",
        durable_fixture.session.current_turn,
        test_command,
    )
    .expect("durable fallback spell row should persist");
    session_turn_runtime::remove_runtime(
        &durable_fixture.session_id,
        durable_fixture.session.current_turn,
    )
    .expect("active runtime should be removable for durable fallback coverage");

    let durable_cast = champion_magic::cast_adventure_spell(
        durable_fixture.caller,
        durable_fixture.session_id,
        durable_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-durable:cast".to_string(),
    )
    .expect("durable learned adventure spell should cast without runtime snapshot");
    assert_eq!(durable_cast.status, CommandStatus::Applied);
}

#[test]
fn champion_magic_negative_paths_return_typed_failures() {
    bootstrap_service_memory();

    let invalid_skill_fixture =
        create_active_magic_session("service-magic-invalid-skill", 1, Vec::new(), 4);
    let invalid_skill = champion_magic::select_champion_level_up(
        invalid_skill_fixture.caller,
        invalid_skill_fixture.session_id.clone(),
        invalid_skill_fixture.champion.id().to_string(),
        "bogus_skill".to_string(),
        "nonce:service-magic-invalid-skill".to_string(),
    )
    .expect("invalid skill choice should return a command response");
    assert_failed_response_code(&invalid_skill, "invalid_skill_choice");

    let non_sour_fixture = create_active_magic_session(
        "service-magic-non-sour",
        0,
        vec!["dirty_tactics".to_string()],
        4,
    );
    let missing_prerequisite = champion_magic::learn_champion_spell(
        non_sour_fixture.caller,
        non_sour_fixture.session_id.clone(),
        non_sour_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-non-sour:learn".to_string(),
    )
    .expect("non-sour spell learn should return a command response");
    assert_failed_response_code(&missing_prerequisite, "spell_prerequisite_missing");

    let sour_fixture = create_active_magic_session(
        "service-magic-negative-sour",
        0,
        vec!["sour_sorcery".to_string()],
        4,
    );
    let unknown_spell = champion_magic::learn_champion_spell(
        sour_fixture.caller,
        sour_fixture.session_id.clone(),
        sour_fixture.champion.id().to_string(),
        "unknown-spell".to_string(),
        "nonce:service-magic-negative-sour:unknown".to_string(),
    )
    .expect("unknown spell learn should return a command response");
    assert_failed_response_code(&unknown_spell, "spell_not_found");

    let unlearned_cast = champion_magic::cast_adventure_spell(
        sour_fixture.caller,
        sour_fixture.session_id.clone(),
        sour_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-negative-sour:unlearned".to_string(),
    )
    .expect("unlearned cast should return a command response");
    assert_failed_response_code(&unlearned_cast, "spell_not_learned");

    let low_mana_fixture = create_active_magic_session(
        "service-magic-low-mana",
        0,
        vec!["sour_sorcery".to_string()],
        1,
    );
    create_durable_spell_row(&low_mana_fixture, &low_mana_fixture.spite_march);
    let insufficient_mana = champion_magic::cast_adventure_spell(
        low_mana_fixture.caller,
        low_mana_fixture.session_id.clone(),
        low_mana_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-low-mana:cast".to_string(),
    )
    .expect("low-mana cast should return a command response");
    assert_failed_response_code(&insufficient_mana, "insufficient_mana");
}

#[test]
fn champion_magic_rejects_duplicate_nonce_duplicate_spell_and_spell_cap() {
    bootstrap_service_memory();

    let nonce_fixture = create_active_magic_session("service-magic-nonce", 2, Vec::new(), 4);
    let selected = champion_magic::select_champion_level_up(
        nonce_fixture.caller,
        nonce_fixture.session_id.clone(),
        nonce_fixture.champion.id().to_string(),
        "dirty_tactics".to_string(),
        "nonce:service-magic-nonce:skill".to_string(),
    )
    .expect("first skill selection should apply");
    assert_eq!(selected.status, CommandStatus::Applied);
    let mismatch = champion_magic::select_champion_level_up(
        nonce_fixture.caller,
        nonce_fixture.session_id.clone(),
        nonce_fixture.champion.id().to_string(),
        "grim_logistics".to_string(),
        "nonce:service-magic-nonce:skill".to_string(),
    )
    .expect("duplicate nonce mismatch should return a command response");
    assert_failed_response_code(&mismatch, "duplicate_nonce_payload_mismatch");

    let duplicate_fixture = create_active_magic_session(
        "service-magic-duplicate-spell",
        0,
        vec!["sour_sorcery".to_string()],
        4,
    );
    let learned = champion_magic::learn_champion_spell(
        duplicate_fixture.caller,
        duplicate_fixture.session_id.clone(),
        duplicate_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-duplicate-spell:learn".to_string(),
    )
    .expect("initial spell learn should apply");
    assert_eq!(learned.status, CommandStatus::Applied);
    let duplicate = champion_magic::learn_champion_spell(
        duplicate_fixture.caller,
        duplicate_fixture.session_id.clone(),
        duplicate_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-duplicate-spell:learn-again".to_string(),
    )
    .expect("duplicate spell learn should return a command response");
    assert_failed_response_code(&duplicate, "spell_already_learned");

    let cap_fixture = create_active_magic_session(
        "service-magic-spell-cap",
        0,
        vec!["sour_sorcery".to_string()],
        4,
    );
    fill_runtime_spellbook_to_cap(&cap_fixture);
    let cap = champion_magic::learn_champion_spell(
        cap_fixture.caller,
        cap_fixture.session_id.clone(),
        cap_fixture.champion.id().to_string(),
        "spite-march".to_string(),
        "nonce:service-magic-spell-cap:learn".to_string(),
    )
    .expect("spell cap learn should return a command response");
    assert_failed_response_code(&cap, "spellbook_cap_exceeded");
}

#[cfg(any(not(feature = "benchmark"), feature = "projection-benchmark"))]
#[test]
fn champion_magic_replays_learn_and_cast_after_projection_flush() {
    bootstrap_service_memory();

    let fixture = create_active_magic_session(
        "service-magic-flush-replay",
        0,
        vec!["sour_sorcery".to_string()],
        4,
    );
    let learn_nonce = "nonce:service-magic-flush-replay:learn".to_string();
    let learned = champion_magic::learn_champion_spell(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        learn_nonce.clone(),
    )
    .expect("runtime spell learn should apply");
    assert_eq!(learned.status, CommandStatus::Applied);
    let learned_replay = champion_magic::learn_champion_spell(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        learn_nonce.clone(),
    )
    .expect("runtime spell learn replay should return");
    assert_eq!(learned_replay.status, CommandStatus::Applied);
    assert_eq!(learned_replay.command_id, learned.command_id);

    let cast_nonce = "nonce:service-magic-flush-replay:cast".to_string();
    let cast = champion_magic::cast_adventure_spell(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        cast_nonce.clone(),
    )
    .expect("runtime-learned spell cast should apply");
    assert_eq!(cast.status, CommandStatus::Applied);
    let cast_replay = champion_magic::cast_adventure_spell(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        cast_nonce.clone(),
    )
    .expect("runtime cast replay should return");
    assert_eq!(cast_replay.status, CommandStatus::Applied);
    assert_eq!(cast_replay.command_id, cast.command_id);

    assert_public_event_sequences_unique(fixture.caller, &fixture.session_id);
    restore_session_turn_runtime_after_upgrade();
    let restored_learn_replay = champion_magic::learn_champion_spell(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        learn_nonce.clone(),
    )
    .expect("runtime spell learn replay should survive upgrade restore");
    assert_eq!(restored_learn_replay.command_id, learned.command_id);
    let restored_cast_replay = champion_magic::cast_adventure_spell(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        cast_nonce.clone(),
    )
    .expect("runtime spell cast replay should survive upgrade restore");
    assert_eq!(restored_cast_replay.command_id, cast.command_id);

    let flushed = flush_barrier::flush_barrier(flush_barrier::FLUSH_BARRIER_UPGRADE)
        .expect("champion magic upgrade flush should succeed");
    assert!(flushed > 0);
    let projection = session_turn_runtime::projection_diagnostic_snapshot();
    for kernel in projection
        .kernels
        .iter()
        .filter(|kernel| kernel.session_id == fixture.session_id)
    {
        assert_eq!(kernel.dirty_queue_len, 0);
        assert_eq!(kernel.lag_generations, 0);
        assert_eq!(kernel.pending_entries, 0);
    }
    assert!(
        champions_artifacts::find_champion_spell(fixture.champion.id(), fixture.spite_march.id())
            .expect("flushed champion spell lookup should not fail")
            .is_some(),
        "learned runtime spell should be durable after projection flush"
    );
    session_turn_runtime::remove_runtime(&fixture.session_id, fixture.session.current_turn)
        .expect("flushed runtime should be removable for durable replay coverage");
    assert_public_event_sequences_unique(fixture.caller, &fixture.session_id);

    let durable_learn_replay = champion_magic::learn_champion_spell(
        fixture.caller,
        fixture.session_id.clone(),
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        learn_nonce,
    )
    .expect("durable spell learn replay should return");
    assert_eq!(durable_learn_replay.status, CommandStatus::Applied);
    assert_eq!(durable_learn_replay.command_id, learned.command_id);

    let durable_cast_replay = champion_magic::cast_adventure_spell(
        fixture.caller,
        fixture.session_id,
        fixture.champion.id().to_string(),
        "spite-march".to_string(),
        cast_nonce,
    )
    .expect("durable cast replay should return");
    assert_eq!(durable_cast_replay.status, CommandStatus::Applied);
    assert_eq!(durable_cast_replay.command_id, cast.command_id);
}

#[test]
fn start_session_inline_setup_replays_original_nonce() {
    bootstrap_service_memory();

    let (player_one, _player_two, session_id) =
        create_ready_two_player_lobby("service-setup-replay");
    let start_nonce = "nonce:service-setup-replay:start".to_string();
    let first_start =
        account_lobby_session::start_session(player_one, session_id.clone(), start_nonce.clone())
            .expect("first start should not trap");
    assert_eq!(first_start.status, CommandStatus::Applied);
    match &first_start.result {
        LobbyCommandResult::Session(session) => assert_eq!(session.state, "active"),
        other => panic!("start_session returned unexpected result: {other:?}"),
    }

    let session_key = Ulid::from_str(&session_id).expect("service session ids are Ulids");
    let session_row_id = Id::<GameSession>::from_key(session_key);
    let setup_command = commands_events_effects::find_game_command_by_idempotency(
        session_row_id,
        "system",
        "setup",
        command_response::nonce_u64("setup_session", &session_id),
    )
    .expect("setup command lookup should not fail")
    .expect("setup command should exist after first start");
    assert_eq!(setup_command.status, "applied");
    assert!(
        commands_events_effects::find_command_effect(setup_command.id(), "seed_ruleset_content")
            .expect("first setup effect lookup should not fail")
            .is_some()
    );
    assert!(
        commands_events_effects::find_command_effect(setup_command.id(), "seed_participants")
            .expect("second setup effect lookup should not fail")
            .is_some()
    );
    let progress = account_lobby_session::get_setup_progress(session_id.clone())
        .expect("setup progress should be readable after inline setup");
    assert_eq!(progress.session_id, session_id);
    assert_eq!(progress.session_state, "active");
    assert!(progress.setup_complete);
    assert!(progress.completed_effect_count > 1);
    assert_eq!(progress.next_effect_key, None);
    assert_eq!(progress.setup_command_status.as_deref(), Some("applied"));

    let setup_job_key = format!("setup_session:{session_id}");
    assert!(
        system_job_repo::find_system_job_by_key(&setup_job_key)
            .expect("setup job lookup should not fail")
            .is_none(),
        "inline setup must not leave a setup system job"
    );
    let jobs_for_command =
        system_job_repo::page_system_jobs_by_command(setup_command.id(), 10, None)
            .expect("setup jobs should page by command");
    assert!(jobs_for_command.items.is_empty());

    let replay = account_lobby_session::start_session(player_one, session_id.clone(), start_nonce)
        .expect("start replay should not trap");
    assert_eq!(replay.command_id, first_start.command_id);
    match &replay.result {
        LobbyCommandResult::Session(session) => assert_eq!(session.state, "active"),
        other => panic!("start_session replay returned unexpected result: {other:?}"),
    }

    let replayed_setup_command = commands_events_effects::find_game_command_by_idempotency(
        session_row_id,
        "system",
        "setup",
        command_response::nonce_u64("setup_session", &session_id),
    )
    .expect("setup command replay lookup should not fail")
    .expect("setup command should still exist after replay");
    assert_eq!(replayed_setup_command.id(), setup_command.id());
    let replayed_progress = account_lobby_session::get_setup_progress(session_id.clone())
        .expect("setup progress should still be readable after replay");
    assert_eq!(
        replayed_progress.completed_effect_count,
        progress.completed_effect_count
    );
    assert_eq!(replayed_progress.next_effect_key, progress.next_effect_key);
    assert!(
        commands_events_effects::find_command_effect(setup_command.id(), "seed_participants")
            .expect("second setup effect replay lookup should not fail")
            .is_some(),
        "replaying the original start nonce must keep setup complete"
    );
    assert!(
        system_job_repo::find_system_job_by_key(&setup_job_key)
            .expect("setup job replay lookup should not fail")
            .is_none(),
        "setup replay must not create a deferred setup job"
    );

    let applying_setup_commands = commands_events_effects::page_game_commands_by_session_status(
        session_row_id,
        "applying",
        10,
        None,
    )
    .expect("setup commands should page by status");
    assert_eq!(
        applying_setup_commands
            .items
            .iter()
            .filter(|command| command.command_type == "setup_session")
            .count(),
        0
    );
}

#[test]
fn lobby_session_setup_recovers_from_starting_state_and_replays_nonce() {
    bootstrap_service_memory();

    let player_one = Principal::self_authenticating(b"service-19d-player-one");
    let player_two = Principal::self_authenticating(b"service-19d-player-two");

    let registered = account_lobby_session::register_player(
        player_one,
        Some("service-19d-one".to_string()),
        Some("Service One".to_string()),
        "nonce:service:register:one".to_string(),
    )
    .expect("player one registration should not trap");
    assert_eq!(registered.status, CommandStatus::Applied);
    let replay = account_lobby_session::register_player(
        player_one,
        Some("service-19d-one".to_string()),
        Some("Service One".to_string()),
        "nonce:service:register:one".to_string(),
    )
    .expect("registration replay should not trap");
    assert_eq!(replay.command_id, registered.command_id);

    account_lobby_session::register_player(
        player_two,
        Some("service-19d-two".to_string()),
        Some("Service Two".to_string()),
        "nonce:service:register:two".to_string(),
    )
    .expect("player two registration should not trap");

    let created = account_lobby_session::create_session(
        player_one,
        "Service 19D Match".to_string(),
        FIRST_PLAYABLE_RULESET_ID.to_string(),
        19_004,
        "nonce:service:create".to_string(),
    )
    .expect("session creation should not trap");
    let session_id = match created.result {
        LobbyCommandResult::Session(session) => session.session_id,
        other => panic!("create_session returned unexpected result: {other:?}"),
    };

    account_lobby_session::join_session(
        player_two,
        session_id.clone(),
        "faction:ashen-ledger".to_string(),
        "nonce:service:join".to_string(),
    )
    .expect("join should not trap");
    account_lobby_session::mark_ready(
        player_one,
        session_id.clone(),
        "nonce:service:ready:one".to_string(),
    )
    .expect("player one ready should not trap");
    account_lobby_session::mark_ready(
        player_two,
        session_id.clone(),
        "nonce:service:ready:two".to_string(),
    )
    .expect("player two ready should not trap");

    let session_key = Ulid::from_str(&session_id).expect("service session ids are Ulids");
    let mut session = sessions::load_session(Id::from_key(session_key))
        .expect("session load should not fail")
        .expect("session row should exist");
    session.state = "starting".to_string();
    sessions::update_session(session).expect("starting state update should persist");

    let final_start_nonce = "nonce:service:start:inline".to_string();
    let started = account_lobby_session::start_session(
        player_one,
        session_id.clone(),
        final_start_nonce.clone(),
    )
    .expect("start recovery should not trap");
    assert_eq!(started.status, CommandStatus::Applied);
    match &started.result {
        LobbyCommandResult::Session(session) => assert_eq!(session.state, "active"),
        other => panic!("start_session returned unexpected result: {other:?}"),
    }
    let started_command_id = started.command_id.clone();

    let start_replay =
        account_lobby_session::start_session(player_one, session_id.clone(), final_start_nonce)
            .expect("start replay should not trap");
    assert_eq!(start_replay.command_id, started_command_id);

    let started_session_id = session_id.clone();
    let participant_two = account_lobby_session::get_my_participant(player_two, session_id)
        .expect("participant two should be readable after recovery");
    assert_eq!(participant_two.slot_index, 1);
    assert!(participant_two.ready);

    let session_key = Ulid::from_str(&started_session_id).expect("service session ids are Ulids");
    let session_id = Id::from_key(session_key);
    let closing_job = system_job_repo::create_system_job(system_job_repo::SystemJobDraft {
        job_key: format!("test:turn_resolution:{started_session_id}:1"),
        job_kind: "turn_resolution".to_string(),
        session_id,
        battle_id: None,
        turn_number: Some(1),
        due_at: Timestamp::now(),
        command_id: None,
        cursor_json: None,
    })
    .expect("turn-resolution job seed should persist");
    let late_nonce = "nonce:service:move:late-closing".to_string();
    let late_move = movement_service::submit_move_intent(
        player_one,
        started_session_id.clone(),
        "champion:west".to_string(),
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        late_nonce.clone(),
        1_000,
    )
    .expect_err("accepted turn job should block new old-turn movement before command creation");
    assert_eq!(late_move.code, "backend_work_pending");
    let missing_late_status = event_service::get_command_status_by_nonce(
        player_one,
        started_session_id.clone(),
        "submit_move_intent".to_string(),
        late_nonce,
    )
    .expect_err("pre-command late movement denial should not leave a command row");
    assert_eq!(missing_late_status.code, "command_status_not_found");
    system_job_repo::complete_system_job(closing_job)
        .expect("test turn-resolution job should be cleared before normal movement");

    let move_nonce = "nonce:service:move:wood".to_string();
    let moved = movement_service::submit_move_intent(
        player_one,
        started_session_id.clone(),
        "champion:west".to_string(),
        vec![MoveCoord::new(9, 24), MoveCoord::new(9, 23)],
        move_nonce.clone(),
        1_000,
    )
    .expect("movement intent should submit against seeded IcyDB rows");
    assert_eq!(moved.status, CommandStatus::Applied);

    let participant_one =
        account_lobby_session::get_my_participant(player_one, started_session_id.clone())
            .expect("participant one should be readable before movement recovery");
    let participant_id = Id::<GameParticipant>::from_key(
        Ulid::from_str(&participant_one.participant_id).expect("participant id should be Ulid"),
    );

    let dirty_after_movement_submit = session_turn_runtime::projection_diagnostic_snapshot();
    assert!(
        dirty_after_movement_submit.total_dirty_queue_len > 0,
        "runtime movement submission should leave dirty projection work before the first barrier"
    );
    let dirty_kernel = dirty_after_movement_submit
        .kernels
        .iter()
        .find(|kernel| kernel.session_id == started_session_id && kernel.turn_number == 1)
        .expect("dirty runtime projection kernel should exist for the active turn");
    assert!(
        dirty_kernel.lag_generations > 0,
        "runtime projection kernel should report lag before flush"
    );

    let sync_nonce = "nonce:service:sync:wood".to_string();
    let sync_payload_json = format!(r#"{{"session_id":"{started_session_id}"}}"#);
    let seeded_sync_command = commands_events_effects::create_game_command(
        session_id,
        "player".to_string(),
        participant_one.participant_id.clone(),
        None,
        Some(participant_id),
        None,
        1,
        command_response::nonce_u64("sync_session_turn", &sync_nonce),
        "sync_session_turn".to_string(),
        command_response::payload_hash(
            "sync_session_turn",
            &participant_one.participant_id,
            &sync_nonce,
            &sync_payload_json,
        ),
        sync_payload_json,
    )
    .expect("pending sync command seed should persist");

    let partial_sync = movement_service::sync_session_turn(
        player_one,
        started_session_id.clone(),
        u64::MAX,
        sync_nonce.clone(),
    )
    .expect("turn sync should write movement snapshots");
    assert_eq!(
        partial_sync.status,
        CommandStatus::Applied,
        "{partial_sync:?}"
    );
    assert_eq!(
        partial_sync.command_id,
        seeded_sync_command.id().to_string()
    );
    let first_sync_completed = partial_sync
        .events
        .iter()
        .any(|event| event.event_type == "session_turn_synced");
    let mut finished_sync_command_id = None;
    let mut finished_sync_nonce = None;
    if !first_sync_completed {
        assert!(
            partial_sync
                .events
                .iter()
                .any(|event| event.event_type == "movement_sync_incomplete"),
            "partial movement sync should report continuation work"
        );

        let finish_sync_nonce = "nonce:service:sync:wood:finish".to_string();
        let finished_sync = movement_service::sync_session_turn(
            player_one,
            started_session_id.clone(),
            u64::MAX,
            finish_sync_nonce.clone(),
        )
        .expect("second turn sync should finish the two-step movement");
        assert_eq!(
            finished_sync.status,
            CommandStatus::Applied,
            "{finished_sync:?}"
        );
        finished_sync_command_id = Some(finished_sync.command_id.clone());
        finished_sync_nonce = Some(finish_sync_nonce);
    }

    let session_key = Ulid::from_str(&started_session_id).expect("service session ids are Ulids");
    let session_id = Id::from_key(session_key);
    let champion = champions_artifacts::find_champion_by_session_xy(session_id, 9, 23)
        .expect("champion lookup should not fail")
        .expect("champion should have moved to the resource pile");
    let object_page = game_view_service::get_visible_objects(
        player_one,
        started_session_id.clone(),
        Viewport::new(0, 16, 24, 24),
        None,
        32,
    )
    .expect("visible objects should hydrate from live rows after movement");
    let champion_object = object_page
        .objects
        .iter()
        .find(|object| {
            object.subject_kind == "champion" && object.subject_id_text == "champion:west"
        })
        .expect("visible object projection should include the moved champion");
    assert_eq!((champion_object.x, champion_object.y), (9, 23));
    assert!(
        object_page
            .objects
            .iter()
            .all(|object| object.subject_id_text != "pile:west-wood-1"),
        "collected resource piles must not render as available objects"
    );

    let clean_after_projection_flush = session_turn_runtime::projection_diagnostic_snapshot();
    assert_eq!(clean_after_projection_flush.total_dirty_queue_len, 0);
    let clean_kernels = clean_after_projection_flush
        .kernels
        .iter()
        .filter(|kernel| kernel.session_id == started_session_id)
        .collect::<Vec<_>>();
    for clean_kernel in clean_kernels {
        assert_eq!(clean_kernel.dirty_queue_len, 0);
        assert_eq!(clean_kernel.lag_generations, 0);
        assert_eq!(clean_kernel.pending_entries, 0);
    }
    let flushed_session = sessions::load_session(session_id)
        .expect("session reload after movement sync should not fail")
        .expect("session should exist after movement sync");
    let had_runtime_before_eviction =
        session_turn_runtime::contains_runtime(&started_session_id, 1)
            || session_turn_runtime::contains_runtime(
                &started_session_id,
                flushed_session.current_turn,
            );
    let removed_turn_one = session_turn_runtime::remove_runtime(&started_session_id, 1);
    let removed_current_turn = if flushed_session.current_turn != 1 {
        session_turn_runtime::remove_runtime(&started_session_id, flushed_session.current_turn)
    } else {
        None
    };
    if had_runtime_before_eviction {
        assert!(
            removed_turn_one.is_some() || removed_current_turn.is_some(),
            "runtime should be evictable after durable projection flush"
        );
    }
    assert!(
        !session_turn_runtime::contains_runtime(&started_session_id, 1),
        "durable fallback assertions must run without the movement turn runtime"
    );
    assert!(
        !session_turn_runtime::contains_runtime(&started_session_id, flushed_session.current_turn),
        "durable fallback assertions must run without the current active turn runtime"
    );

    let seeded_sync_command_id = seeded_sync_command.id().to_string();
    let mut status_checks = vec![
        (
            event_service::get_command_status(
                player_one,
                started_session_id.clone(),
                moved.command_id.clone(),
            )
            .expect("movement command status should read back after runtime eviction"),
            moved.command_id.clone(),
            "submit_move_intent",
        ),
        (
            event_service::get_command_status_by_nonce(
                player_one,
                started_session_id.clone(),
                "submit_move_intent".to_string(),
                move_nonce.clone(),
            )
            .expect("movement command nonce should read back after runtime eviction"),
            moved.command_id.clone(),
            "submit_move_intent",
        ),
        (
            event_service::get_command_status(
                player_one,
                started_session_id.clone(),
                seeded_sync_command_id.clone(),
            )
            .expect("partial sync command status should read back after runtime eviction"),
            seeded_sync_command_id.clone(),
            "sync_session_turn",
        ),
        (
            event_service::get_command_status_by_nonce(
                player_one,
                started_session_id.clone(),
                "sync_session_turn".to_string(),
                sync_nonce.clone(),
            )
            .expect("partial sync nonce should read back after runtime eviction"),
            seeded_sync_command_id.clone(),
            "sync_session_turn",
        ),
    ];
    if let (Some(command_id), Some(nonce)) = (finished_sync_command_id, finished_sync_nonce) {
        status_checks.push((
            event_service::get_command_status(
                player_one,
                started_session_id.clone(),
                command_id.clone(),
            )
            .expect("finished sync command status should read back after runtime eviction"),
            command_id.clone(),
            "sync_session_turn",
        ));
        status_checks.push((
            event_service::get_command_status_by_nonce(
                player_one,
                started_session_id.clone(),
                "sync_session_turn".to_string(),
                nonce,
            )
            .expect("finished sync nonce should read back after runtime eviction"),
            command_id,
            "sync_session_turn",
        ));
    }
    for (status, expected_command_id, expected_kind) in status_checks {
        assert_eq!(status.command_id, expected_command_id);
        assert_eq!(status.status, CommandStatus::Applied);
        assert_eq!(status.phase, CommandPhase::Complete);
        assert!(
            status
                .result_json
                .as_deref()
                .is_some_and(|json| json.contains(expected_kind)),
            "durable command receipt should retain the command result JSON for {expected_kind}"
        );
    }

    let durable_events = event_service::get_events_after(
        player_one,
        started_session_id.clone(),
        "public".to_string(),
        0,
        100,
    )
    .expect("event feed should read durable movement events after runtime eviction");
    let expected_events = if first_sync_completed {
        vec![
            "movement_intent_submitted",
            "resource_picked_up",
            "session_turn_synced",
        ]
    } else {
        vec![
            "movement_intent_submitted",
            "movement_sync_incomplete",
            "resource_picked_up",
        ]
    };
    for expected_event in expected_events {
        assert!(
            durable_events
                .events
                .iter()
                .any(|event| event.event_type == expected_event),
            "durable event feed should include {expected_event}"
        );
    }

    let durable_intent = movement_repo::find_movement_intent(session_id, champion.id(), 1)
        .expect("durable movement intent lookup should not fail")
        .expect("flushed runtime movement intent should exist durably");
    assert_eq!(durable_intent.status, "resolved");
    assert_eq!(
        durable_intent.command_id,
        Ulid::from_str(&moved.command_id).unwrap()
    );
    assert_eq!(durable_intent.actor_participant_id, participant_id.key());
    assert_eq!(durable_intent.champion_id, champion.id().key());
    assert!(
        durable_intent.path_json.contains("9,23"),
        "durable movement intent should retain the submitted path"
    );

    let snapshots = movement_repo::page_movement_snapshots_for_champion_turn(
        session_id,
        1,
        champion.id(),
        10,
        None,
    )
    .expect("movement snapshots should page through typed IcyDB rows after runtime eviction");
    assert!(
        snapshots
            .items
            .iter()
            .any(|snapshot| snapshot.outcome == "stopped_object_interaction"),
        "turn sync should persist a first-class movement snapshot row for object stops"
    );

    let durable_champion = champions_artifacts::load_champion(champion.id())
        .expect("durable champion lookup should not fail")
        .expect("durable champion should exist after movement");
    assert_eq!((durable_champion.x, durable_champion.y), (9, 23));
    let durable_occupancy = map_visibility_occupancy::find_occupancy_by_occupant(
        session_id,
        "champion",
        &champion.id().to_string(),
        0,
    )
    .expect("durable occupancy lookup should not fail")
    .expect("champion occupancy should survive projection flush");
    assert_eq!((durable_occupancy.x, durable_occupancy.y), (9, 23));
    assert!(durable_occupancy.blocking);

    let durable_participant = sessions::load_participant(participant_id)
        .expect("durable participant lookup should not fail")
        .expect("participant should exist after movement resource pickup");
    assert!(
        durable_participant.wood > participant_one.resources.wood,
        "resource pile pickup should flush participant resources durably"
    );
    assert!(
        durable_participant.last_resource_command_id.is_some(),
        "resource pickup should leave a durable participant resource command marker"
    );

    let durable_pile = map_visibility_occupancy::find_world_object_by_session_xy(session_id, 9, 23)
        .expect("durable world object lookup should not fail")
        .expect("resource pile should remain readable after collection");
    assert_eq!(durable_pile.state, "collected");
    assert_eq!(durable_pile.scoring_kind, "resource_pile");
    assert_eq!(durable_pile.last_visited_turn, 1);

    let durable_quest =
        scenario_repo::find_quest_by_participant_key(session_id, participant_id, OPENING_QUEST_KEY)
            .expect("durable opening quest lookup should not fail")
            .expect("opening quest row should remain readable after movement projection flush");
    assert_eq!(durable_quest.quest_key, OPENING_QUEST_KEY);
    let durable_rule =
        scenario_repo::find_scenario_rule_by_key(session_id, "rule:central-objectives")
            .expect("durable scenario rule lookup should not fail")
            .expect("scenario rule row should remain readable after movement projection flush");
    assert_eq!(durable_rule.rule_key, "rule:central-objectives");

    let durable_object_page = game_view_service::get_visible_objects(
        player_one,
        started_session_id.clone(),
        Viewport::new(0, 16, 24, 24),
        None,
        32,
    )
    .expect("visible objects should hydrate from durable rows after runtime eviction");
    let durable_champion_object = durable_object_page
        .objects
        .iter()
        .find(|object| {
            object.subject_kind == "champion" && object.subject_id_text == "champion:west"
        })
        .expect("durable visible object projection should include the moved champion");
    assert_eq!(
        (durable_champion_object.x, durable_champion_object.y),
        (9, 23)
    );
    assert!(
        durable_object_page
            .objects
            .iter()
            .all(|object| object.subject_id_text != "pile:west-wood-1"),
        "durable visible object reads must hide collected resource piles"
    );
    let durable_chunks = game_view_service::get_visible_map_chunks(
        player_one,
        started_session_id.clone(),
        Viewport::new(0, 16, 24, 24),
        None,
        9,
    )
    .expect("visible map chunks should hydrate from durable rows after runtime eviction");
    assert!(
        !durable_chunks.chunks.is_empty(),
        "durable map visibility chunks should remain readable"
    );
    let known_objects = map_visibility_occupancy::page_known_objects_for_participant(
        session_id,
        participant_id,
        32,
        None,
    )
    .expect("known objects should page from durable rows after runtime eviction");
    assert!(
        known_objects
            .items
            .iter()
            .any(|object| object.subject_kind == "town" && object.subject_id_text == "town:west"),
        "durable known-object rows should remain readable after runtime eviction"
    );

    let stop_after_durable_projection_coverage = true;
    if stop_after_durable_projection_coverage {
        return;
    }

    let mut continuation_session = sessions::load_session(session_id)
        .expect("session reload for guarded movement continuation should not fail")
        .expect("session should exist before guarded movement continuation");
    session_turn_runtime::ensure_active_turn_runtime(&mut continuation_session)
        .expect("guarded movement continuation should start from a fresh active-turn runtime");

    let session_row = sessions::load_session(session_id)
        .expect("session reload should not fail")
        .expect("session should exist for spell setup");
    let hex_spark =
        content::find_spell_by_ruleset_slug(Id::from_key(session_row.ruleset_id), "hex-spark")
            .expect("spell lookup should not fail")
            .expect("hex spark should be seeded");
    let mut spell_champion = champion.clone();
    if !spell_champion
        .skill_keys
        .iter()
        .any(|key| key == "sour_sorcery")
    {
        spell_champion.skill_keys.push("sour_sorcery".to_string());
        spell_champion.skill_keys.sort();
        spell_champion.skill_keys.dedup();
        spell_champion.last_command_id = Some(seeded_sync_command.id().key());
        champions_artifacts::update_champion(spell_champion)
            .expect("battle spell skill should persist before battle stack creation");
    }
    champions_artifacts::create_champion_spell(
        session_id,
        champion.id(),
        hex_spark.id(),
        "hex-spark",
        session_row.current_turn,
        seeded_sync_command.id(),
    )
    .expect("learned battle spell should persist before battle stack creation");

    let guarded = movement_service::submit_move_intent(
        player_one,
        started_session_id.clone(),
        champion.id().to_string(),
        vec![
            MoveCoord::new(10, 23),
            MoveCoord::new(11, 23),
            MoveCoord::new(12, 23),
            MoveCoord::new(12, 22),
        ],
        "nonce:service:move:guarded".to_string(),
        122_000,
    )
    .expect("guarded movement intent should submit");
    assert_eq!(guarded.status, CommandStatus::Applied);

    let mut guarded_sync = None;
    let mut saw_partial_guarded_sync = false;
    for step in 0..6 {
        let synced = movement_service::sync_session_turn(
            player_one,
            started_session_id.clone(),
            u64::MAX,
            format!("nonce:service:sync:guarded:{step}"),
        )
        .expect("guarded movement sync should progress");
        saw_partial_guarded_sync |= synced
            .events
            .iter()
            .any(|event| event.event_type == "movement_sync_incomplete");
        if synced.events.iter().any(|event| {
            event.event_type == "neutral_encounter_pending"
                && event
                    .payload
                    .as_deref()
                    .is_some_and(|payload| payload.contains("\"battle_id\""))
        }) {
            guarded_sync = Some(synced);
            break;
        }
    }
    assert!(
        saw_partial_guarded_sync,
        "guarded movement should park at least one partial sync slice"
    );
    let guarded_sync = guarded_sync.expect("guarded movement should start neutral battle");
    assert!(guarded_sync.events.iter().any(|event| {
        event.event_type == "neutral_encounter_pending"
            && event
                .payload
                .as_deref()
                .is_some_and(|payload| payload.contains("\"battle_id\""))
    }));

    let champion_after_guard = champions_artifacts::load_champion(champion.id())
        .expect("champion reload should not fail")
        .expect("champion should still exist");
    assert_eq!(champion_after_guard.status, "in_battle");
    let battle_id = champion_after_guard
        .in_battle_id
        .map(Id::<Battle>::from_key)
        .expect("neutral contact should set champion battle id");
    let battle = battles::find_battle_by_attacker(champion.id())
        .expect("battle lookup should not fail")
        .expect("neutral contact should persist battle row");
    assert_eq!(battle.id(), battle_id);
    assert_eq!(battle.battle_type, "neutral");
    assert_eq!(battle.state, "active");
    assert_eq!(battle.attacker_champion_id, Some(champion.id().key()));
    assert!(battle.defender_neutral_army_id.is_some());
    assert!(battle.active_stack_id.is_some());
    assert!(
        !battles::page_battle_stacks_by_side(battle_id, "attacker", 10, None)
            .expect("attacker battle stacks should page")
            .items
            .is_empty()
    );
    assert!(
        !battles::page_battle_stacks_by_side(battle_id, "defender", 10, None)
            .expect("defender battle stacks should page")
            .items
            .is_empty()
    );

    let battle_id_text = battle_id.to_string();
    let own_battle = battle_service::get_battle_state(
        player_one,
        started_session_id.clone(),
        battle_id_text.clone(),
        0,
    )
    .expect("involved participant should see neutral battle tactics");
    assert_eq!(own_battle.battle_type, "neutral");
    assert!(!own_battle.legal_actions_for_caller.is_empty());
    let cast_action = own_battle
        .legal_actions_for_caller
        .iter()
        .find(|action| {
            action.action == "CastAbility"
                && action.ability_key.as_deref() == Some("spell:hex-spark")
        })
        .expect("learned battle spell should appear as CastAbility");
    assert!(cast_action.enabled);
    assert!(!cast_action.targets.is_empty());
    assert!(own_battle.legal_actions_for_caller.iter().any(|action| {
        action.action == "Retreat"
            && !action.enabled
            && action.disabled_reason.as_deref() == Some("retreat_deferred_v1_no_rehire_flow")
    }));
    assert!(own_battle.legal_actions_for_caller.iter().any(|action| {
        action.action == "Surrender"
            && !action.enabled
            && action.disabled_reason.as_deref() == Some("surrender_deferred_v1_no_payment_terms")
    }));

    let denied = battle_service::get_battle_state(
        player_two,
        started_session_id.clone(),
        battle_id_text.clone(),
        0,
    )
    .expect_err("uninvolved participant must not see neutral battle tactics");
    assert_eq!(denied.code, "battle_not_visible");

    let battle_action = battle_service::submit_battle_action(
        player_one,
        started_session_id.clone(),
        first_enabled_battle_action(&own_battle),
        "nonce:service:battle:privacy:action".to_string(),
        0,
    )
    .expect("involved participant should submit a battle action");
    assert_eq!(battle_action.status, CommandStatus::Applied);

    let private_events = event_service::get_events_after(
        player_one,
        started_session_id.clone(),
        format!("participant:{}", participant_one.participant_id),
        0,
        50,
    )
    .expect("participant battle event feed should load");
    let private_action = private_events
        .events
        .iter()
        .find(|event| event.event_type == "battle_action_applied")
        .expect("participant feed should include detailed battle action event");
    let private_payload = private_action
        .payload
        .as_deref()
        .expect("private battle event should include payload");
    assert!(private_payload.contains("subject_id_text"));
    assert!(private_payload.contains(r#""payload":"#));

    let public_events = event_service::get_events_after(
        player_one,
        started_session_id.clone(),
        "public".to_string(),
        0,
        50,
    )
    .expect("public battle event feed should load");
    let public_action = public_events
        .events
        .iter()
        .find(|event| event.event_type == "battle_action_applied")
        .expect("public feed should include redacted battle action event");
    let public_payload = public_action
        .payload
        .as_deref()
        .expect("public battle event should include redacted payload");
    assert!(public_payload.contains(r#""redacted":true"#));
    assert!(!public_payload.contains("subject_id_text"));
    assert!(!public_payload.contains("stack_id"));

    let forbidden_participant_one_events = event_service::get_events_after(
        player_two,
        started_session_id,
        format!("participant:{}", participant_one.participant_id),
        0,
        50,
    )
    .expect_err("participant event audiences must not be readable by opponents");
    assert_eq!(
        forbidden_participant_one_events.code,
        "audience_not_allowed"
    );
}

fn first_enabled_battle_action(view: &BattleView) -> BattleActionInput {
    let active_stack_id = view
        .active_stack_id
        .clone()
        .expect("active battle should have an active stack");
    let action = view
        .legal_actions_for_caller
        .iter()
        .find(|action| action.enabled && action.action == "Defend")
        .or_else(|| {
            view.legal_actions_for_caller
                .iter()
                .find(|action| action.enabled)
        })
        .expect("battle view should expose an enabled action");
    BattleActionInput {
        battle_id: view.battle_id.clone(),
        battle_stack_id: active_stack_id,
        action: action.action.clone(),
        ability_key: action.ability_key.clone(),
        target_stack_id: action.targets.first().cloned(),
        destination: action.path.first().copied(),
    }
}
