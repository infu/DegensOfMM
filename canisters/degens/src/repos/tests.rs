use domm_degens_schema::schema::{Battle, Champion, GameSession, PlayerAccount, SystemJob};
use icydb::{
    db::query::FieldRef,
    traits::EntityValue,
    types::{Id, Principal, Timestamp, Ulid},
};

use super::{
    aftermath_history, battle_round_ready, battles, champions_artifacts, cleanup,
    commands_events_effects, content, economy_expansion, foundation, map_visibility_occupancy,
    movement, players, scenario_progress, sessions, system_jobs, towns, turn_ready, worldgen,
};

fn bootstrap_repo_memory() {
    icydb::__reexports::canic_memory::api::MemoryApi::bootstrap_owner_range(
        "domm-degens-canister",
        20,
        120,
    )
    .expect("repository tests should reserve the generated canister memory range");
}

#[test]
fn repository_create_read_update_page_delete_smoke() {
    bootstrap_repo_memory();

    let player = players::create_player_account(
        Principal::dummy(31),
        Some("repo19c_player".to_string()),
        Some("Repo 19C".to_string()),
    )
    .expect("player create should use typed IcyDB create");
    let ruleset = content::create_ruleset_definition(
        "repo19c_rules".to_string(),
        1,
        "Repo 19C Rules".to_string(),
        None,
        Some("repo19c_hash".to_string()),
    )
    .expect("ruleset create should use typed IcyDB create");
    let faction = content::create_faction_definition(
        ruleset.id(),
        "repo19c_faction".to_string(),
        "Repo 19C Faction".to_string(),
        "repo19c_trait".to_string(),
    )
    .expect("faction create should use typed IcyDB create");

    let mut first = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        "Repo 19C Session A".to_string(),
        19_001,
        16,
        16,
        Timestamp::from_millis(1_900_100),
    )
    .expect("session create should use typed IcyDB create");
    let mut second = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        "Repo 19C Session B".to_string(),
        19_002,
        16,
        16,
        Timestamp::from_millis(1_900_200),
    )
    .expect("second session create should use typed IcyDB create");
    let mut third = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        "Repo 19C Session C".to_string(),
        19_003,
        16,
        16,
        Timestamp::from_millis(1_900_300),
    )
    .expect("third session create should use typed IcyDB create");

    first.state = "repo19c".to_string();
    second.state = "repo19c".to_string();
    third.state = "repo19c".to_string();
    let first = foundation::update("tests.session_state_a", first).expect("session update");
    let second = foundation::update("tests.session_state_b", second).expect("session update");
    let third = foundation::update("tests.session_state_c", third).expect("session update");

    let participant =
        sessions::create_participant(first.id(), player.id(), faction.id(), 0, "red".to_string())
            .expect("participant create should use typed IcyDB create");

    let by_principal = players::find_by_principal(player.account_principal)
        .expect("principal lookup should be typed")
        .expect("player should be found");
    assert_eq!(by_principal.id(), player.id());

    let mut updated_player = by_principal;
    updated_player.display_name = Some("Repo 19C Updated".to_string());
    let updated_player = players::update_player_account(updated_player)
        .expect("player update should use typed IcyDB update");
    assert_eq!(
        updated_player.display_name.as_deref(),
        Some("Repo 19C Updated")
    );

    let loaded_session = sessions::load_session(first.id())
        .expect("session load by id should be typed")
        .expect("session should exist");
    assert_eq!(loaded_session.id(), first.id());

    let found_participant = sessions::find_participant_by_session_player(first.id(), player.id())
        .expect("participant lookup should be typed")
        .expect("participant should exist");
    assert_eq!(found_participant.id(), participant.id());

    let page = sessions::page_sessions_by_state("repo19c", 2, None)
        .expect("session page should execute through IcyDB cursor pagination");
    assert_eq!(page.limit, 2);
    assert_eq!(page.items.len(), 2);
    assert!(
        page.next_cursor.is_some(),
        "third row should produce a cursor"
    );

    let next_page = sessions::page_sessions_by_state("repo19c", 2, page.next_cursor)
        .expect("second session page should accept the IcyDB cursor");
    assert!(
        next_page
            .items
            .iter()
            .any(|session| session.id() == third.id())
    );

    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_participant", participant.id())
            .expect("participant delete"),
        1
    );
    for session in [first, second, third] {
        assert_eq!(
            cleanup::delete_row_by_id("tests.delete_session", session.id())
                .expect("session delete"),
            1
        );
    }
    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_faction", faction.id()).expect("faction delete"),
        1
    );
    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_ruleset", ruleset.id()).expect("ruleset delete"),
        1
    );
    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_player", updated_player.id())
            .expect("player delete"),
        1
    );
}

