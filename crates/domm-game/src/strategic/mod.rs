mod backend;
mod driver;
#[cfg(test)]
mod tests;
mod types;

pub use backend::StrategicFixtureBackend;
pub use driver::{StrategicBackend, StrategicHeadlessDriver, run_first_playable_strategic_gate};
pub use types::{
    StrategicCall, StrategicCommandReceipt, StrategicError, StrategicGameView, StrategicGateReport,
    StrategicStepView,
};
