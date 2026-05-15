use std::fmt::Write as _;

use candid::CandidType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const ROLL_HASH_VERSION: &str = "domm.roll.v1";
const MAX_REJECTION_ATTEMPTS: u8 = 32;

/// Explicit deterministic pseudo-random roll key.
///
/// This is deliberately a value object, not a mutable RNG cursor. Gameplay code
/// should build one key per logical random outcome and advance `roll_index` only
/// when the rules call for another independent outcome in the same domain.
#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RollKey {
    pub session_seed: String,
    pub domain_key: String,
    pub turn_number: u32,
    pub command_id_or_system_key: String,
    pub actor_id_text: String,
    pub target_id_text: String,
    pub roll_index: u32,
}

impl RollKey {
    #[must_use]
    pub fn new(
        session_seed: impl Into<String>,
        domain_key: impl Into<String>,
        turn_number: u32,
        command_id_or_system_key: impl Into<String>,
        actor_id_text: impl Into<String>,
        target_id_text: impl Into<String>,
        roll_index: u32,
    ) -> Self {
        Self {
            session_seed: session_seed.into(),
            domain_key: domain_key.into(),
            turn_number,
            command_id_or_system_key: command_id_or_system_key.into(),
            actor_id_text: actor_id_text.into(),
            target_id_text: target_id_text.into(),
            roll_index,
        }
    }

