use std::collections::BTreeSet;

use crate::content::first_playable_content_manifest;
use crate::effects::{
    EffectDomain, EffectRequest, dispatch_effect, legal_effect_action, resolve_chance_effect,
    validate_status_keys,
};
use crate::rng::RollKey;

#[test]
fn first_playable_content_effect_keys_have_handlers() {
    let manifest = first_playable_content_manifest();
    let mut checked = BTreeSet::new();

    for unit in &manifest.units {
        for ability in &unit.ability_keys {
            let resolution = dispatch_effect(EffectRequest::new(EffectDomain::Ability, ability));
            assert!(
                resolution.supported,
                "missing ability handler for {ability}"
            );
            checked.insert(format!("ability:{ability}"));
        }
    }
    for building in &manifest.buildings {
        if let Some(effect_key) = building.effect_key.as_deref() {
            let resolution =
                dispatch_effect(EffectRequest::new(EffectDomain::Building, effect_key));
            assert!(
                resolution.supported,
                "missing building handler for {effect_key}"
            );
            checked.insert(format!("building:{effect_key}"));
        }
    }
    for artifact in &manifest.artifacts {
        let resolution = dispatch_effect(EffectRequest::new(
            EffectDomain::Artifact,
            &artifact.effect_key,
        ));
        assert!(
            resolution.supported,
            "missing artifact handler for {}",
            artifact.effect_key
        );
        checked.insert(format!("artifact:{}", artifact.effect_key));
    }
    for object in &manifest.map_objects {
        let resolution = dispatch_effect(EffectRequest::new(
            EffectDomain::Object,
            &object.interaction_key,
        ));
        assert!(
            resolution.supported,
            "missing object handler for {}",
            object.interaction_key
        );
        checked.insert(format!("object:{}", object.interaction_key));
    }
    for spell in &manifest.spells {
        let resolution =
            dispatch_effect(EffectRequest::new(EffectDomain::Spell, &spell.effect_key));
        assert!(
            resolution.supported,
            "missing spell handler for {}",
            spell.effect_key
        );
    }

    assert!(checked.contains("ability:ranged"));
    assert!(checked.contains("ability:guarded"));
    assert!(checked.contains("artifact:minor_might_plus_1"));
    assert!(checked.contains("object:grant_resource_reward"));
}

#[test]
fn unsupported_systems_return_typed_disabled_reasons() {
    for (domain, reason) in [
        (EffectDomain::Spell, "unsupported_spell_effect"),
        (EffectDomain::SkillTree, "unsupported_skill_effect"),
        (EffectDomain::Morale, "morale_disabled_v1"),
        (EffectDomain::Luck, "luck_disabled_v1"),
        (EffectDomain::Status, "complex_status_deferred_v1"),
    ] {
        let resolution = dispatch_effect(EffectRequest::new(domain, "placeholder"));
        assert!(!resolution.supported);
        assert_eq!(resolution.disabled_reason.as_deref(), Some(reason));
    }
}

#[test]
fn chance_effect_rolls_are_deterministic_and_auditable() {
    let roll_key = RollKey::new(
        "domm:first-playable:v1",
        "effect:guarded",
        3,
        "command:effect",
        "stack:a",
        "stack:b",
        0,
    );
    let first = resolve_chance_effect(
        EffectRequest::new(EffectDomain::Ability, "guarded"),
        50,
        &roll_key,
    )
    .expect("chance effect should resolve");
    let retry = resolve_chance_effect(
        EffectRequest::new(EffectDomain::Ability, "guarded"),
        50,
        &roll_key,
    )
    .expect("chance effect retry should resolve");

    assert_eq!(first.roll_audit, retry.roll_audit);
    assert_eq!(
        first.roll_audit.as_ref().unwrap().domain_key,
        "effect:guarded"
    );
}

#[test]
fn status_keys_are_bounded() {
    let ok = vec!["a".to_string(); 8];
    validate_status_keys(&ok).expect("eight status keys are allowed");

    let too_many = vec!["a".to_string(); 9];
    let error = validate_status_keys(&too_many).expect_err("nine should exceed v1 cap");
    assert!(format!("{error}").contains("status key cap exceeded"));
}

#[test]
fn cast_ability_is_never_enabled_for_v1_content() {
    let manifest = first_playable_content_manifest();
    for unit in &manifest.units {
        for ability in &unit.ability_keys {
            let action = legal_effect_action("CastAbility", ability);
            assert!(!action.enabled);
            assert_eq!(
                action.disabled_reason.as_deref(),
                Some("unsupported_cast_ability")
            );
        }
    }
}
