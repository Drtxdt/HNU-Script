use sql_killer_core::{cases, database, exercises, language, model::*, project, trace};
use std::io::{Read, Write};

#[test]
fn reference_insertion_respects_aliases_shadowing_and_self_joins() {
    let schema = database::schema(&database::memory("student").unwrap()).unwrap();
    let resolve = |sql: &str, cursor: usize| {
        language::resolve_reference(sql, cursor, &schema, "sqlite", "Student", "Sno")
    };
    let sql = "SELECT S.Sname FROM Student S";
    assert_eq!(resolve(sql, 7)[0].insert_text, "\"S\".\"Sno\"");
    assert_eq!(
        resolve("SELECT * FROM Student A JOIN Student B ON A.Sno=B.Sno", 7).len(),
        2
    );
    assert!(resolve("SELECT * FROM Course", 7).is_empty());
    let sql = "SELECT * FROM Student S WHERE EXISTS(SELECT * FROM SC S WHERE S.Sno='x')";
    assert!(
        resolve(sql, sql.find("S.Sno").unwrap()).is_empty(),
        "inner alias shadows outer binding"
    );
    assert_eq!(
        resolve("SELECT * FROM Student", 7)[0].insert_text,
        "\"Student\".\"Sno\""
    );
}
#[test]
fn cartesian_product_retains_every_branch_terminal() {
    let c = database::memory("student").unwrap();
    for sql in [
        "SELECT * FROM Student,Course",
        "SELECT * FROM Student S JOIN SC C ON S.Sno=C.Sno, Course K",
    ] {
        let t = trace::trace_query(&c, sql, &Budget::default());
        assert!(t.verified, "{}", t.message);
        let product = t.steps.iter().find(|s| s.title == "FROM 笛卡尔积").unwrap();
        assert_eq!(product.parents.len(), 2);
        assert!(product.parents.iter().all(|id| *id < product.id));
        if sql.contains("JOIN") {
            assert!(product.parents.iter().any(|id| t.steps[*id].kind == "join"));
        }
    }
}
#[test]
fn grade_diff_uses_native_numeric_and_order_contracts() {
    let t = |rows| Table {
        columns: vec!["n".into()],
        rows,
        truncated: false,
    };
    let a = t(vec![
        vec![Cell::Integer("1".into())],
        vec![Cell::Null],
        vec![Cell::Integer("1".into())],
    ]);
    let b = t(vec![
        vec![Cell::Decimal("1.00".into())],
        vec![Cell::Decimal("1.0".into())],
        vec![Cell::Null],
    ]);
    assert!(exercises::compare_tables(&a, &b, false, false).equal);
    assert!(!exercises::compare_tables(&a, &b, true, false).equal);
    let mut partial = b.clone();
    partial.truncated = true;
    assert!(!exercises::compare_tables(&a, &partial, false, false).equal);
}
#[test]
fn two_query_comparison_uses_snapshot_and_does_not_write() {
    let c = database::memory("student").unwrap();
    let before = c.total_changes();
    let r = cases::compare_queries(
        &c,
        "SELECT Sno FROM Student",
        "SELECT Sno FROM Student WHERE 0",
        &Budget::default(),
    )
    .unwrap();
    assert!(!r.diff.equal);
    assert_eq!(c.total_changes(), before);
    assert!(
        cases::compare_queries(&c, "DELETE FROM Student", "SELECT 1", &Budget::default()).is_err()
    );
    assert!(!database::execute(&c, "SELECT * FROM Student", &Budget::default()).modified);
    assert!(database::execute(&c, "CREATE TABLE NewTable(n)", &Budget::default()).modified);
}
#[test]
fn version_one_project_migrates_without_losing_answers_or_database() {
    let base = std::path::Path::new("../build/self-study-tests");
    std::fs::create_dir_all(base).unwrap();
    let dir = tempfile::tempdir_in(base).unwrap();
    let path = dir.path().join("legacy.sqlkproj");
    let c = database::memory("student").unwrap();
    let mut p = project::Workspace {
        exercise_seed: 3,
        ..Default::default()
    };
    p.exercise_answers
        .insert("where@3".into(), "SELECT Sno FROM Student".into());
    project::save(&path, &p, &c).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut data = Vec::new();
    zip.by_name("database.sqlite")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    drop(zip);
    let mut manifest = serde_json::to_value(&p).unwrap();
    manifest["format_version"] = 1.into();
    manifest["view"] = serde_json::Value::Null;
    manifest.as_object_mut().unwrap().remove("learning");
    manifest.as_object_mut().unwrap().remove("case_progress");
    let mut out = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    out.start_file("manifest.json", options).unwrap();
    out.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    out.start_file("database.sqlite", options).unwrap();
    out.write_all(&data).unwrap();
    out.finish().unwrap();
    let (restored, db) = project::load(&path, dir.path()).unwrap();
    assert_eq!(restored.format_version, 3);
    assert_eq!(restored.exercise_seed, 3);
    assert_eq!(restored.exercise_answers, p.exercise_answers);
    assert_eq!(restored.view["migrated_from"], 1);
    assert_eq!(database::schema(&db).unwrap().relations.len(), 5);
    let still_old: serde_json::Value = {
        let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        serde_json::from_reader(z.by_name("manifest.json").unwrap()).unwrap()
    };
    assert_eq!(still_old["format_version"], 1);
}

#[test]
fn truncated_trace_is_incomplete_and_never_verified() {
    let c = database::memory("student").unwrap();
    let b = Budget {
        max_rows: 1,
        ..Budget::default()
    };
    let t = trace::trace_query(&c, "SELECT * FROM Student", &b);
    assert_eq!(t.status, "incomplete");
    assert!(!t.verified);
    assert!(t.steps.is_empty());
}
