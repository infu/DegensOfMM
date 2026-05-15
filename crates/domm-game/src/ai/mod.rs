mod decision;
#[cfg(test)]
mod tests;
mod types;

pub use decision::{decide_for_actor, run_ai_update};
pub use types::{
    AI_MAX_ACTORS_PER_UPDATE, AI_MAX_CANDIDATES_PER_ACTOR, AI_MAX_EMITTED_COMMANDS_PER_UPDATE,
    AiActorStateRecord, AiCommandDraft, AiDecisionInput, AiError, AiUpdateReport,
};
