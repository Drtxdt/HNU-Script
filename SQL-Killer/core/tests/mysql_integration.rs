use sql_killer_core::{
    database::mysql::*, exercises, model::*, mysql_async::prelude::Queryable, trace,
};

// Only targets the separately initialized local QA instance. No developer service is used.
#[tokio::test]
#[ignore = "requires tools/mysql-qa.ps1 isolated server on port 13367"]
async fn real_mysql_adapter_and_two_sessions() {
    let cfg = MysqlConfig {
        host: "127.0.0.1".into(),
        port: 13367,
        user: "root".into(),
        password: String::new(),
        database: String::new(),
        tls: false,
    };
    let mut admin = connect(&cfg).await.unwrap();
    let name = format!("sqlkiller_qa_{}", std::process::id());
    create_sandbox(&mut admin, &name, "student").await.unwrap();
    let cfg = MysqlConfig {
        database: name.clone(),
        ..cfg
    };
    let mut a = connect(&cfg).await.unwrap();
    let mut b = connect(&cfg).await.unwrap();
    assert_eq!(
        schema(&mut a)
            .await
            .unwrap()
            .relations
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case("SC"))
            .unwrap()
            .foreign_keys
            .len(),
        2
    );
    assert!(a
        .query_drop("INSERT INTO SC VALUES('missing','1',80)")
        .await
        .is_err());
    let result = execute(
        &mut a,
        "SELECT 9007199254740993 AS n, CAST(1.25 AS DECIMAL(20,2)) AS d, NULL AS z",
        &Budget::default(),
    )
    .await;
    assert_eq!(result.status, "completed", "{:?}", result.error);
    assert_eq!(
        result.results[0].table.rows[0][0],
        Cell::Integer("9007199254740993".into())
    );
    assert_eq!(
        result.results[0].table.rows[0][1],
        Cell::Decimal("1.25".into())
    );
    let result = trace::trace_mysql(
        &cfg,
        "SELECT Sdept,COUNT(*) FROM Student GROUP BY Sdept",
        &Budget::default(),
    )
    .await;
    assert_eq!(result.status, "completed", "{}", result.message);
    a.query_drop("START TRANSACTION").await.unwrap();
    a.query_drop("UPDATE Student SET Sage=30 WHERE Sno='2021310725'")
        .await
        .unwrap();
    let read = query(
        &mut b,
        "SELECT Sage FROM Student WHERE Sno='2021310725'",
        &Budget::default(),
    )
    .await
    .unwrap();
    assert_ne!(read.rows[0][0], Cell::Integer("30".into()));
    let pending = async {
        b.query_drop("UPDATE Student SET Sage=31 WHERE Sno='2021310725'")
            .await
    };
    let release = async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        a.query_drop("ROLLBACK").await.unwrap()
    };
    let (start, ()) = tokio::join!(pending, release);
    start.unwrap();
    // InnoDB deadlock must be observed from the server, not inferred from a model.
    a.query_drop("START TRANSACTION").await.unwrap();
    b.query_drop("START TRANSACTION").await.unwrap();
    a.query_drop("UPDATE Student SET Sage=Sage+1 WHERE Sno='2021310721'")
        .await
        .unwrap();
    b.query_drop("UPDATE Student SET Sage=Sage+1 WHERE Sno='2021310722'")
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        a.query_drop("UPDATE Student SET Sage=Sage+1 WHERE Sno='2021310722'"),
        b.query_drop("UPDATE Student SET Sage=Sage+1 WHERE Sno='2021310721'")
    );
    assert!(
        left.as_ref()
            .err()
            .is_some_and(|e| e.to_string().contains("1213"))
            || right
                .as_ref()
                .err()
                .is_some_and(|e| e.to_string().contains("1213"))
    );
    a.query_drop("ROLLBACK").await.unwrap();
    b.query_drop("ROLLBACK").await.unwrap();
    let multi=execute(&mut a,"CREATE PROCEDURE multi_results() BEGIN SELECT 1 AS first; SELECT 2 AS second; END; CALL multi_results();",&Budget::default()).await;
    assert_eq!(multi.status, "completed", "{:?}", multi.error);
    assert_eq!(
        multi
            .results
            .iter()
            .filter(|r| !r.table.columns.is_empty())
            .count(),
        2
    );
    for q in exercises::catalog(0)
        .into_iter()
        .filter(|q| q.engine == "mysql")
    {
        let result = exercises::grade_mysql(&cfg, &q.id, &q.solution, 0, &Budget::default()).await;
        assert_eq!(result.status, "passed", "{}: {}", q.id, result.message);
    }
    a.disconnect().await.unwrap();
    b.disconnect().await.unwrap();
    admin
        .query_drop(format!("DROP DATABASE `{name}`"))
        .await
        .unwrap();
    admin.disconnect().await.unwrap();
}
