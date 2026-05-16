use candid::CandidType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::rng::{RngError, RollAudit, RollKey};

pub const STATUS_KEY_CAP: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub enum EffectDomain {
    Ability,
    Artifact,
    Building,
    Object,
    Spell,
    SkillTree,
    Morale,
    Luck,
    Status,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EffectRequest {
    pub domain: EffectDomain,
    pub effect_key: String,
    pub actor_id_text: Option<String>,
    pub target_id_text: Option<String>,
}

impl EffectRequest {
    #[must_use]
    pub fn new(domain: EffectDomain, effect_key: impl Into<String>) -> Self {
        Self {
            domain,
            effect_key: effect_key.into(),
            actor_id_text: None,
            target_id_text: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct EffectResolution {
    pub domain: EffectDomain,
    pub effect_key: String,
    pub supported: bool,
    pub handler_key: Option<String>,
    pub disabled_reason: Option<String>,
    pub roll_audit: Option<RollAudit>,
    pub status_keys: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Serialize, Deserialize)]
pub struct LegalEffectAction {
    pub action: String,
    pub effect_key: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EffectError {
    #[error(transparent)]
    Rng(#[from] RngError),
    #[error("status key cap exceeded: {count}")]
    StatusKeyCapExceeded { count: usize },
}

#[must_use]
pub fn dispatch_effect(request: EffectRequest) -> EffectResolution {
    match request.domain {
        EffectDomain::Ability => match request.effect_key.as_str() {
            "ranged" => supported(request, "ability:ranged_passive", &[]),
            "guarded" => supported(request, "ability:guarded_passive", &["guarded"]),
            _ => disabled(request, "unsupported_ability"),
        },
        EffectDomain::Artifact => match request.effect_key.as_str() {
            "minor_might_plus_1" => supported(request, "artifact:minor_might_plus_1", &[]),
            _ => disabled(request, "unsupported_artifact_effect"),
        },
        EffectDomain::Building => match request.effect_key.as_str() {
            "town_hall_level_1" => supported(request, "building:town_hall_level_1", &[]),
            "town_income_gold_250" => supported(request, "building:town_income_gold_250", &[]),
            _ => disabled(request, "unsupported_building_effect"),
        },
        EffectDomain::Object => match request.effect_key.as_str() {
            "capture_gold_income" => supported(request, "object:capture_gold_income", &[]),
            "capture_crystal_income" => supported(request, "object:capture_crystal_income", &[]),
            "grant_resource_reward" => supported(request, "object:grant_resource_reward", &[]),
            "score_central_objective" => supported(request, "object:score_central_objective", &[]),
            "external_dwelling_direct_recruit" => {
                supported(request, "object:external_dwelling_direct_recruit", &[])
            }
            _ => disabled(request, "unsupported_object_effect"),
        },
        EffectDomain::Spell => match request.effect_key.as_str() {
            "spell:hex_spark_damage_15" => {
                supported(request, "spell:hex_spark_damage_15", &["hexed"])
            }
            "spell:spite_march_movement_30" => {
                supported(request, "spell:spite_march_movement_30", &[])
            }
            _ => disabled(request, "unsupported_spell_effect"),
        },
        EffectDomain::SkillTree => match request.effect_key.as_str() {
            "skill:sour_sorcery" => supported(request, "skill:sour_sorcery", &[]),
            "skill:dirty_tactics" => supported(request, "skill:dirty_tactics", &[]),
            "skill:grim_logistics" => supported(request, "skill:grim_logistics", &[]),
            _ => disabled(request, "unsupported_skill_effect"),
        },
        EffectDomain::Morale => disabled(request, "morale_disabled_v1"),
        EffectDomain::Luck => disabled(request, "luck_disabled_v1"),
        EffectDomain::Status => disabled(request, "complex_status_deferred_v1"),
    }
}

pub fn resolve_chance_effect(
    request: EffectRequest,
    chance_percent: u64,
    roll_key: &RollKey,
) -> Result<EffectResolution, EffectError> {
    let roll = roll_key.roll_between_inclusive(1, 100)?;
    let mut resolution = dispatch_effect(request);
    resolution.roll_audit = Some(roll.audit());
    if roll.value > chance_percent {
        resolution.supported = false;
        resolution.disabled_reason = Some("chance_roll_failed".to_string());
    }
    Ok(resolution)
}

pub fn validate_status_keys(status_keys: &[String]) -> Result<(), EffectError> {
    if status_keys.len() > STATUS_KEY_CAP {
        return Err(EffectError::StatusKeyCapExceeded {
            count: status_keys.len(),
        });
    }
    Ok(())
}

#[must_use]
pub fn legal_effect_action(action: &str, effect_key: &str) -> LegalEffectAction {
    if action == "CastAbility" && !effect_key.starts_with("spell:") {
        return LegalEffectAction {
            action: action.to_string(),
            effect_key: effect_key.to_string(),
            enabled: false,
            disabled_reason: Some("unsupported_cast_ability".to_string()),
        };
    }
    let domain = if effect_key.starts_with("spell:") {
        EffectDomain::Spell
    } else {
        EffectDomain::Ability
    };
    let resolution = dispatch_effect(EffectRequest::new(domain, effect_key));
    LegalEffectAction {
        action: action.to_string(),
        effect_key: effect_key.to_string(),
        enabled: resolution.supported,
        disabled_reason: resolution.disabled_reason,
    }
}

fn supported(request: EffectRequest, handler_key: &str, status_keys: &[&str]) -> EffectResolution {
    EffectResolution {
        domain: request.domain,
        effect_key: request.effect_key,
        supported: true,
        handler_key: Some(handler_key.to_string()),
        disabled_reason: None,
        roll_audit: None,
        status_keys: status_keys.iter().map(|key| (*key).to_string()).collect(),
    }
}

fn disabled(request: EffectRequest, reason: &str) -> EffectResolution {
    EffectResolution {
        domain: request.domain,
        effect_key: request.effect_key,
        supported: false,
        handler_key: None,
        disabled_reason: Some(reason.to_string()),
        roll_audit: None,
        status_keys: Vec::new(),
    }
}
