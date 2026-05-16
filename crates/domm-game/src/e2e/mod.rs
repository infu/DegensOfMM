mod audit;
mod driver;
mod movement_conflict;
#[cfg(test)]
mod tests;
mod types;

pub use audit::part_two_spec_audit;
pub use driver::run_first_playable_e2e_fixture;
pub use movement_conflict::run_e2e_movement_conflict_probe;
pub use types::{
    EndToEndCoverage, EndToEndError, EndToEndFirstPlayableReport, EndToEndMeasurements,
    ManualSmokeCommand, MovementConflictReport, SpecAuditRow, SpecAuditStatus,
};
