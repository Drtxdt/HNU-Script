use sql_killer_core::{cases_mysql, database::mysql::MysqlConfig, model::Budget};
#[tokio::test]
#[ignore = "requires the isolated tools/mysql-qa.ps1 server on port 13367"]
async fn native_case_repair_checks_and_cleanup() {
    let cfg = MysqlConfig {
        host: "127.0.0.1".into(),
        port: 13367,
        user: "root".into(),
        password: String::new(),
        database: String::new(),
        tls: false,
    };
    for strategy in ["naive", "unique_only", "row_lock", "conditional_update"] {
        let r = cases_mysql::validate(&cfg, strategy, &Budget::default())
            .await
            .unwrap();
        assert_eq!(
            r.status,
            if matches!(strategy, "naive" | "unique_only") {
                "failed"
            } else {
                "passed"
            },
            "{strategy}: {}",
            r.message
        );
        assert_eq!(r.mode, "mysql");
        assert_eq!(r.scenarios.len(), 5);
    }
}
