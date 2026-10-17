//! Native repair checks. All DDL targets a newly created, uniquely named database.
use crate::{
    cases::{RepairScenario, RepairValidation},
    database::mysql::{connect, MysqlConfig},
    model::{failure, Budget},
};
use mysql_async::{prelude::Queryable, Conn};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Barrier;

async fn worker(
    c: &mut Conn,
    strategy: &str,
    barrier: Arc<Barrier>,
    request: &str,
    student: usize,
    fail: bool,
    b: &Budget,
) -> Result<(), String> {
    c.query_drop("START TRANSACTION").await.map_err(failure)?;
    let work=async {
        b.check()?;
        let naive=matches!(strategy,"naive"|"unique_only");
        let mut old=0;
        if naive { old=c.query_first::<usize,_>("SELECT enrolled_count FROM CaseCourse WHERE Cno='C1'").await.map_err(failure)?.ok_or("缺少课程")?; }
        // Both unsafe reads finish before either unsafe write. Safe strategies
        // rendezvous before acquiring the row lock, so the harness cannot deadlock.
        barrier.wait().await;
        if strategy=="conditional_update" {
            c.query_drop("UPDATE CaseCourse SET enrolled_count=enrolled_count+1 WHERE Cno='C1' AND enrolled_count<capacity").await.map_err(failure)?;
            if c.affected_rows()!=1 {c.query_drop("ROLLBACK").await.map_err(failure)?;return Ok(())}
        } else {
            if !naive {old=c.query_first::<usize,_>("SELECT enrolled_count FROM CaseCourse WHERE Cno='C1' FOR UPDATE").await.map_err(failure)?.ok_or("缺少课程")?;}
            if old>=40 {c.query_drop("ROLLBACK").await.map_err(failure)?;return Ok(())}
            c.exec_drop("UPDATE CaseCourse SET enrolled_count=? WHERE Cno='C1'",(old+1,)).await.map_err(failure)?;
        }
        b.check()?;
        if fail {c.query_drop("INSERT INTO Enrollment(request_id,Sno,Cno,status) VALUES(NULL,999,'C1','success')").await.map_err(failure)?;}
        c.exec_drop("INSERT INTO Enrollment VALUES(?,?,'C1','success')",(request,student)).await.map_err(failure)?;
        c.query_drop("COMMIT").await.map_err(failure)?;
        Ok(())
    }.await;
    if work.is_err() {
        let _ = c.query_drop("ROLLBACK").await;
    }
    work
}

pub async fn validate(
    cfg: &MysqlConfig,
    strategy: &str,
    b: &Budget,
) -> Result<RepairValidation, String> {
    if !["naive", "unique_only", "row_lock", "conditional_update"].contains(&strategy) {
        return Err("未知修复策略".into());
    }
    let name = format!(
        "sqlkiller_case_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(failure)?
            .as_nanos()
    );
    let mut admin = connect(cfg).await?;
    admin
        .query_drop(format!("CREATE DATABASE `{name}`"))
        .await
        .map_err(failure)?;
    let mut isolated = cfg.clone();
    isolated.database = name.clone();
    let run = async {
        let mut a = connect(&isolated).await?;
        let mut c = connect(&isolated).await?;
        a.query_drop("SET SESSION innodb_lock_wait_timeout=2")
            .await
            .map_err(failure)?;
        c.query_drop("SET SESSION innodb_lock_wait_timeout=2")
            .await
            .map_err(failure)?;
        a.query_drop("CREATE TABLE CaseCourse(Cno VARCHAR(255) PRIMARY KEY,capacity INT NOT NULL,enrolled_count INT NOT NULL) ENGINE=InnoDB").await.map_err(failure)?;
        a.query_drop("CREATE TABLE Enrollment(request_id VARCHAR(255) PRIMARY KEY,Sno INT NOT NULL,Cno VARCHAR(255) NOT NULL,status VARCHAR(20) NOT NULL,UNIQUE(Sno,Cno)) ENGINE=InnoDB").await.map_err(failure)?;
        let mut scenarios = Vec::new();
        for (title, initial, duplicate, fail, expected) in [
            ("剩一个名额", 39, false, false, 40),
            ("已满", 40, false, false, 40),
            ("空课程", 0, false, false, 2),
            ("重复请求", 0, true, false, 1),
            ("插入失败回滚", 0, false, true, 1),
        ] {
            b.check()?;
            a.query_drop("DELETE FROM Enrollment")
                .await
                .map_err(failure)?;
            a.query_drop("DELETE FROM CaseCourse")
                .await
                .map_err(failure)?;
            a.exec_drop("INSERT INTO CaseCourse VALUES('C1',40,?)", (initial,))
                .await
                .map_err(failure)?;
            for n in 0..initial {
                a.exec_drop(
                    "INSERT INTO Enrollment VALUES(?,?,'C1','success')",
                    (format!("old{n}"), n),
                )
                .await
                .map_err(failure)?;
            }
            let barrier = Arc::new(Barrier::new(2));
            let (x, y) = tokio::join!(
                worker(
                    &mut a,
                    strategy,
                    barrier.clone(),
                    if duplicate { "same" } else { "newA" },
                    100,
                    false,
                    b
                ),
                worker(
                    &mut c,
                    strategy,
                    barrier,
                    if duplicate { "same" } else { "newB" },
                    if duplicate { 100 } else { 101 },
                    fail,
                    b
                )
            );
            // An insert error is expected only in the failure and duplicate probes.
            if !duplicate && !fail {
                x?;
                y?;
            }
            let actual = a
                .query_first::<usize, _>("SELECT COUNT(*) FROM Enrollment WHERE status='success'")
                .await
                .map_err(failure)?
                .ok_or("没有计数结果")?;
            let counter = a
                .query_first::<usize, _>("SELECT enrolled_count FROM CaseCourse WHERE Cno='C1'")
                .await
                .map_err(failure)?
                .ok_or("没有计数器结果")?;
            scenarios.push(RepairScenario {
                name: title.into(),
                passed: actual == counter && actual == expected && actual <= 40,
                actual,
                counter,
                schedules: 1,
            });
        }
        Ok::<_,String>(RepairValidation{status:if scenarios.iter().all(|s|s.passed){"passed"}else{"failed"}.into(),mode:"mysql".into(),strategy:strategy.into(),scenarios,message:"真实 InnoDB 双会话验证；原方案强制两次读取先于覆盖写入。请求等待不被推断为已观测到服务器锁等待。仅代表这些测试场景。".into()})
    };
    let result = tokio::time::timeout(Duration::from_secs(9), run)
        .await
        .map_err(|_| "MySQL 验证超时，未授予通过状态".to_string())
        .and_then(|r| r);
    // Dropping the experiment future also drops both sessions before cleanup.
    let cleanup = tokio::time::timeout(
        Duration::from_secs(3),
        admin.query_drop(format!("DROP DATABASE `{name}`")),
    )
    .await;
    match cleanup {
        Ok(Ok(())) => result,
        _ => Err(format!(
            "临时案件库 {name} 清理未确认；验证标记为未完成，请核对服务器状态"
        )),
    }
}
