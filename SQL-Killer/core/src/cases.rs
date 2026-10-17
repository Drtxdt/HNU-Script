//! A versioned, read-only investigation; repair validation is a finite model,
//! deliberately separate from a native MySQL experiment.
use crate::{
    database,
    exercises::{compare_tables, TableDiff},
    model::*,
    schedule, trace,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use ts_rs::TS;

pub const ID: &str = "enrollment-41";
pub const VERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseTask {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub starter: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseSpec {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub briefing: String,
    pub tasks: Vec<CaseTask>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct EvidenceSnapshot {
    pub id: String,
    pub task: String,
    pub sql: String,
    pub table: Table,
    pub case_version: u32,
    pub seed: u64,
    pub snapshot: String,
    pub note: String,
    pub hypothesis: String,
    pub links: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseProgress {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub board: crate::case_experience::EvidenceBoard,
    #[serde(default)]
    pub closure_revision: u32,
    #[serde(default)]
    pub round: u32,
    #[serde(default)]
    pub history: Vec<crate::case_experience::CaseConclusion>,
    pub version: u32,
    pub seed: u64,
    pub task: String,
    pub answers: BTreeMap<String, String>,
    pub completed: Vec<String>,
    pub evidence: Vec<EvidenceSnapshot>,
    pub cause: String,
    pub model_strategy: String,
    pub mysql_strategy: String,
    pub closed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseGrade {
    pub status: String,
    pub message: String,
    pub diff: Option<TableDiff>,
    pub expected: Option<Table>,
    pub actual: Option<Table>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct RepairScenario {
    pub name: String,
    pub passed: bool,
    pub actual: usize,
    pub counter: usize,
    pub schedules: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct RepairValidation {
    pub status: String,
    pub mode: String,
    pub strategy: String,
    pub scenarios: Vec<RepairScenario>,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct QueryComparison {
    pub steps: Vec<crate::comparison::ComparisonStep>,
    pub first_difference: Option<usize>,
    pub left: Trace,
    pub right: Trace,
    pub diff: TableDiff,
}
pub fn spec() -> CaseSpec {
    let tasks = [
        ("capacity","核对选课人数","找出成功选课人数超过容量的课程。输出 Cno、capacity、actual；只统计成功记录。","SELECT C.Cno,C.capacity,COUNT(*) AS actual\nFROM CaseCourse C JOIN Enrollment E ON E.Cno=C.Cno\nWHERE E.status='success'\nGROUP BY C.Cno,C.capacity\nHAVING ...;"),
        ("requests","核实异常请求","核实异常课程中有日志记录的成功请求及学生身份，同名不等于同一个人。输出 request_id、Sno、Sname，每个请求一次。","SELECT DISTINCT E.request_id,S.Sno,S.Sname\nFROM Enrollment E JOIN CaseStudent S ON ...\nJOIN RequestLog L ON ...\nWHERE ...;"),
        ("timeline","重建事件顺序","按事件序号呈现请求的实际操作。输出 event_seq、request_id、phase、observed_count，按 event_seq 排序。","SELECT event_seq,request_id,phase,observed_count\nFROM RequestLog\nORDER BY ...;"),
        ("reproduce","检验并发解释","根据已取得的证据预测并回放调度，填写实际成功人数、计数器和根因。",""),
        ("repair","验证修复方案","比较策略在容量、计数一致性、重复请求和失败回滚下的表现，再完成解释链。",""),
    ].into_iter().map(|(id,title,prompt,starter)|CaseTask{id:id.into(),title:title.into(),prompt:prompt.into(),starter:starter.into()}).collect();
    CaseSpec { id:ID.into(),version:VERSION,title:"选课异常调查".into(),briefing:"教务处收到选课记录与系统显示不一致的投诉。管理员认为数据正常，请用可复核的记录给出调查结论。调查使用独立数据，不改变自由实验数据库。".into(),tasks }
}
pub fn check_context(id: &str, version: u32) -> Result<(), String> {
    if id != ID || version != VERSION {
        Err("案件内容版本不匹配；旧证据保留为历史，请重新开始或验证当前版本".into())
    } else {
        Ok(())
    }
}
pub fn snapshot(seed: u64) -> String {
    format!("{ID}:v{VERSION}:{seed}")
}
pub fn fixture(seed: u64, variant: usize) -> Result<rusqlite::Connection, String> {
    let c = database::memory("empty")?;
    c.execute_batch("CREATE TABLE CaseStudent(Sno TEXT PRIMARY KEY,Sname TEXT NOT NULL); CREATE TABLE CaseCourse(Cno TEXT PRIMARY KEY,Cname TEXT,capacity INTEGER,enrolled_count INTEGER); CREATE TABLE Enrollment(request_id TEXT PRIMARY KEY,Sno TEXT REFERENCES CaseStudent,Cno TEXT REFERENCES CaseCourse,status TEXT,UNIQUE(Sno,Cno)); CREATE TABLE RequestLog(event_seq INTEGER PRIMARY KEY,request_id TEXT,tx TEXT,phase TEXT,observed_count INTEGER,event_time TEXT);").map_err(failure)?;
    let course = format!("C{}_{variant}", seed % 100000);
    c.execute(
        "INSERT INTO CaseCourse VALUES(?1,'数据库原理',40,40)",
        [&course],
    )
    .map_err(failure)?;
    c.execute("INSERT INTO CaseCourse VALUES('other','未满课程',50,0)", [])
        .map_err(failure)?;
    for n in 1..=44 {
        let student = format!("S{}_{}", variant * 100 + n, seed % 100000);
        c.execute(
            "INSERT INTO CaseStudent VALUES(?1,?2)",
            rusqlite::params![
                student,
                if n >= 40 {
                    "同名学生".into()
                } else {
                    format!("学生{n}")
                }
            ],
        )
        .map_err(failure)?;
        let request = format!("R{}_{}", variant * 100 + n, seed % 100000);
        c.execute(
            "INSERT INTO Enrollment VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                request,
                student,
                course,
                if n <= 41 { "success" } else { "cancelled" }
            ],
        )
        .map_err(failure)?;
    }
    let events = [
        (40, "T1", "read", Some(39)),
        (41, "T2", "read", Some(39)),
        (40, "T1", "write", Some(40)),
        (40, "T1", "commit", None),
        (41, "T2", "write", Some(40)),
        (41, "T2", "commit", None),
    ];
    for (i, (n, tx, phase, value)) in events.into_iter().enumerate() {
        c.execute(
            "INSERT INTO RequestLog VALUES(?1,?2,?3,?4,?5,?6)",
            rusqlite::params![
                i + 1 + variant * 10,
                format!("R{}_{}", variant * 100 + n, seed % 100000),
                tx,
                phase,
                value,
                format!("2026-09-01 09:00:{:02}", i / 2)
            ],
        )
        .map_err(failure)?;
    }
    Ok(c)
}
pub fn query(c: &rusqlite::Connection, sql: &str, b: &Budget) -> Result<Table, String> {
    b.check()?;
    if sql.len() > 65536 {
        return Err("调查 SQL 超过 64 KiB 限制".into());
    }
    let statements = crate::script::split_script(sql);
    if statements.len() != 1 {
        return Err("调查请使用一条只读查询".into());
    }
    let stmt = c.prepare(sql).map_err(failure)?;
    if !stmt.readonly()
        || stmt.column_count() == 0
        || !matches!(
            crate::script::tokenize(sql)
                .first()
                .map(|t| t.text.to_uppercase())
                .as_deref(),
            Some("SELECT" | "WITH")
        )
    {
        return Err("调查只允许 SELECT 或 WITH 查询".into());
    }
    drop(stmt);
    let t = database::query(c, sql, b)?;
    if t.truncated {
        return Err("调查结果超过限制，不能用于证据或判题".into());
    }
    Ok(t)
}
fn reference(task: &str) -> Result<&'static str, String> {
    Ok(match task {
    "capacity"=>"SELECT C.Cno,C.capacity,COUNT(*) actual FROM CaseCourse C JOIN Enrollment E ON E.Cno=C.Cno WHERE E.status='success' GROUP BY C.Cno,C.capacity HAVING COUNT(*)>C.capacity",
    "requests"=>"SELECT DISTINCT E.request_id,S.Sno,S.Sname FROM Enrollment E JOIN CaseStudent S ON S.Sno=E.Sno JOIN RequestLog L ON L.request_id=E.request_id JOIN CaseCourse C ON C.Cno=E.Cno WHERE E.status='success' AND E.Cno IN (SELECT E2.Cno FROM Enrollment E2 JOIN CaseCourse C2 ON C2.Cno=E2.Cno WHERE E2.status='success' GROUP BY E2.Cno,C2.capacity HAVING COUNT(*)>C2.capacity)",
    "timeline"=>"SELECT event_seq,request_id,phase,observed_count FROM RequestLog ORDER BY event_seq",
    _=>return Err("此任务请使用调度观察或修复验证".into())})
}
pub fn grade(task: &str, sql: &str, seed: u64, b: &Budget) -> CaseGrade {
    let run = (|| -> Result<CaseGrade, String> {
        if task == "reproduce" {
            let answer: serde_json::Value = serde_json::from_str(sql).map_err(failure)?;
            let passed =
                answer == serde_json::json!({"actual":41,"counter":40,"cause":"stale_read"});
            return Ok(CaseGrade {
                status: if passed { "passed" } else { "failed" }.into(),
                message: "实际人数由成功记录确定；计数器发生基于旧值的覆盖写入。".into(),
                diff: None,
                expected: None,
                actual: None,
            });
        }
        for variant in 0..3 {
            b.check()?;
            let c = fixture(seed, variant)?;
            let expected = query(&c, reference(task)?, b)?;
            let actual = query(&c, sql, b)?;
            let diff = compare_tables(&expected, &actual, task == "timeline", false);
            if !diff.equal {
                return Ok(CaseGrade {
                    status: "failed".into(),
                    message: if variant == 0 {
                        "公开数据结果不同；核对输出列、状态和重复次数"
                    } else {
                        "隐藏边界数据未通过；请避免固定学号、课程号和事件序号"
                    }
                    .into(),
                    diff: (variant == 0).then_some(diff),
                    expected: (variant == 0).then_some(expected),
                    actual: (variant == 0).then_some(actual),
                });
            }
        }
        Ok(CaseGrade {
            status: "passed".into(),
            message: "通过三组隔离数据；接受符合结果约定的不同 SQL 解法".into(),
            diff: None,
            expected: None,
            actual: None,
        })
    })();
    run.unwrap_or_else(|e| CaseGrade {
        status: if b.check().is_err() || e.contains("限制") {
            "incomplete"
        } else {
            "failed"
        }
        .into(),
        message: e,
        diff: None,
        expected: None,
        actual: None,
    })
}
pub fn reproduction(b: &Budget) -> Result<(String, schedule::Simulation), String> {
    let source="T1 read Count\nT2 read Count\nT1 set Count Count+1\nT1 write Count\nT1 insert E40 1\nT1 commit\nT2 set Count Count+1\nT2 write Count\nT2 insert E41 1\nT2 commit";
    let mut initial = BTreeMap::from([("Count".into(), 39)]);
    for i in 1..=39 {
        initial.insert(format!("E{i}"), 1);
    }
    Ok((
        source.into(),
        schedule::simulate_schedule(source, initial, "raw", "ru", b)?,
    ))
}

#[derive(Clone, Default)]
struct Worker {
    pc: usize,
    read: usize,
    pending: usize,
    inserted: bool,
}
#[derive(Clone)]
struct Model {
    count: usize,
    rows: usize,
    requests: BTreeSet<String>,
    workers: [Worker; 2],
    lock: Option<usize>,
}
fn explore(
    state: Model,
    strategy: &str,
    capacity: usize,
    duplicate: bool,
    fail: bool,
    b: &Budget,
    out: &mut Vec<(usize, usize)>,
) -> Result<(), String> {
    b.check()?;
    if state.workers.iter().all(|w| w.pc == 5) {
        out.push((state.rows, state.count));
        return Ok(());
    }
    for i in 0..2 {
        let mut s = state.clone();
        let pc = s.workers[i].pc;
        if pc == 5 {
            continue;
        }
        if (strategy == "row_lock" && pc == 0 || pc == 2) && s.lock.is_some_and(|owner| owner != i)
        {
            continue;
        }
        match pc {
            0 => {
                if strategy == "row_lock" {
                    s.lock = Some(i)
                }
                s.workers[i].read = s.count;
                s.workers[i].pc = 1;
            }
            1 => {
                s.workers[i].pc = if s.workers[i].read < capacity { 2 } else { 5 };
                if s.workers[i].pc == 5 && s.lock == Some(i) {
                    s.lock = None
                }
            }
            2 => {
                s.lock = Some(i);
                let n = if strategy == "conditional_update" {
                    s.count
                } else {
                    s.workers[i].read
                };
                if strategy == "conditional_update" && n >= capacity {
                    s.lock = None;
                    s.workers[i].pc = 5
                } else {
                    s.workers[i].pending = n + 1;
                    s.workers[i].pc = 3
                }
            }
            3 => {
                let key = if duplicate {
                    "same".into()
                } else {
                    format!("request{i}")
                };
                if fail && i == 1 || s.requests.contains(&key) {
                    s.workers[i].pc = 5;
                    s.lock = None
                } else {
                    s.workers[i].inserted = true;
                    s.workers[i].pc = 4
                }
            }
            4 => {
                s.count = s.workers[i].pending;
                if s.workers[i].inserted {
                    s.rows += 1;
                    s.requests.insert(if duplicate {
                        "same".into()
                    } else {
                        format!("request{i}")
                    });
                }
                s.workers[i].pc = 5;
                s.lock = None
            }
            _ => unreachable!(),
        }
        explore(s, strategy, capacity, duplicate, fail, b, out)?;
    }
    Ok(())
}
pub fn validate_repair(strategy: &str, b: &Budget) -> Result<RepairValidation, String> {
    if !["naive", "unique_only", "row_lock", "conditional_update"].contains(&strategy) {
        return Err("未知修复策略".into());
    }
    let mut scenarios = Vec::new();
    for (name, initial, duplicate, fail, expected) in [
        ("剩一个名额", 39, false, false, 40),
        ("已满", 40, false, false, 40),
        ("空课程", 0, false, false, 2),
        ("重复请求", 0, true, false, 1),
        ("插入失败回滚", 0, false, true, 1),
    ] {
        let state = Model {
            count: initial,
            rows: initial,
            requests: BTreeSet::new(),
            workers: Default::default(),
            lock: None,
        };
        let mut outcomes = Vec::new();
        explore(state, strategy, 40, duplicate, fail, b, &mut outcomes)?;
        let valid =
            |&(rows, count): &(usize, usize)| rows == count && rows <= 40 && rows == expected;
        let (actual, counter) = outcomes
            .iter()
            .find(|o| !valid(o))
            .copied()
            .unwrap_or(outcomes[0]);
        scenarios.push(RepairScenario {
            name: name.into(),
            passed: outcomes.iter().all(valid),
            actual,
            counter,
            schedules: outcomes.len(),
        });
    }
    Ok(RepairValidation{status:if scenarios.iter().all(|s|s.passed){"passed"}else{"failed"}.into(),mode:"model".into(),strategy:strategy.into(),scenarios,message:"枚举两个请求在有限事务模型中的所有允许交错；仅代表所列模型与场景。唯一约束防重复，不能单独保证课程容量。".into()})
}
pub fn validate_evidence(cards: &[EvidenceSnapshot]) -> Result<(), String> {
    if cards.len() > 20 || serde_json::to_vec(cards).map_err(failure)?.len() > 4 * 1024 * 1024 {
        return Err("证据板超过 20 张卡或 4 MiB".into());
    }
    let mut ids = BTreeSet::new();
    for e in cards {
        if !ids.insert(&e.id)
            || e.id.is_empty()
            || e.table.rows.len() > 200
            || e.sql.len() > 65536
            || serde_json::to_vec(e).map_err(failure)?.len() > 256 * 1024
        {
            return Err("证据卡重复或超过 200 行 / 256 KiB".into());
        }
    }
    for e in cards {
        if e.links.iter().any(|id| !ids.contains(id) || id == &e.id) {
            return Err("证据关联不存在或指向自己".into());
        }
    }
    Ok(())
}
pub fn close_case(p: &CaseProgress, b: &Budget) -> Result<CaseGrade, String> {
    let review = crate::case_experience::review(p, b)?;
    if !review.can_close {
        return Err(review
            .items
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.title.as_str())
            .collect::<Vec<_>>()
            .join("；"));
    }
    Ok(CaseGrade {
        status: "passed".into(),
        message: "案件已结案：查询、日志、并发复现和模型修复相互支持。MySQL 实测单独记录。".into(),
        diff: None,
        expected: None,
        actual: None,
    })
}
pub fn compare_queries(
    c: &rusqlite::Connection,
    left: &str,
    right: &str,
    b: &Budget,
) -> Result<QueryComparison, String> {
    let frozen = database::snapshot(c)?;
    let left = trace::trace_query(&frozen, left, b);
    let right = trace::trace_query(&frozen, right, b);
    if !left.verified || !right.verified {
        return Err(format!(
            "仅支持可完整生成过程的确定性只读查询：{}；{}",
            left.message, right.message
        ));
    }
    let diff = compare_tables(&left.result, &right.result, false, false);
    let steps =
        crate::comparison::align(&left, &right, left.source.as_str(), right.source.as_str());
    let first_difference = steps
        .iter()
        .position(|s| s.diff.as_ref().is_some_and(|d| !d.equal));
    Ok(QueryComparison {
        left,
        right,
        diff,
        steps,
        first_difference,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strategies_and_reproduction() {
        for s in ["row_lock", "conditional_update"] {
            assert_eq!(
                validate_repair(s, &Budget::default()).unwrap().status,
                "passed",
                "{s}"
            )
        }
        for s in ["naive", "unique_only"] {
            assert_eq!(
                validate_repair(s, &Budget::default()).unwrap().status,
                "failed"
            )
        }
        let (_, s) = reproduction(&Budget::default()).unwrap();
        assert_eq!(s.steps.last().unwrap().data["Count"], 40);
    }
    #[test]
    fn queries_are_semantic_isolated_and_versioned() {
        for task in ["capacity", "requests", "timeline"] {
            assert_eq!(
                grade(task, reference(task).unwrap(), 42, &Budget::default()).status,
                "passed"
            )
        }
        assert_eq!(
            grade("capacity", "SELECT 'C42_0',40,41", 42, &Budget::default()).status,
            "failed"
        );
        assert!(query(
            &fixture(0, 0).unwrap(),
            "DELETE FROM Enrollment",
            &Budget::default()
        )
        .is_err());
        assert!(check_context(ID, 2).is_err());
    }
    #[test]
    fn closure_rechecks_answers_and_rejects_tampered_or_stale_evidence() {
        let mut p = CaseProgress {
            version: VERSION,
            cause: "stale_read".into(),
            model_strategy: "row_lock".into(),
            ..Default::default()
        };
        for task in ["capacity", "requests", "timeline"] {
            let sql = reference(task).unwrap();
            p.answers.insert(task.into(), sql.into());
            let table = query(&fixture(0, 0).unwrap(), sql, &Budget::default()).unwrap();
            p.evidence.push(EvidenceSnapshot {
                id: task.into(),
                task: task.into(),
                sql: sql.into(),
                table,
                case_version: VERSION,
                seed: 0,
                snapshot: snapshot(0),
                note: String::new(),
                hypothesis: "stale_read".into(),
                links: vec![],
            });
        }
        p.answers.insert(
            "reproduce".into(),
            r#"{"actual":41,"counter":40,"cause":"stale_read"}"#.into(),
        );
        p.board.relations = vec![
            crate::case_experience::EvidenceRelation {
                from: "capacity".into(),
                to: "requests".into(),
                kind: "explains".into(),
            },
            crate::case_experience::EvidenceRelation {
                from: "requests".into(),
                to: "timeline".into(),
                kind: "explains".into(),
            },
        ];
        assert_eq!(close_case(&p, &Budget::default()).unwrap().status, "passed");
        let mut disconnected = p.clone();
        disconnected.board.relations.clear();
        assert!(close_case(&disconnected, &Budget::default()).is_err());
        let mut bad = p.clone();
        bad.evidence[0].snapshot = "old".into();
        assert!(close_case(&bad, &Budget::default()).is_err());
        let mut bad = p.clone();
        bad.evidence[0].table.rows[0][2] = Cell::Integer("40".into());
        assert!(close_case(&bad, &Budget::default()).is_err());
        let mut bad = p.clone();
        bad.answers.insert("capacity".into(), "SELECT 1".into());
        assert!(close_case(&bad, &Budget::default()).is_err());
        p.model_strategy = "unique_only".into();
        assert!(close_case(&p, &Budget::default()).is_err());
    }
}
