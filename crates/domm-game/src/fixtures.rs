use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

/// Canonical v1 fixture seed for the first playable 1v1 scenario.
pub const FIRST_PLAYABLE_SCENARIO_SEED: &str = "domm:first-playable:v1";

/// Part 2 requires 60-second simultaneous turn windows.
pub const TURN_DURATION_MS: u64 = 60_000;

/// Stable fixture clock values for deterministic scenario tests.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct FixtureClock {
    pub start_timestamp_ms: u64,
    pub turn_duration_ms: u64,
}

/// Stable principals used by headless public API tests.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct FixturePrincipals {
    pub player_one: Principal,
    pub player_two: Principal,
    pub controller: Principal,
}

/// Stable command nonces used by idempotency and retry tests.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct CommandNonces {
    pub register_player_one: String,
    pub register_player_two: String,
    pub create_session: String,
    pub join_session: String,
    pub mark_ready_player_one: String,
    pub mark_ready_player_two: String,
    pub start_session: String,
    pub inspect_session: String,
}

/// Stable semantic IDs used before the IcyDB schema creates durable IDs.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct FixtureIds {
    pub session_id: String,
    pub player_one_id: String,
    pub player_two_id: String,
    pub participant_one_id: String,
    pub participant_two_id: String,
    pub map_id: String,
}

/// Deterministic data shared by pure, generated-session, and canister tests.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct ScenarioFixture {
    pub scenario_seed: String,
    pub clock: FixtureClock,
    pub principals: FixturePrincipals,
    pub command_nonces: CommandNonces,
    pub ids: FixtureIds,
}

/// Build the canonical deterministic fixture for the first playable scenario.
#[must_use]
pub fn first_playable_fixture() -> ScenarioFixture {
    ScenarioFixture {
        scenario_seed: FIRST_PLAYABLE_SCENARIO_SEED.to_string(),
        clock: FixtureClock {
            start_timestamp_ms: 1_800_000_000_000,
            turn_duration_ms: TURN_DURATION_MS,
        },
        principals: FixturePrincipals {
            player_one: Principal::self_authenticating(&[0x11; 32]),
            player_two: Principal::self_authenticating(&[0x22; 32]),
            controller: Principal::self_authenticating(&[0xCC; 32]),
        },
        command_nonces: CommandNonces {
            register_player_one: "nonce:register:p1:v1".to_string(),
            register_player_two: "nonce:register:p2:v1".to_string(),
            create_session: "nonce:lobby:create:v1".to_string(),
            join_session: "nonce:lobby:join:p2:v1".to_string(),
            mark_ready_player_one: "nonce:lobby:ready:p1:v1".to_string(),
            mark_ready_player_two: "nonce:lobby:ready:p2:v1".to_string(),
            start_session: "nonce:lobby:start:v1".to_string(),
            inspect_session: "nonce:query:inspect:v1".to_string(),
        },
        ids: FixtureIds {
            session_id: "fixture-session-first-playable".to_string(),
            player_one_id: "fixture-player-one".to_string(),
            player_two_id: "fixture-player-two".to_string(),
            participant_one_id: "fixture-participant-one".to_string(),
            participant_two_id: "fixture-participant-two".to_string(),
            map_id: "fixture-map-first-playable".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use candid::{Decode, Encode};

    use super::{FIRST_PLAYABLE_SCENARIO_SEED, TURN_DURATION_MS, first_playable_fixture};

    #[test]
    fn first_playable_fixture_is_stable() {
        let fixture = first_playable_fixture();

        assert_eq!(fixture.scenario_seed, FIRST_PLAYABLE_SCENARIO_SEED);
        assert_eq!(fixture.clock.turn_duration_ms, TURN_DURATION_MS);
        assert_eq!(fixture.ids.session_id, "fixture-session-first-playable");
        assert_ne!(fixture.principals.player_one, fixture.principals.player_two);
    }

    #[test]
    fn fixture_supports_candid_roundtrip() {
        let fixture = first_playable_fixture();
        let encoded = Encode!(&fixture).expect("fixture should encode as candid");
        let decoded =
            Decode!(&encoded, super::ScenarioFixture).expect("fixture should decode from candid");

        assert_eq!(decoded, fixture);
    }
}