#[test]
fn repository_insert_many_atomic_and_storage_errors_are_sanitized() {
    bootstrap_repo_memory();

    let first = PlayerAccount {
        id: Ulid::from_u128(19_100),
        account_principal: Principal::dummy(41),
        username: Some("repo19c_batch_a".to_string()),
        display_name: None,
        ..Default::default()
    };
    let second = PlayerAccount {
        id: Ulid::from_u128(19_101),
        account_principal: Principal::dummy(42),
        username: Some("repo19c_batch_b".to_string()),
        display_name: None,
        ..Default::default()
    };
    let third = PlayerAccount {
        id: Ulid::from_u128(19_102),
        account_principal: Principal::dummy(43),
        username: Some("repo19c_insert_single".to_string()),
        display_name: None,
        ..Default::default()
    };

    let inserted = foundation::insert_many_atomic("tests.insert_many_players", [first, second])
        .expect("atomic batch insert should persist both rows");
    assert_eq!(inserted.len(), 2);

    let inserted_single =
        foundation::insert("tests.insert_single_player", third).expect("single insert");
    assert_eq!(
        players::find_by_username("repo19c_insert_single")
            .expect("username lookup should execute")
            .expect("inserted single player should exist")
            .id(),
        inserted_single.id()
    );

    let duplicate = players::create_player_account(
        inserted[0].account_principal,
        Some("repo19c_duplicate".to_string()),
        None,
    )
    .expect_err("duplicate principal should map through repository error handling");
    assert_eq!(duplicate.code, "icydb_repository_error");
    assert!(!duplicate.message.contains("account_principal"));
    assert!(duplicate.details_json.is_none());

    for player in inserted {
        assert_eq!(
            cleanup::delete_row_by_id("tests.delete_batch_player", player.id())
                .expect("batch player delete"),
            1
        );
    }
    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_single_player", inserted_single.id())
            .expect("single player delete"),
        1
    );
}

