#[cfg(test)]
mod tests;
mod types;

pub use types::{
    EffectDomain, EffectError, EffectRequest, EffectResolution, LegalEffectAction, dispatch_effect,
    legal_effect_action, resolve_chance_effect, validate_status_keys,
};
