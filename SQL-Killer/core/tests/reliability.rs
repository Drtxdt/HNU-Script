use sql_killer_core::{algebra, database, model::*, project, schedule, trace};
#[test]
fn decimal_65_digits_stays_exact() {
    let table = |v: &str| Table {
        columns: vec!["x".into()],
        rows: vec![vec![Cell::Decimal(v.into())]],
        truncated: false,
    };
    assert!(sql_killer_core::exercises::tables_equal(
        &table("1234567890123456789012345678901234567890.00"),
        &table("1234567890123456789012345678901234567890"),
        false,
        false
    ));
    assert!(!sql_killer_core::exercises::tables_equal(
        &table("1234567890123456789012345678901234567890"),
        &table("1234567890123456789012345678901234567891"),
        false,
        false
    ));
}
use std::{
    collections::BTreeMap,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
#[test]
fn active_transaction_snapshot_keeps_uncommitted_changes_without_committing() {
    let c = database::memory("student").unwrap();
    c.execute_batch("BEGIN;UPDATE Student SET Sage=99;")
        .unwrap();
    let t = trace::trace_query(&c, "SELECT DISTINCT Sage FROM Student", &Budget::default());
    assert!(t.verified, "{}", t.message);
    assert_eq!(t.result.rows, vec![vec![Cell::Integer("99".into())]]);
    assert!(!c.is_autocommit());
    c.execute_batch("ROLLBACK").unwrap();
}
#[test]
fn cancellation_and_limits_are_not_results() {
    let c = database::memory("student").unwrap();
    let b = Budget::default();
    b.cancelled.store(true, Ordering::Relaxed);
    assert_eq!(
        database::execute(&c, "SELECT * FROM Student", &b).status,
        "cancelled"
    );
    let b = Budget {
        deadline: Instant::now() - Duration::from_millis(1),
        ..Budget::default()
    };
    assert_eq!(
        database::execute(&c, "SELECT * FROM Student", &b).status,
        "limit"
    );
}
#[test]
fn unicode_error_location_is_utf16() {
    let c = database::memory("empty").unwrap();
    let sql = "SELECT '中😀'; SELECT 1 + FROM nowhere;";
    let r = database::execute(&c, sql, &Budget::default());
    assert_eq!(r.status, "error");
    let range = r.error_range.unwrap();
    assert_eq!(&sql[range.start..range.end], "F");
    assert_eq!(range.column, sql[..range.start].encode_utf16().count() + 1);
}
#[test]
fn queued_transaction_evaluates_only_after_its_read() {
    let r = schedule::simulate_schedule(
        "T1 write A 9\nT2 read A\nT2 set A A+1\nT2 write A\nT2 commit\nT1 commit",
        BTreeMap::from([("A".into(), 1)]),
        "locks",
        "rc",
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(r.status, "completed");
    assert_eq!(r.steps.last().unwrap().data["A"], 10);
}
#[test]
fn predicate_lock_blocks_moving_out_of_range() {
    let r = schedule::simulate_schedule(
        "T1 scan A 0 100\nT2 write A1 200\nT2 commit\nT1 commit",
        BTreeMap::from([("A1".into(), 50)]),
        "locks",
        "serializable",
        &Budget::default(),
    )
    .unwrap();
    assert!(r.steps.iter().any(|s| s.status == "waiting"));
    assert_eq!(r.status, "completed");
}
#[test]
fn algebra_rename_and_domains() {
    let c = database::memory("student").unwrap();
    let t = algebra::evaluate_algebra(
        &c,
        "project{R.id}(rename{R;Sno:id}(Student))",
        false,
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(t.trace.result.columns, vec!["id"]);
    assert!(algebra::evaluate_algebra(
        &c,
        "union(project{Sage}(Student),project{Sname}(Student))",
        false,
        &Budget::default()
    )
    .is_err());
}
#[test]
fn corrupted_project_reports_error() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("损坏.sqlkproj");
    std::fs::write(&p, b"invalid zip").unwrap();
    assert!(project::load(&p, d.path()).is_err());
}