#[test]
fn repository_hot_path_plans_are_indexed_and_bounded() {
    bootstrap_repo_memory();

    let session_id = Id::<GameSession>::from_key(Ulid::from_u128(19_200));
    let player_id = Id::<PlayerAccount>::from_key(Ulid::from_u128(19_201));
    let participant_id =
        Id::<domm_degens_schema::schema::GameParticipant>::from_key(Ulid::from_u128(19_202));
    let champion_id = Id::<Champion>::from_key(Ulid::from_u128(19_203));
    let _battle_id = Id::<Battle>::from_key(Ulid::from_u128(19_204));

    let plans = [
        (
            "principal lookup",
            players::principal_lookup_plan_text(Principal::dummy(51))
                .expect("principal plan should build"),
        ),
        (
            "ruleset lookup",
            content::ruleset_lookup_plan_text("repo19c_rules", 1)
                .expect("ruleset plan should build"),
        ),
        (
            "participant lookup",
            sessions::participant_lookup_plan_text(session_id, player_id)
                .expect("participant plan should build"),
        ),
        (
            "command idempotency",
            commands_events_effects::game_command_idempotency_plan_text(
                session_id,
                "participant",
                "p1",
                77,
            )
            .expect("command plan should build"),
        ),
        (
            "event feed",
            commands_events_effects::event_feed_plan_text(session_id, "participant:p1", 3, 50)
                .expect("event plan should build"),
        ),
        (
            "map chunk",
            map_visibility_occupancy::map_chunk_plan_text(session_id, 1, 2)
                .expect("map chunk plan should build"),
        ),
        (
            "visibility",
            map_visibility_occupancy::visibility_plan_text(participant_id, 1, 2)
                .expect("visibility plan should build"),
        ),
        (
            "town owner",
            towns::towns_by_owner_plan_text(session_id, participant_id, 50)
                .expect("town plan should build"),
        ),
        (
            "champion owner",
            champions_artifacts::champions_by_session_owner_plan_text(
                session_id,
                participant_id,
                "active",
                50,
            )
            .expect("champion plan should build"),
        ),
        (
            "movement intent",
            movement::movement_intent_plan_text(session_id, champion_id, 1)
                .expect("movement plan should build"),
        ),
        (
            "movement snapshot",
            movement::movement_snapshot_plan_text(session_id, 1, champion_id)
                .expect("movement snapshot plan should build"),
        ),
        (
            "objective progress",
            scenario_progress::objective_plan_text(session_id, "objective:north")
                .expect("objective plan should build"),
        ),
        (
            "quest state",
            scenario_progress::quest_plan_text(session_id, participant_id, "quest:opening-ledger")
                .expect("quest plan should build"),
        ),
        (
            "world event",
            scenario_progress::world_event_plan_text(session_id, "week:1")
                .expect("world event plan should build"),
        ),
        (
            "scenario rule",
            scenario_progress::scenario_rule_plan_text(session_id, "active")
                .expect("scenario rule plan should build"),
        ),
        (
            "scenario rule status",
            scenario_progress::scenario_rule_status_plan_text(session_id, "active")
                .expect("scenario rule status plan should build"),
        ),
        (
            "skirmish settings",
            worldgen::skirmish_settings_plan_text(session_id)
                .expect("skirmish settings plan should build"),
        ),
        (
            "procedural map",
            worldgen::procedural_map_plan_text(session_id, "procedural:first-playable-preview")
                .expect("procedural map plan should build"),
        ),
        (
            "naval routes",
            worldgen::naval_routes_plan_text(session_id, "disabled")
                .expect("naval route plan should build"),
        ),
        (
            "siege rules",
            worldgen::siege_rules_plan_text(session_id, "disabled")
                .expect("siege rule plan should build"),
        ),
        (
            "active battles",
            battles::active_battles_plan_text(session_id, "active", 2)
                .expect("battle plan should build"),
        ),
        (
            "battle stacks",
            battles::battle_stacks_plan_text(_battle_id, 50)
                .expect("battle stack plan should build"),
        ),
        (
            "battle obstacles",
            battles::battle_obstacles_plan_text(_battle_id, 50)
                .expect("battle obstacle plan should build"),
        ),
        (
            "battle occupancy",
            battles::battle_occupancy_plan_text(_battle_id, 50)
                .expect("battle occupancy plan should build"),
        ),
        (
            "system jobs due",
            system_jobs::due_jobs_plan_text(Timestamp::from_millis(19_200), 2)
                .expect("system jobs due plan should build"),
        ),
        (
            "turn ready",
            turn_ready::turn_ready_session_turn_plan_text(session_id, 1, 2)
                .expect("turn ready plan should build"),
        ),
        (
            "battle round ready",
            battle_round_ready::battle_round_ready_plan_text(session_id, _battle_id, 1, 2)
                .expect("battle round ready plan should build"),
        ),
        (
            "match history",
            aftermath_history::match_history_plan_text(player_id, 50)
                .expect("history plan should build"),
        ),
    ];

    for (label, plan) in plans {
        assert!(
            plan.contains("Index") || plan.contains("ByKey"),
            "{label} should use an indexed or primary-key access path:\n{plan}"
        );
        assert!(
            !plan.contains("FullScan"),
            "{label} must not plan as a full scan:\n{plan}"
        );
        assert!(
            plan.contains("limit: Some(") || plan.contains("limit=Some"),
            "{label} must carry an explicit bounded limit:\n{plan}"
        );
    }
}

