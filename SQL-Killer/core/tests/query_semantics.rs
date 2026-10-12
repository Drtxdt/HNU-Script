use sql_killer_core::{database, model::Budget, trace};

#[test]
fn logical_trace_matches_native_edge_cases() {
    let db = database::memory("student").unwrap();
    let queries = [
        "SELECT * FROM Student",
        "SELECT S.Sno,C.Grade FROM Student S LEFT JOIN SC C ON S.Sno=C.Sno AND C.Grade>85",
        "SELECT S.Sno,C.Grade FROM Student S LEFT JOIN SC C ON S.Sno=C.Sno WHERE C.Grade>85",
        "SELECT * FROM Student NATURAL JOIN SC",
        "SELECT Sdept,COUNT(*),AVG(Sage) FROM Student GROUP BY Sdept HAVING COUNT(*)>1 ORDER BY Sdept",
        "SELECT COUNT(*),COUNT(Grade),SUM(Grade) FROM SC WHERE 0",
        "SELECT DISTINCT Sdept FROM Student ORDER BY Sdept LIMIT 2 OFFSET 1",
        "SELECT Sno FROM Student WHERE Sno NOT IN (SELECT Sno FROM SC WHERE Grade IS NULL)",
        "SELECT Sno FROM Student WHERE Sage NOT IN (SELECT Grade FROM SC)",
        "SELECT S.Sno FROM Student S WHERE EXISTS(SELECT 1 FROM SC C WHERE C.Sno=S.Sno)",
        "SELECT S.Sno,(SELECT MAX(Grade) FROM SC WHERE SC.Sno=S.Sno) AS Grade FROM Student S",
        "SELECT S.Sno FROM Student S WHERE S.Sage>(SELECT AVG(T.Sage) FROM Student T WHERE T.Sdept=S.Sdept)",
        "WITH X AS (SELECT Sno FROM Student WHERE Sage>=18) SELECT X.Sno FROM X ORDER BY Sno LIMIT 2",
        "WITH X AS (SELECT Sno FROM SC), Y AS (SELECT Sno FROM X) SELECT * FROM Y",
        "SELECT Sno FROM Student UNION SELECT Sno FROM SC ORDER BY Sno",
        "SELECT Sno FROM SC UNION ALL SELECT Sno FROM SC",
        "SELECT Sno FROM Student EXCEPT SELECT Sno FROM SC",
        "SELECT Sno FROM Student INTERSECT SELECT Sno FROM SC",
        "SELECT 1,NULL,2+3",
        "SELECT * FROM Student S JOIN SC ON S.Sno=SC.Sno JOIN Course C ON C.Cno=SC.Cno",
    ];
    for sql in queries {
        let result = trace::trace_query(&db, sql, &Budget::default());
        assert_eq!(result.status, "completed", "{sql}\n{}", result.message);
        assert!(result.verified, "{sql}");
    }
}

#[test]
fn unsupported_trace_never_fakes_a_complete_result() {
    let db = database::memory("student").unwrap();
    for sql in [
        "SELECT random()",
        "SELECT ROW_NUMBER() OVER () FROM Student",
        "DELETE FROM Student",
    ] {
        let result = trace::trace_query(&db, sql, &Budget::default());
        assert_eq!(result.status, "unsupported");
        assert!(!result.verified);
        assert!(result.steps.is_empty());
    }
}
