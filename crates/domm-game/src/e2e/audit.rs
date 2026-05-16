use super::types::{SpecAuditRow, SpecAuditStatus};

pub fn part_two_spec_audit() -> Vec<SpecAuditRow> {
    vec![
        implemented(
            "schema and entity model",
            "Part 2 rows, indexes, relation strengths, defaults, migration tests, and deletion ordering are represented in schema and macro tests.",
        ),
        implemented(
            "command/event/recovery core",
            "Commands, effects, pending recovery, event sequence allocation, idempotency, payload hashes, retry behavior, and event feeds are covered.",
        ),
        implemented(
            "deterministic randomness",
            "Gameplay randomness uses keyed hash input helpers with no host entropy or mutable RNG cursor dependency.",
        ),
        implemented(
            "ruleset and first playable content",
            "The first playable manifest, map, factions, units, buildings, objects, neutral seed, walkthrough, and hashes are pinned by tests.",
        ),
        implemented(
            "lobby and session lifecycle",
            "Registration, create/join/ready/start, setup recovery, participant limits, session views, and match-history shell are implemented.",
        ),
        implemented(
            "map, terrain, occupancy, and visibility",
            "Chunks, movement costs, flags, discovered and visible bitsets, known objects, occupancy, redaction, and viewport limits are implemented.",
        ),
        implemented(
            "client and public DTO contract",
            "The probe client and API fixture use public DTOs for game views, map, objects, champions, towns, battles, events, command status, previews, and content.",
        ),
        implemented(
            "resources and lazy economy",
            "Balances, ledgers, income sources, turn summaries, caps, pickup rewards, mine ownership, and bounded materialization are implemented.",
        ),
        implemented(
            "towns, buildings, and recruitment",
            "Town ownership, building commands, previews, recruit pools, recruit commands, target selection, and garrison stacking are implemented.",
        ),
        implemented(
            "champions, armies, artifacts, and strategic state",
            "Champion rows, stack caps, movement state, artifact ownership/capture foundation, DTO redaction, defeat, and progression foundations are implemented.",
        ),
        implemented(
            "effects, abilities, spells, and statuses",
            "The v1 effect dispatch exists with explicit disabled reasons for unsupported spellbook, skill-tree, morale, luck, and complex status behavior.",
        ),
        implemented(
            "movement intents and turn sync",
            "Replaceable intents, movement previews, deterministic microsteps, blockers, movement conflict resolution, object stops, cursors, recovery, and sync budgets are implemented.",
        ),
        implemented(
            "world objects, pickups, mines, and captures",
            "Object visits, once-only rewards, refresh policy, mine income ownership, movement-triggered interactions, and scoreboard rows are implemented.",
        ),
        implemented(
            "neutral armies and encounter starts",
            "Neutral rows, stacks, strength labels, redaction, guard-contact encounter starts, defeat cleanup, and explicit v1 no-roaming/no-join policy are implemented.",
        ),
        implemented(
            "battle engine and battle commands",
            "Battle rows, stack snapshots, tactical occupancy, initiative, legal actions, command idempotency, deadlines, auto-defend, sync, events, and recovery are implemented.",
        ),
        implemented(
            "aftermath, capture, defeat, and victory",
            "Neutral aftermath, town capture, income cutover, champion defeat, artifact capture foundation, victory checks, summaries, and match history are implemented.",
        ),
        implemented(
            "deterministic AI surface",
            "The canister-safe AI draft layer is bounded, deterministic, visible-state based, and command-emitting only.",
        ),
        implemented(
            "cleanup, compaction, and storage limits",
            "Finished-session compaction, retained summaries, occupancy cleanup, raw-log retention, active-session caps, and bounded cleanup retries are implemented.",
        ),
        implemented(
            "performance budgets and query contracts",
            "Hard limits, payload caps, pagination caps, active-session caps, command/event/ledger retention caps, and first-playable measurement output are implemented.",
        ),
        implemented(
            "schema evolution and migration safety",
            "Append-only hot fields, persisted defaults, generated defaults, index ordinals, relation strength, deletion order, and unsupported drift failures are tested.",
        ),
        implemented(
            "playable web client and first playable route",
            "Gate E client coverage drives lobby, map, movement, retry, sync, build, recruit, battle, result, rematch affordance, and history through public APIs.",
        ),
        implemented(
            "checkpoint 19 end-to-end fixture",
            "The automated fixture now composes the backend victory route with a deterministic movement conflict probe and records command/query/event/storage measurements.",
        ),
        deferred(
            "Part 1 expansion systems",
            "Campaign, large procedural maps, naval movement, complex siege, skill-tree choices, full spellbook, quests, markets, taverns, external dwellings, ranked, guild, diplomacy, and broader meta systems remain in checkpoints 21-27 until Part 2 adds bounded specs.",
        ),
    ]
}

fn implemented(area: &str, note: &str) -> SpecAuditRow {
    SpecAuditRow {
        area: area.to_string(),
        status: SpecAuditStatus::Implemented,
        note: note.to_string(),
    }
}

fn deferred(area: &str, note: &str) -> SpecAuditRow {
    SpecAuditRow {
        area: area.to_string(),
        status: SpecAuditStatus::Deferred,
        note: note.to_string(),
    }
}