#[test]
fn repository_query_inventory_covers_required_hot_paths() {
    let plans = [
        players::PRINCIPAL_LOOKUP,
        content::RULESET_SLUG_VERSION_LOOKUP,
        sessions::PARTICIPANT_SESSION_PLAYER_LOOKUP,
        commands_events_effects::GAME_COMMAND_IDEMPOTENCY_LOOKUP,
        commands_events_effects::EVENT_FEED_LOOKUP,
        commands_events_effects::COMMAND_EFFECT_SESSION_STATUS_LOOKUP,
        map_visibility_occupancy::MAP_CHUNK_COORD_LOOKUP,
        map_visibility_occupancy::VISIBILITY_CHUNK_LOOKUP,
        map_visibility_occupancy::OCCUPANCY_CELL_LOOKUP,
        map_visibility_occupancy::OCCUPANCY_OCCUPANT_LOOKUP,
        map_visibility_occupancy::KNOWN_OBJECT_PARTICIPANT_LOOKUP,
        map_visibility_occupancy::KNOWN_OBJECT_SUBJECT_LOOKUP,
        map_visibility_occupancy::WORLD_OBJECT_OWNER_SCORING_LOOKUP,
        towns::TOWNS_BY_OWNER_LOOKUP,
        champions_artifacts::CHAMPIONS_BY_SESSION_OWNER_LOOKUP,
        economy_expansion::TAVERN_OFFERS_LOOKUP,
        economy_expansion::TAVERN_OFFER_KEY_LOOKUP,
        economy_expansion::MARKET_TRADE_COMMAND_LOOKUP,
        economy_expansion::DWELLING_POOL_OBJECT_LOOKUP,
        economy_expansion::DWELLING_RECRUIT_COMMAND_LOOKUP,
        scenario_progress::OBJECTIVE_BY_KEY_LOOKUP,
        scenario_progress::OBJECTIVES_BY_STATUS_LOOKUP,
        scenario_progress::QUEST_BY_PARTICIPANT_KEY_LOOKUP,
        scenario_progress::WORLD_EVENT_BY_KEY_LOOKUP,
        scenario_progress::WORLD_EVENTS_BY_WINDOW_LOOKUP,
        scenario_progress::SCENARIO_RULE_BY_KEY_LOOKUP,
        scenario_progress::SCENARIO_RULES_BY_STATE_LOOKUP,
        scenario_progress::SCENARIO_RULES_BY_STATUS_LOOKUP,
        worldgen::SKIRMISH_SETTINGS_BY_SESSION_LOOKUP,
        worldgen::PROCEDURAL_MAP_BY_KEY_LOOKUP,
        worldgen::PROCEDURAL_MAPS_BY_STATUS_LOOKUP,
        worldgen::NAVAL_ROUTE_BY_KEY_LOOKUP,
        worldgen::NAVAL_ROUTES_BY_STATUS_LOOKUP,
        worldgen::SIEGE_RULE_BY_KEY_LOOKUP,
        worldgen::SIEGE_RULES_BY_STATUS_LOOKUP,
        champions_artifacts::CHAMPIONS_BY_OWNER_LOOKUP,
        champions_artifacts::CHAMPION_SPELLS_LOOKUP,
        movement::MOVEMENT_INTENT_UNIQUE_LOOKUP,
        movement::MOVEMENT_SNAPSHOT_UNIQUE_LOOKUP,
        movement::MOVEMENT_SNAPSHOTS_BY_CHAMPION_LOOKUP,
        battles::BATTLES_BY_SESSION_STATE_LOOKUP,
        system_jobs::SYSTEM_JOB_BY_KEY_LOOKUP,
        system_jobs::SYSTEM_JOBS_BY_STATUS_DUE_LOOKUP,
        system_jobs::SYSTEM_JOBS_BY_SESSION_STATUS_DUE_LOOKUP,
        system_jobs::SYSTEM_JOBS_BY_BATTLE_STATUS_DUE_LOOKUP,
        system_jobs::SYSTEM_JOBS_BY_COMMAND_LOOKUP,
        turn_ready::TURN_READY_UNIQUE_LOOKUP,
        turn_ready::TURN_READY_BY_SESSION_TURN_LOOKUP,
        turn_ready::TURN_READY_BY_COMMAND_LOOKUP,
        battle_round_ready::BATTLE_ROUND_READY_UNIQUE_LOOKUP,
        battle_round_ready::BATTLE_ROUND_READY_BY_SESSION_BATTLE_ROUND_LOOKUP,
        battle_round_ready::BATTLE_ROUND_READY_BY_COMMAND_LOOKUP,
        aftermath_history::MATCH_HISTORY_LOOKUP,
    ];

    for plan in plans {
        assert!(!plan.name.is_empty(), "query plan names are audit handles");
        assert!(!plan.entity.is_empty(), "query plan must name its entity");
        assert!(
            !plan.indexed_fields.is_empty(),
            "{} must record the schema index fields it depends on",
            plan.name
        );
        let limit = plan
            .bounded_limit
            .expect("hot-path repository plans must declare bounded limits");
        assert!(
            limit <= domm_game::MAX_LIST_LIMIT
                || limit == domm_game::MAX_ACTIVE_BATTLES_PER_SESSION,
            "{} has an unbounded or excessive limit",
            plan.name
        );
    }
}

