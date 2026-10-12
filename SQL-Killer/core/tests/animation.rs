use sql_killer_core::{algebra, database, model::*, schedule, trace};
#[test]
fn engine_truth_and_duplicate_occurrences() {
    let c = database::memory("empty").unwrap();
    c.execute_batch("CREATE TABLE T(n INTEGER);INSERT INTO T VALUES(1),(1),(0),(NULL);")
        .unwrap();
    let t = trace::trace_query(&c, "SELECT * FROM T WHERE n=1", &Budget::default());
    assert!(t.verified, "{}", t.message);
    let step = t.steps.iter().find(|s| s.kind == "filter").unwrap();
    assert_eq!(
        step.events
            .iter()
            .map(|e| e.truth.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["TRUE", "TRUE", "FALSE", "UNKNOWN"]
    );
    assert_ne!(
        step.events[0].targets[0].index,
        step.events[1].targets[0].index
    );
    assert!(step.events.iter().all(|e| e.certainty == "exact"));
}
#[test]
fn join_has_two_parents_and_ambiguous_sources_are_never_exact() {
    let c = database::memory("empty").unwrap();
    c.execute_batch("CREATE TABLE A(n);CREATE TABLE B(n);INSERT INTO A VALUES(1),(1),(2);INSERT INTO B VALUES(1);").unwrap();
    let t = trace::trace_query(
        &c,
        "SELECT A.n,B.n FROM A LEFT JOIN B ON A.n=B.n",
        &Budget::default(),
    );
    assert!(t.verified, "{}", t.message);
    let join = t.steps.iter().find(|s| s.kind == "join").unwrap();
    assert_eq!(join.parents.len(), 2);
    assert!(join
        .events
        .iter()
        .any(|e| e.sources.len() > 2 && e.certainty == "candidate"));
    assert!(join.events.iter().any(|e| e.message.contains("NULL")));
}
#[test]
fn native_groups_null_and_case_insensitive_keys() {
    let c = database::memory("empty").unwrap();
    c.execute_batch("CREATE TABLE T(k TEXT COLLATE NOCASE,n);INSERT INTO T VALUES('A',1),('a',2),(NULL,3),(NULL,NULL);").unwrap();
    let t = trace::trace_query(
        &c,
        "SELECT k,COUNT(*),COUNT(n) FROM T GROUP BY k",
        &Budget::default(),
    );
    assert!(t.verified, "{}", t.message);
    let group = t.steps.iter().find(|s| s.kind == "group").unwrap();
    assert_eq!(group.events.len(), 4);
    assert_eq!(
        group.events[0].targets[0].index,
        group.events[1].targets[0].index
    );
    assert_eq!(
        group.events[2].targets[0].index,
        group.events[3].targets[0].index
    );
    assert!(group.events.iter().all(|e| e.certainty == "exact"));
}
#[test]
fn empty_aggregate_and_algebra_have_events() {
    let c = database::memory("student").unwrap();
    let t = trace::trace_query(
        &c,
        "SELECT COUNT(*),AVG(Sage) FROM Student WHERE 0",
        &Budget::default(),
    );
    assert!(t.verified);
    assert!(t.steps.last().unwrap().events[0].message.contains("空输入"));
    let a = algebra::evaluate_algebra(
        &c,
        "project{Sno}(select{Sage>17}(Student))",
        true,
        &Budget::default(),
    )
    .unwrap();
    assert!(a.trace.steps.iter().all(|s| !s.events.is_empty()));
    assert!(a.trace.steps.iter().all(|s| s.source_range.is_some()));
}
#[test]
fn schedule_events_preserve_original_lines_and_real_changes() {
    let r = schedule::simulate_schedule(
        "#说明\nT1 read A\nT1 write A 20\nT1 rollback",
        std::collections::BTreeMap::from([("A".into(), 10)]),
        "raw",
        "ru",
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(r.steps[0].source_line, 2);
    assert_eq!(r.steps[1].changes[0].before, "10");
    assert_eq!(r.steps[1].changes[0].after, "20");
    assert_eq!(r.steps[2].changes[0].after, "10");
}
#[test]
fn native_running_counts_and_having_truth() {
    let c = database::memory("empty").unwrap();
    c.execute_batch("CREATE TABLE T(k,n); INSERT INTO T VALUES('a',1),('a',NULL),('b',2);")
        .unwrap();
    let t = trace::trace_query(
        &c,
        "SELECT k,COUNT(*) AS total,COUNT(n) AS present FROM T GROUP BY k HAVING total > 1",
        &Budget::default(),
    );
    assert!(t.verified, "{}", t.message);
    let aggregate = t.steps.iter().find(|s| s.kind == "aggregate").unwrap();
    assert_eq!(aggregate.events.len(), 3);
    assert!(aggregate.events[1].message.contains("COUNT(*) = 2"));
    assert!(aggregate.events[1].message.contains("COUNT(n) = 1"));
    let having = t.steps.iter().find(|s| s.kind == "having").unwrap();
    assert_eq!(
        having
            .events
            .iter()
            .map(|e| e.truth.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("TRUE"), Some("FALSE")]
    );
}
#[test]
fn division_checks_missing_pairs_and_empty_divisor() {
    let c = database::memory("empty").unwrap();
    c.execute_batch("CREATE TABLE R(x,y); CREATE TABLE S(y); INSERT INTO R VALUES(1,2),(1,3),(2,2);INSERT INTO S VALUES(2),(3);").unwrap();
    let a = algebra::evaluate_algebra(&c, "divide(R,S)", false, &Budget::default()).unwrap();
    let events = &a.trace.steps.last().unwrap().events;
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .filter(|e| e.truth.as_deref() == Some("FALSE"))
            .count(),
        1
    );
    c.execute("DELETE FROM S", []).unwrap();
    let a = algebra::evaluate_algebra(&c, "divide(R,S)", false, &Budget::default()).unwrap();
    assert_eq!(a.trace.steps.last().unwrap().events.len(), 2);
    assert!(a
        .trace
        .steps
        .last()
        .unwrap()
        .events
        .iter()
        .all(|e| e.truth.as_deref() == Some("TRUE")));
}
#[test]
fn hidden_exercise_failures_do_not_expose_answers() {
    let q = sql_killer_core::exercises::catalog(0)
        .into_iter()
        .find(|q| q.id == "where")
        .unwrap();
    let answer = format!("{} AND Sno NOT LIKE 'x%'", q.solution);
    let grade = sql_killer_core::exercises::grade("where", &answer, 0, &Budget::default());
    assert_eq!(grade.status, "failed");
    assert!(grade.cases > 0);
    assert!(grade.expected.is_none() && grade.actual.is_none());
}
#[test]
fn ast_ranges_distinguish_repeated_clauses_and_unicode() {
    let c = database::memory("student").unwrap();
    let sql="SELECT '😀中文',Sno FROM Student WHERE Sage>17 UNION ALL SELECT '😀中文',Sno FROM Student WHERE Sage>19";
    let t = trace::trace_query(&c, sql, &Budget::default());
    assert!(t.verified, "{}", t.message);
    let ranges = t
        .steps
        .iter()
        .filter(|s| s.kind == "filter")
        .map(|s| s.source_range.as_ref().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ranges.len(), 2);
    assert_ne!(ranges[0].start, ranges[1].start);
    assert_eq!(&sql[ranges[0].start..ranges[0].end], "Sage>17");
    assert_eq!(&sql[ranges[1].start..ranges[1].end], "Sage>19");
    assert_eq!(
        ranges[1].column,
        sql[..ranges[1].start].encode_utf16().count() + 1
    );
}
