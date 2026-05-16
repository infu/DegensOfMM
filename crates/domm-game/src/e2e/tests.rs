use crate::fixtures::first_playable_fixture;

use super::{SpecAuditStatus, part_two_spec_audit, run_first_playable_e2e_fixture};

#[test]
fn checkpoint_19_e2e_fixture_covers_first_playable_scope() {
    let report = run_first_playable_e2e_fixture().expect("e2e fixture should pass");
    let fixture = first_playable_fixture();

    eprintln!(
        "checkpoint-19 e2e: commands={} events={} queries={} rows={} max_query_bytes={} estimated_response_bytes={}",
        report.measurements.command_count,
        report.measurements.event_count,
        report.measurements.query_count,
        report.measurements.storage_row_count,
        report.measurements.max_query_bytes,
        report.measurements.estimated_response_bytes
    );

    assert_eq!(report.session_id, fixture.ids.session_id);
    assert!(report.coverage.complete(), "{:?}", report.coverage);
    assert_eq!(report.measurements.command_count, 32);
    assert_eq!(report.measurements.event_count, 42);
    assert_eq!(report.measurements.query_count, 19);
    assert_eq!(report.measurements.storage_row_count, 190);
    assert_eq!(report.measurements.max_query_bytes, 5072);
    assert!(report.measurements.recovery_retry_count >= 1);

    assert_eq!(report.movement_conflict.snapshot_count, 2);
    assert!(report.movement_conflict.stopped_tile_conflict);
    assert_eq!(
        (
            report.movement_conflict.west_final_x,
            report.movement_conflict.west_final_y
        ),
        (9, 24)
    );
    assert_eq!(
        (
            report.movement_conflict.east_final_x,
            report.movement_conflict.east_final_y
        ),
        (10, 24)
    );
    assert!(
        report
            .manual_smoke_commands
            .iter()
            .any(|command| command.command == "make smoke-e2e")
    );
    assert!(
        report
            .spec_audit
            .iter()
            .all(|row| row.status != SpecAuditStatus::Missing),
        "{:?}",
        report.spec_audit
    );
}

#[test]
fn part_two_spec_audit_has_no_missing_required_first_playable_scope() {
    let audit = part_two_spec_audit();

    assert!(audit.len() >= 20);
    assert!(
        audit
            .iter()
            .any(|row| row.status == SpecAuditStatus::Deferred)
    );
    assert!(
        audit
            .iter()
            .all(|row| row.status != SpecAuditStatus::Missing),
        "{audit:?}"
    );
    assert!(
        audit
            .iter()
            .filter(|row| row.status == SpecAuditStatus::Deferred)
            .all(|row| row.note.contains("checkpoints 21-27"))
    );
}