#[test]
fn system_job_due_pagination_executes() {
    bootstrap_repo_memory();

    let player = players::create_player_account(
        Principal::dummy(61),
        Some("repo19c_jobs_player".to_string()),
        Some("Repo 19C Jobs".to_string()),
    )
    .expect("player create should use typed IcyDB create");
    let ruleset = content::create_ruleset_definition(
        "repo19c_jobs_rules".to_string(),
        1,
        "Repo 19C Jobs Rules".to_string(),
        None,
        Some("repo19c_jobs_hash".to_string()),
    )
    .expect("ruleset create should use typed IcyDB create");
    let session = sessions::create_game_session(
        ruleset.id(),
        player.id(),
        "Repo 19C Jobs Session".to_string(),
        19_401,
        16,
        16,
        Timestamp::from_millis(1_940_100),
    )
    .expect("session create should use typed IcyDB create");

    let due_at = Timestamp::from_millis(1_940_000);
    let job = system_jobs::create_system_job(system_jobs::SystemJobDraft {
        job_key: "repo19c:job:due".to_string(),
        job_kind: "turn_deadline".to_string(),
        session_id: session.id(),
        battle_id: None,
        turn_number: Some(1),
        due_at,
        command_id: None,
        cursor_json: None,
    })
    .expect("system job create should use typed IcyDB create");

    let db = crate::db();
    let raw_page = db
        .load::<SystemJob>()
        .filter(FieldRef::new("status").eq(system_jobs::STATUS_SCHEDULED))
        .filter(FieldRef::new("due_at").lte(Timestamp::from_millis(1_940_001)))
        .order_asc("id")
        .limit(10)
        .page()
        .expect("raw due system job page builder should succeed");
    let raw_page = raw_page
        .execute()
        .expect("raw due system job page should execute");
    assert!(
        raw_page
            .into_items()
            .iter()
            .any(|item| item.id() == job.id())
    );

    let page = system_jobs::page_due_system_jobs(Timestamp::from_millis(1_940_001), 10, None)
        .expect("due system job pagination should execute");
    assert!(page.items.iter().any(|item| item.id() == job.id()));

    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_job", job.id()).expect("job delete"),
        1
    );
    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_job_session", session.id())
            .expect("session delete"),
        1
    );
    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_job_ruleset", ruleset.id())
            .expect("ruleset delete"),
        1
    );
    assert_eq!(
        cleanup::delete_row_by_id("tests.delete_job_player", player.id()).expect("player delete"),
        1
    );
}

#[test]
fn gameplay_repositories_do_not_use_generic_sql_or_core_db() {
    let repo_sources = [
        include_str!("aftermath_history.rs"),
        include_str!("battles.rs"),
        include_str!("champions_artifacts.rs"),
        include_str!("cleanup.rs"),
        include_str!("commands_events_effects.rs"),
        include_str!("content.rs"),
        include_str!("economy.rs"),
        include_str!("economy_expansion.rs"),
        include_str!("foundation.rs"),
        include_str!("map_visibility_occupancy.rs"),
        include_str!("movement.rs"),
        include_str!("neutrals.rs"),
        include_str!("players.rs"),
        include_str!("scenario_progress.rs"),
        include_str!("sessions.rs"),
        include_str!("system_jobs.rs"),
        include_str!("towns.rs"),
        include_str!("turn_ready.rs"),
        include_str!("battle_round_ready.rs"),
        include_str!("worldgen.rs"),
    ];

    for source in repo_sources {
        assert!(!source.contains("execute_sql"));
        assert!(!source.contains("compile_sql"));
        assert!(!source.contains("core_db("));
        assert!(!source.contains("execute_generated_sql"));
    }
}
