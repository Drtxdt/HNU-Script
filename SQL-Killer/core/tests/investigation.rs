use sql_killer_core::{case_experience as ux, cases, model::Budget};
#[test]
fn guide_opens_logs_from_verified_answer_not_checkbox() {
    let b = Budget::default();
    let mut p = cases::CaseProgress {
        version: 1,
        ..Default::default()
    };
    let initial = ux::context(&p, &b).unwrap();
    assert_eq!(initial.tasks.len(), 1);
    assert!(!initial
        .schema
        .relations
        .iter()
        .any(|r| r.name == "RequestLog"));
    assert!(!initial.narrative.contains("39"));
    assert!(!initial.title.contains("41"));
    p.completed.push("capacity".into());
    assert_eq!(ux::stage(&p, &b).unwrap(), 0);
    p.answers.insert("capacity".into(),"SELECT C.Cno,C.capacity,COUNT(*) actual FROM CaseCourse C JOIN Enrollment E ON E.Cno=C.Cno WHERE E.status='success' GROUP BY C.Cno,C.capacity HAVING COUNT(*)>C.capacity".into());
    let next = ux::context(&p, &b).unwrap();
    assert_eq!(next.tasks.len(), 2);
    assert!(next.schema.relations.iter().any(|r| r.name == "RequestLog"));
    p.mode = "expert".into();
    let expert = ux::context(&p, &b).unwrap();
    assert_eq!(expert.tasks.len(), 5);
    assert!(expert.tasks.iter().all(|t| t.starter.is_empty()));
}
#[test]
fn stage_zero_database_cannot_preview_or_query_logs() {
    let b = Budget::default();
    let p = cases::CaseProgress::default();
    let c = ux::accessible_fixture(&p, &b).unwrap();
    assert!(cases::query(&c, "SELECT * FROM RequestLog", &b).is_err());
    assert!(ux::require_task(&p, "repair", &b).is_err());
}
#[test]
fn legacy_case_keeps_conclusion_and_moves_links_to_ordinary_relations() {
    let mut p:cases::CaseProgress=serde_json::from_value(serde_json::json!({"version":1,"seed":0,"task":"capacity","answers":{},"completed":[],"evidence":[],"cause":"stale_read","model_strategy":"row_lock","mysql_strategy":"","closed":true})).unwrap();
    ux::migrate(&mut p);
    assert!(p.closed);
    assert_eq!(p.closure_revision, 0);
    assert_eq!(p.mode, "guided");
    assert!(ux::validate_board(&p).is_ok());
}
