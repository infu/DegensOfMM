mod backend;
mod driver;
#[cfg(test)]
mod tests;
mod types;

pub use backend::PlayableFixtureBackend;
pub use driver::run_first_playable_backend_gate;
pub use types::{
    PlayableBattleView, PlayableCall, PlayableCommandReceipt, PlayableError, PlayableEventPage,
    PlayableGateReport, PlayableMatchView,
};