    #[must_use]
    pub fn with_roll_index(&self, roll_index: u32) -> Self {
        Self {
            roll_index,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn roll(&self) -> DeterministicRoll {
        deterministic_roll(self, None)
    }

    pub fn roll_below(&self, upper_exclusive: u64) -> Result<BoundedRoll, RngError> {
        bounded_roll(self, 0, upper_exclusive.checked_sub(1), upper_exclusive)
    }

    pub fn roll_between_inclusive(
        &self,
        min_inclusive: u64,
        max_inclusive: u64,
    ) -> Result<BoundedRoll, RngError> {
        if max_inclusive < min_inclusive {
            return Err(RngError::InvalidRange {
                min_inclusive,
                max_inclusive,
            });
        }

        if min_inclusive == 0 && max_inclusive == u64::MAX {
            let roll = self.roll();
            return Ok(BoundedRoll {
                key: self.clone(),
                raw_value: roll.value,
                value: roll.value,
                min_inclusive,
                max_inclusive,
                rejection_attempt: 0,
                digest_hex: roll.digest_hex,
            });
        }

        let upper_exclusive = max_inclusive
            .checked_sub(min_inclusive)
            .and_then(|range| range.checked_add(1))
            .expect("non-full u64 inclusive ranges fit in u64");
        let mut bounded = bounded_roll(self, min_inclusive, Some(max_inclusive), upper_exclusive)?;
        bounded.value += min_inclusive;
        Ok(bounded)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct DeterministicRoll {
    pub key: RollKey,
    pub value: u64,
    pub digest_hex: String,
}

impl DeterministicRoll {
    #[must_use]
    pub fn audit(&self) -> RollAudit {
        RollAudit {
            domain_key: self.key.domain_key.clone(),
            roll_index: self.key.roll_index,
            raw_value: self.value,
            value: self.value,
            min_inclusive: 0,
            max_inclusive: u64::MAX,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct BoundedRoll {
    pub key: RollKey,
    pub raw_value: u64,
    pub value: u64,
    pub min_inclusive: u64,
    pub max_inclusive: u64,
    pub rejection_attempt: u8,
    pub digest_hex: String,
}

impl BoundedRoll {
    #[must_use]
    pub fn audit(&self) -> RollAudit {
        RollAudit {
            domain_key: self.key.domain_key.clone(),
            roll_index: self.key.roll_index,
            raw_value: self.raw_value,
            value: self.value,
            min_inclusive: self.min_inclusive,
            max_inclusive: self.max_inclusive,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct RollAudit {
    pub domain_key: String,
    pub roll_index: u32,
    pub raw_value: u64,
    pub value: u64,
    pub min_inclusive: u64,
    pub max_inclusive: u64,
}

impl RollAudit {
    #[must_use]
    pub fn to_event_json(&self) -> String {
        format!(
            "{{\"domain_key\":\"{}\",\"roll_index\":{},\"raw_value\":{},\"value\":{},\"min_inclusive\":{},\"max_inclusive\":{}}}",
            escape_json(&self.domain_key),
            self.roll_index,
            self.raw_value,
            self.value,
            self.min_inclusive,
            self.max_inclusive,
        )
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RngError {
    #[error("upper_exclusive must be greater than zero")]
    EmptyRange,
    #[error("invalid range {min_inclusive}..={max_inclusive}")]
    InvalidRange {
        min_inclusive: u64,
        max_inclusive: u64,
    },
    #[error(
        "deterministic rejection sampling exceeded {attempts} attempts for range {upper_exclusive}"
    )]
    RejectionAttemptsExceeded { upper_exclusive: u64, attempts: u8 },
}

#[must_use]
pub fn hash64(key: &RollKey) -> u64 {
    key.roll().value
}

pub fn roll_below(key: &RollKey, upper_exclusive: u64) -> Result<BoundedRoll, RngError> {
    key.roll_below(upper_exclusive)
}

pub fn roll_between_inclusive(
    key: &RollKey,
    min_inclusive: u64,
    max_inclusive: u64,
) -> Result<BoundedRoll, RngError> {
    key.roll_between_inclusive(min_inclusive, max_inclusive)
}

fn bounded_roll(
    key: &RollKey,
    min_inclusive: u64,
    max_inclusive: Option<u64>,
    upper_exclusive: u64,
) -> Result<BoundedRoll, RngError> {
    if upper_exclusive == 0 {
        return Err(RngError::EmptyRange);
    }

    let acceptance_zone = u64::MAX - (u64::MAX % upper_exclusive);
    for rejection_attempt in 0..MAX_REJECTION_ATTEMPTS {
        let roll = deterministic_roll(key, Some(rejection_attempt));
        if roll.value < acceptance_zone {
            return Ok(BoundedRoll {
                key: key.clone(),
                raw_value: roll.value,
                value: roll.value % upper_exclusive,
                min_inclusive,
                max_inclusive: max_inclusive.unwrap_or(upper_exclusive - 1),
                rejection_attempt,
                digest_hex: roll.digest_hex,
            });
        }
    }

    Err(RngError::RejectionAttemptsExceeded {
        upper_exclusive,
        attempts: MAX_REJECTION_ATTEMPTS,
    })
}

fn deterministic_roll(key: &RollKey, rejection_attempt: Option<u8>) -> DeterministicRoll {
    let digest = roll_digest(key, rejection_attempt);
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(&digest[..8]);

    DeterministicRoll {
        key: key.clone(),
        value: u64::from_be_bytes(raw),
        digest_hex: to_hex(&digest),
    }
}

fn roll_digest(key: &RollKey, rejection_attempt: Option<u8>) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hash_text(&mut hasher, "version", ROLL_HASH_VERSION);
    hash_text(&mut hasher, "session_seed", &key.session_seed);
    hash_text(&mut hasher, "domain_key", &key.domain_key);
    hash_u32(&mut hasher, "turn_number", key.turn_number);
    hash_text(
        &mut hasher,
        "command_id_or_system_key",
        &key.command_id_or_system_key,
    );
    hash_text(&mut hasher, "actor_id_text", &key.actor_id_text);
    hash_text(&mut hasher, "target_id_text", &key.target_id_text);
    hash_u32(&mut hasher, "roll_index", key.roll_index);
    hash_u8(
        &mut hasher,
        "rejection_attempt",
        rejection_attempt.unwrap_or(0),
    );
    hasher.finalize().to_vec()
}

fn hash_text(hasher: &mut Sha256, label: &str, value: &str) {
    hash_bytes(hasher, label, value.as_bytes());
}

fn hash_u32(hasher: &mut Sha256, label: &str, value: u32) {
    hash_bytes(hasher, label, &value.to_be_bytes());
}

fn hash_u8(hasher: &mut Sha256, label: &str, value: u8) {
    hash_bytes(hasher, label, &[value]);
}

fn hash_bytes(hasher: &mut Sha256, label: &str, value: &[u8]) {
    hasher.update(label.as_bytes());
    hasher.update(b":");
    hasher.update(value.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value);
    hasher.update(b"\n");
}

fn to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{RngError, RollKey, hash64, roll_between_inclusive};
    use crate::fixtures::first_playable_fixture;

    fn combat_key() -> RollKey {
        let fixture = first_playable_fixture();
        RollKey::new(
            fixture.scenario_seed,
            "combat_damage",
            3,
            "battle-command:fixture",
            "fixture-champion-one",
            "fixture-neutral-stack-one",
            0,
        )
    }

    #[test]
    fn same_explicit_key_reproduces_same_roll() {
        let key = combat_key();
        let first = key.roll();
        let second = key.roll();
        let first_damage = key
            .roll_between_inclusive(6, 10)
            .expect("damage range should roll");
        let second_damage = key
            .roll_between_inclusive(6, 10)
            .expect("damage range should roll");

        assert_eq!(first, second);
        assert_eq!(first_damage, second_damage);
        assert!((6..=10).contains(&first_damage.value));
    }

    #[test]
    fn domain_key_separates_independent_random_streams() {
        let combat = combat_key();
        let artifact = RollKey::new(
            combat.session_seed.clone(),
            "artifact_capture",
            combat.turn_number,
            combat.command_id_or_system_key.clone(),
            combat.actor_id_text.clone(),
            combat.target_id_text.clone(),
            combat.roll_index,
        );

        assert_ne!(hash64(&combat), hash64(&artifact));
    }

    #[test]
    fn roll_index_separates_multiple_rolls_in_same_domain() {
        let first = combat_key();
        let second = first.with_roll_index(1);

        assert_ne!(hash64(&first), hash64(&second));
    }

    #[test]
    fn all_explicit_key_fields_affect_the_roll() {
        let base = combat_key();
        let changed_command = RollKey::new(
            base.session_seed.clone(),
            base.domain_key.clone(),
            base.turn_number,
            "battle-command:other",
            base.actor_id_text.clone(),
            base.target_id_text.clone(),
            base.roll_index,
        );
        let changed_actor = RollKey::new(
            base.session_seed.clone(),
            base.domain_key.clone(),
            base.turn_number,
            base.command_id_or_system_key.clone(),
            "fixture-champion-two",
            base.target_id_text.clone(),
            base.roll_index,
        );
        let changed_target = RollKey::new(
            base.session_seed.clone(),
            base.domain_key.clone(),
            base.turn_number,
            base.command_id_or_system_key.clone(),
            base.actor_id_text.clone(),
            "fixture-neutral-stack-two",
            base.roll_index,
        );

        assert_ne!(hash64(&base), hash64(&changed_command));
        assert_ne!(hash64(&base), hash64(&changed_actor));
        assert_ne!(hash64(&base), hash64(&changed_target));
    }

    #[test]
    fn fixture_roll_is_stable() {
        let roll = combat_key().roll();
        let bounded = combat_key()
            .roll_between_inclusive(6, 10)
            .expect("damage range should roll");

        assert_eq!(roll.value, 10_565_608_839_925_773_534);
        assert_eq!(
            roll.digest_hex,
            "92a09542cb80b4de815460e4fecc1263d6605aaafaf85f9867706068d9fc786f"
        );
        assert_eq!(bounded.value, 10);
        assert_eq!(
            bounded.audit().to_event_json(),
            "{\"domain_key\":\"combat_damage\",\"roll_index\":0,\"raw_value\":10565608839925773534,\"value\":10,\"min_inclusive\":6,\"max_inclusive\":10}"
        );
    }

    #[test]
    fn bounded_roll_rejects_empty_and_inverted_ranges() {
        let key = combat_key();

        assert_eq!(key.roll_below(0), Err(RngError::EmptyRange));
        assert_eq!(
            key.roll_between_inclusive(10, 6),
            Err(RngError::InvalidRange {
                min_inclusive: 10,
                max_inclusive: 6,
            })
        );
    }

    #[test]
    fn free_function_matches_key_method() {
        let key = combat_key();

        assert_eq!(
            roll_between_inclusive(&key, 1, 100).expect("range should roll"),
            key.roll_between_inclusive(1, 100)
                .expect("range should roll")
        );
    }
}
