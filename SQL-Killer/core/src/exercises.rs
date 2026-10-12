use crate::{algebra, database, model::*, script::tokenize};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeMap, str::FromStr};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Question {
    pub id: String,
    pub chapter: usize,
    pub chapter_title: String,
    pub title: String,
    pub prompt: String,
    pub starter: String,
    pub hint: String,
    pub solution: String,
    pub explanation: String,
    pub kind: String,
    pub engine: String,
    pub fixture: String,
    pub source: String,
    pub pages: String,
    pub ordered: bool,
    pub check_names: bool,
    pub probes: Vec<String>,
    pub seed: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Grade {
    pub status: String,
    pub message: String,
    pub cases: usize,
    pub expected: Option<Table>,
    pub actual: Option<Table>,
    pub explanation: String,
}
const CHAPTERS: [&str; 17] = [
    "关系、模式与元组",
    "键与完整性约束",
    "关系代数",
    "数据定义 DDL",
    "SELECT 与 WHERE",
    "NULL 与 LIKE",
    "聚合与分组",
    "连接 JOIN",
    "子查询",
    "集合运算",
    "数据修改 DML",
    "视图与索引",
    "存储过程与函数",
    "触发器",
    "事务与恢复性",
    "隔离级别",
    "可串行化调度",
];
pub fn catalog(seed: u64) -> Vec<Question> {
    let age = 17 + (seed % 4);
    let mut q = Vec::new();
    macro_rules! add {($id:expr,$ch:expr,$title:expr,$kind:expr,$prompt:expr,$answer:expr)=>{{let ch=$ch;let source=if ch<=2{"02-(1)关系数据模型 - 数据结构、约束和基本操作.pdf"}else if ch==3{"02-(2)关系数据模型 - 复杂关系代数.pdf"}else if ch<=6{"03-(1)SQL-基础数据定义语言、基础数据操作语言.pdf"}else if ch<=14{"03-(2)SQL-复杂数据操作、高级数据定义-无索引版.pdf"}else{"04-(1)事务的概念、性质和调度.pdf"};q.push(Question{id:$id.into(),chapter:ch,chapter_title:CHAPTERS[ch-1].into(),title:$title.into(),prompt:$prompt.to_string(),starter:if $kind=="query"{"SELECT\nFROM Student;"}else{""}.into(),hint:"先确认输出列、重复行、NULL 和边界数据，再检查语句作用范围。".into(),solution:$answer.into(),explanation:"系统在隔离数据上运行参考答案与用户答案，比较语义结果；通过测试不代表证明所有数据库上的等价。".into(),kind:$kind.into(),engine:"sqlite".into(),fixture:"student".into(),source:source.into(),pages:"参见对应章节；本题为依据课件编写的练习".into(),ordered:false,check_names:false,probes:vec![],seed});}}}
    add!("relation","1".parse::<usize>().unwrap(),"关系与元组","concept","填写 JSON：关系中一行的名称为 tuple，列的名称为 attribute。例：{\"row\":\"...\",\"column\":\"...\"}","{\"row\":\"tuple\",\"column\":\"attribute\"}");
    add!(
        "keys",
        2,
        "Student 的候选主键",
        "concept",
        "填写 JSON 数组：Student 中标识每名学生的主键列。",
        "[\"Sno\"]"
    );
    add!("constraint",2,"带检查的成绩表","schema","创建 GradeBook(Sno TEXT 主键且 NOT NULL, Grade INTEGER NOT NULL，范围 0 到 100)。","CREATE TABLE GradeBook(Sno TEXT PRIMARY KEY NOT NULL, Grade INTEGER NOT NULL CHECK(Grade BETWEEN 0 AND 100));");
    add!(
        "algebra-filter",
        3,
        "选择与投影",
        "algebra",
        "使用关系代数返回年龄大于等于 18 岁学生的 Sno、Sname。",
        "project{Sno,Sname}(select{Sage >= 18}(Student))"
    );
    add!(
        "algebra-join",
        3,
        "自然连接",
        "algebra",
        "返回选修 1 号课程的学生 Sno、Sname、Grade。",
        "project{Sno,Sname,Grade}(select{Cno = '1'}(join(Student,SC)))"
    );
    add!(
        "create",
        4,
        "创建课程笔记表",
        "schema",
        "创建 Note(Id INTEGER 主键, Body TEXT NOT NULL)。",
        "CREATE TABLE Note(Id INTEGER PRIMARY KEY, Body TEXT NOT NULL);"
    );
    add!(
        "alter",
        4,
        "增加备注字段",
        "schema",
        "为 Student 增加 Memo TEXT 列，保留原有数据。",
        "ALTER TABLE Student ADD COLUMN Memo TEXT;"
    );
    add!(
        "where",
        5,
        "按年龄筛选",
        "query",
        format!("返回年龄大于等于 {age} 的学生 Sno、Sname、Sage；不要求顺序。"),
        format!("SELECT Sno,Sname,Sage FROM Student WHERE Sage >= {age}")
    );
    add!(
        "projection",
        5,
        "选择输出列",
        "query",
        "按原数据重复次数返回 Sname、Sdept 两列，不要求排序。",
        "SELECT Sname,Sdept FROM Student"
    );
    add!(
        "null",
        6,
        "尚无成绩",
        "query",
        "返回成绩为 NULL 的选课记录 Sno、Cno。",
        "SELECT Sno,Cno FROM SC WHERE Grade IS NULL"
    );
    add!(
        "like",
        6,
        "姓张的学生",
        "query",
        "返回姓名以张开头的学生全部列。",
        "SELECT * FROM Student WHERE Sname LIKE '张%'"
    );
    add!(
        "aggregate",
        7,
        "各系平均年龄",
        "query",
        "返回 Sdept、平均年龄两列，只保留平均年龄大于 17 的系。",
        "SELECT Sdept,AVG(Sage) FROM Student GROUP BY Sdept HAVING AVG(Sage)>17"
    );
    add!(
        "count",
        7,
        "COUNT 的 NULL 行为",
        "query",
        "返回 SC 总行数和非 NULL 成绩数，共两列一行。",
        "SELECT COUNT(*),COUNT(Grade) FROM SC"
    );
    add!(
        "join",
        8,
        "学生成绩",
        "query",
        "返回有选课记录的学生 Sno、Sname、Cno、Grade。",
        "SELECT S.Sno,S.Sname,C.Cno,C.Grade FROM Student S JOIN SC C ON S.Sno=C.Sno"
    );
    add!(
        "left",
        8,
        "保留未选课学生",
        "query",
        "返回全部学生 Sno、Sname 及其 Cno，没有选课时 Cno 为 NULL。",
        "SELECT S.Sno,S.Sname,C.Cno FROM Student S LEFT JOIN SC C ON S.Sno=C.Sno"
    );
    add!(
        "subquery",
        9,
        "高于平均年龄",
        "query",
        "返回年龄高于全体学生平均年龄的 Sno、Sname。",
        "SELECT Sno,Sname FROM Student WHERE Sage>(SELECT AVG(Sage) FROM Student)"
    );
    add!("correlated",9,"高于本系平均年龄","query","返回年龄高于本系平均年龄的 Sno、Sname。","SELECT S.Sno,S.Sname FROM Student S WHERE S.Sage>(SELECT AVG(T.Sage) FROM Student T WHERE T.Sdept=S.Sdept)");
    add!(
        "exists",
        9,
        "没有选课的学生",
        "query",
        "返回没有任何 SC 记录的学生 Sno。",
        "SELECT S.Sno FROM Student S WHERE NOT EXISTS(SELECT 1 FROM SC WHERE SC.Sno=S.Sno)"
    );
    add!(
        "union",
        10,
        "去重合并",
        "query",
        "返回 CS 系或年龄为 18 的学生 Sno，去除重复。",
        "SELECT Sno FROM Student WHERE Sdept='CS' UNION SELECT Sno FROM Student WHERE Sage=18"
    );
    add!(
        "except",
        10,
        "集合差",
        "query",
        "返回所有学生学号与选课学号的差集。",
        "SELECT Sno FROM Student EXCEPT SELECT Sno FROM SC"
    );
    add!(
        "update",
        11,
        "单系年龄增加",
        "mutation",
        "只把 CS 系学生的 Sage 加 1，其余所有数据保持不变。",
        "UPDATE Student SET Sage=Sage+1 WHERE Sdept='CS';"
    );
    add!(
        "delete",
        11,
        "删除空成绩记录",
        "mutation",
        "只删除 SC 中 Grade 为 NULL 的记录。",
        "DELETE FROM SC WHERE Grade IS NULL;"
    );
    add!(
        "view",
        12,
        "创建 CS 视图",
        "schema",
        "创建 CSStudents 视图，包含 CS 系学生 Sno、Sname。",
        "CREATE VIEW CSStudents AS SELECT Sno,Sname FROM Student WHERE Sdept='CS';"
    );
    add!(
        "index",
        12,
        "创建年龄索引",
        "schema",
        "创建名为 idx_student_age 的 Student(Sage) 索引。",
        "CREATE INDEX idx_student_age ON Student(Sage);"
    );
    add!("procedure",13,"查询课程成绩的过程","routine","在 MySQL 创建过程 course_grades(IN p_cno VARCHAR(255))，返回该课 Sno、Grade，按 Sno 排序。","CREATE PROCEDURE course_grades(IN p_cno VARCHAR(255)) BEGIN SELECT Sno,Grade FROM SC WHERE Cno=p_cno ORDER BY Sno; END;");
    add!("function",13,"求和函数","routine","在 MySQL 创建函数 sum_to(n INT)，返回 1 到 n 的和；n<=0 时为 0。","CREATE FUNCTION sum_to(n INT) RETURNS BIGINT DETERMINISTIC NO SQL RETURN IF(n>0,n*(n+1)/2,0);");
    add!("trigger",14,"记录成绩变化","schema","创建 ChangeLog(Sno TEXT, OldGrade REAL, NewGrade REAL) 和触发器 grade_log：每次 UPDATE SC 的 Grade 后写入 OLD 与 NEW 成绩。","CREATE TABLE ChangeLog(Sno TEXT,OldGrade REAL,NewGrade REAL); CREATE TRIGGER grade_log AFTER UPDATE OF Grade ON SC BEGIN INSERT INTO ChangeLog VALUES(NEW.Sno,OLD.Grade,NEW.Grade); END;");
    add!(
        "rollback",
        15,
        "撤销修改",
        "mutation",
        "开启事务，将全部学生年龄改为 99，再回滚；保留原始数据库。",
        "BEGIN; UPDATE Student SET Sage=99; ROLLBACK;"
    );
    add!(
        "recovery",
        15,
        "不可恢复调度",
        "concept",
        "T1 写 A；T2 读 A；T2 提交；T1 回滚。填写 JSON：是否可恢复、是否无级联。",
        "{\"recoverable\":false,\"cascadeless\":false}"
    );
    add!(
        "isolation",
        16,
        "隔离级别矩阵",
        "concept",
        "按课本模型填写 JSON：读提交是否允许脏读、不可重复读、幻读。",
        "{\"dirty\":false,\"nonrepeatable\":true,\"phantom\":true}"
    );
    add!("snapshot",16,"MySQL 快照读","concept","MySQL REPEATABLE READ 中普通 SELECT 与 SELECT FOR UPDATE 是否使用相同读取机制？填写 JSON 布尔值。","false");
    add!("schedule",17,"冲突优先图","schedule","对 T1 read A; T2 write A; T1 write B; T2 read B 填写 JSON：{\"conflict_serializable\":布尔值,\"order\":[事务名,...]}。","{\"conflict_serializable\":true,\"order\":[\"T1\",\"T2\"]}");
    add!(
        "view-schedule",
        17,
        "盲写与视图等价",
        "concept",
        "T1 write A; T2 write A; T1 write A。填写 JSON：是否冲突可串行化、是否视图可串行化。",
        "{\"conflict_serializable\":false,\"view_serializable\":true}"
    );
    for question in &mut q {
        question.pages = match question.id.as_str() {
            "relation" => "pp.10–18",
            "keys" => "pp.20–22",
            "constraint" => "pp.27–34（SQLite 语法改编）",
            "algebra-filter" => "pp.13–14",
            "algebra-join" => "pp.41–43",
            "create" => "pp.9–14",
            "alter" => "pp.27–29",
            "where" => "pp.56–60",
            "projection" => "pp.54–55, 64–65",
            "null" => "pp.68–70",
            "like" => "pp.61–62",
            "aggregate" => "pp.18–26",
            "count" => "pp.14–17",
            "join" => "pp.29–30",
            "left" => "p.31",
            "subquery" => "pp.33–37",
            "correlated" => "pp.38–41",
            "exists" => "pp.42–45",
            "union" | "except" => "pp.50–51",
            "update" => "pp.55–58",
            "delete" => "pp.59–62",
            "view" => "pp.33–36",
            "index" => "pp.43–49",
            "procedure" => "pp.66–69",
            "function" => "pp.70–81",
            "trigger" => "pp.83–89（SQLite 语法改编）",
            "rollback" => "pp.11–12, 18",
            "recovery" => "pp.26–29",
            "isolation" | "snapshot" => "pp.37–48；MySQL 快照读为补充",
            "schedule" => "pp.54–61",
            _ => "pp.65–79",
        }
        .into();
        if matches!(question.id.as_str(), "view" | "index") {
            question.source = "03-(1)SQL-基础数据定义语言、基础数据操作语言.pdf".into();
        }
        match question.id.as_str() {
            "constraint" => {
                question.fixture = "empty".into();
                question.probes = vec![
                    "INSERT INTO GradeBook VALUES('s1',90)".into(),
                    "INSERT INTO GradeBook VALUES('s2',101)".into(),
                    "INSERT INTO GradeBook VALUES(NULL,50)".into(),
                    "INSERT INTO GradeBook VALUES('s3',NULL)".into(),
                    "INSERT INTO GradeBook VALUES('s4',-1)".into(),
                ];
            }
            "create" => question.fixture = "empty".into(),
            "trigger" => {
                question.probes = vec![
                    "UPDATE SC SET Grade=COALESCE(Grade,0)+1 WHERE Grade IS NULL OR Grade<99"
                        .into(),
                ]
            }
            "procedure" => {
                question.engine = "mysql".into();
                question.probes = vec![
                    "CALL course_grades('1')".into(),
                    "CALL course_grades('2')".into(),
                    "CALL course_grades('none')".into(),
                ];
                question.ordered = true;
            }
            "function" => {
                question.engine = "mysql".into();
                question.probes = vec!["SELECT sum_to(10),sum_to(0),sum_to(-1),sum_to(100)".into()];
            }
            "algebra-filter" => question.pages = "基本操作；复杂关系代数 pp.13–14".into(),
            "algebra-join" => question.pages = "pp.41–43".into(),
            "isolation" => question.pages = "pp.38–48".into(),
            "schedule" => question.pages = "pp.54–60".into(),
            "view-schedule" => question.pages = "pp.67–79".into(),
            _ => {}
        }
    }
    q
}
fn numeric(c: &Cell) -> Option<Decimal> {
    match c {
        Cell::Integer(s) | Cell::Unsigned(s) | Cell::Decimal(s) => Decimal::from_str(s).ok(),
        Cell::Real(v) => Decimal::from_f64_retain(*v),
        _ => None,
    }
}
fn exact_decimal(c: &Cell) -> Option<(bool, String, String)> {
    let text = match c {
        Cell::Integer(s) | Cell::Unsigned(s) | Cell::Decimal(s) => s,
        _ => return None,
    };
    let negative = text.starts_with('-');
    let text = text.trim_start_matches(['-', '+']);
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if !whole.bytes().all(|c| c.is_ascii_digit()) || !fraction.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    let fraction = fraction.trim_end_matches('0');
    Some((
        negative && (whole != "0" || !fraction.is_empty()),
        whole.into(),
        fraction.into(),
    ))
}
fn equal(a: &Cell, b: &Cell) -> bool {
    if a == b {
        return true;
    }
    if let (Some(x), Some(y)) = (exact_decimal(a), exact_decimal(b)) {
        return x == y;
    }
    match (numeric(a), numeric(b)) {
        (Some(x), Some(y)) => {
            if matches!(a, Cell::Real(_)) || matches!(b, Cell::Real(_)) {
                let x = x.to_string().parse::<f64>().unwrap_or(f64::NAN);
                let y = y.to_string().parse::<f64>().unwrap_or(f64::NAN);
                (x - y).abs() <= 1e-9_f64.max(1e-9 * x.abs().max(y.abs()))
            } else {
                x == y
            }
        }
        _ => false,
    }
}
fn row_order(a: &[Cell], b: &[Cell]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let o = if let (Some((xn, xi, xf)), Some((yn, yi, yf))) =
            (exact_decimal(x), exact_decimal(y))
        {
            if xn != yn {
                yn.cmp(&xn)
            } else {
                let order = xi.len().cmp(&yi.len()).then(xi.cmp(&yi)).then(xf.cmp(&yf));
                if xn {
                    order.reverse()
                } else {
                    order
                }
            }
        } else {
            match (numeric(x), numeric(y)) {
                (Some(x), Some(y)) => x.cmp(&y),
                _ => serde_json::to_string(x)
                    .unwrap_or_default()
                    .cmp(&serde_json::to_string(y).unwrap_or_default()),
            }
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}
pub fn tables_equal(a: &Table, b: &Table, ordered: bool, names: bool) -> bool {
    if a.truncated
        || b.truncated
        || a.columns.len() != b.columns.len()
        || a.rows.len() != b.rows.len()
        || names && a.columns != b.columns
    {
        return false;
    }
    let mut a = a.rows.iter().collect::<Vec<_>>();
    let mut b = b.rows.iter().collect::<Vec<_>>();
    if !ordered {
        a.sort_by(|x, y| row_order(x, y));
        b.sort_by(|x, y| row_order(x, y));
    }
    a.iter()
        .zip(b)
        .all(|(x, y)| x.len() == y.len() && x.iter().zip(y).all(|(x, y)| equal(x, y)))
}
fn grade_result(
    status: &str,
    message: impl ToString,
    cases: usize,
    expected: Option<Table>,
    actual: Option<Table>,
) -> Grade {
    Grade {
        status: status.into(),
        message: message.to_string(),
        cases,
        expected,
        actual,
        explanation: "按题目约定验证测试数据与行为；未完成的运行不会算作错误答案。".into(),
    }
}
pub fn fixture(name: &str, variant: usize) -> Result<rusqlite::Connection, String> {
    let c = database::memory(name)?;
    if name == "student" {
        match variant {
            1 => {
                c.execute_batch("INSERT INTO Student VALUES('x1','张三','男',22,'IS'),('x2','张三','女',16,'IS'),('x3','李博','男',19,'CS'); INSERT INTO SC VALUES('x1','1',NULL),('x2','1',88),('x3','3',90); UPDATE Student SET Sage=Sage+2 WHERE Sno='2021310725';").map_err(failure)?;
            }
            2 => {
                c.execute_batch("DELETE FROM SC; DELETE FROM Student;")
                    .map_err(failure)?;
            }
            _ => {}
        }
    }
    Ok(c)
}
pub fn grade(id: &str, answer: &str, seed: u64, b: &Budget) -> Grade {
    let Some(q) = catalog(seed).into_iter().find(|q| q.id == id) else {
        return grade_result("incomplete", "题目不存在", 0, None, None);
    };
    if q.engine == "mysql" {
        return grade_result(
            "incomplete",
            "本题需要连接 MySQL 专用教学沙箱",
            0,
            None,
            None,
        );
    }
    if matches!(q.kind.as_str(), "concept" | "schedule") {
        let a = serde_json::from_str::<serde_json::Value>(answer);
        let expected = serde_json::from_str::<serde_json::Value>(&q.solution).unwrap();
        return match a {
            Ok(value) => grade_result(
                if value == expected {
                    "passed"
                } else {
                    "failed"
                },
                if value == expected {
                    "概念回答正确"
                } else {
                    "请核对题目给定模型与操作次序"
                },
                1,
                None,
                None,
            ),
            Err(e) => grade_result("failed", format!("答案需要 JSON：{e}"), 0, None, None),
        };
    }
    let run = (|| -> Result<Grade, String> {
        for variant in 0..3 {
            b.check()?;
            let user = fixture(&q.fixture, variant)?;
            let reference = fixture(&q.fixture, variant)?;
            if q.kind == "algebra" {
                let a = algebra::evaluate_algebra(&user, answer, false, b)?
                    .trace
                    .result;
                let e = algebra::evaluate_algebra(&reference, &q.solution, false, b)?
                    .trace
                    .result;
                if !tables_equal(&a, &e, q.ordered, q.check_names) {
                    return Ok(grade_result(
                        "failed",
                        format!("数据组 {} 结果不同", variant + 1),
                        variant,
                        (variant == 0).then_some(e),
                        (variant == 0).then_some(a),
                    ));
                }
                continue;
            }
            if q.id == "rollback" {
                let mut observed = false;
                for s in crate::script::split_script(answer) {
                    if tokenize(&s.sql)
                        .first()
                        .is_some_and(|t| t.text.eq_ignore_ascii_case("ROLLBACK"))
                    {
                        let before = database::query(
                            &user,
                            "SELECT COUNT(*) FROM Student WHERE Sage IS NULL OR Sage<>99",
                            b,
                        )?;
                        observed = !user.is_autocommit()
                            && before.rows == vec![vec![Cell::Integer("0".into())]];
                    }
                    let r = database::execute(&user, &s.sql, b);
                    if r.status != "completed" {
                        return Ok(grade_result(
                            "failed",
                            r.error.unwrap_or_default(),
                            variant,
                            None,
                            None,
                        ));
                    }
                }
                if !observed
                    || !user.is_autocommit()
                    || !same_dump(&dump(&user, b)?, &dump(&reference, b)?)
                {
                    return Ok(grade_result(
                        "failed",
                        "需要在事务内实际将年龄改为 99，随后回滚恢复原值",
                        variant,
                        None,
                        None,
                    ));
                }
                continue;
            }
            let a = database::execute(&user, answer, b);
            let e = database::execute(&reference, &q.solution, b);
            if e.status != "completed" {
                return Err(format!("参考答案错误：{:?}", e.error));
            }
            if a.status != "completed" {
                if a.status == "limit" || a.status == "cancelled" {
                    return Err(a.error.unwrap_or_default());
                }
                return Ok(grade_result(
                    "failed",
                    a.error.unwrap_or_default(),
                    variant,
                    None,
                    None,
                ));
            }
            if a.results.iter().any(|r| r.table.truncated) {
                return Err("答案输出超过判题限制".into());
            }
            if q.kind == "query" {
                if a.results.len() != 1 || !user.is_autocommit() {
                    return Ok(grade_result(
                        "failed",
                        "查询题请提交一条只读 SELECT",
                        variant,
                        None,
                        None,
                    ));
                }
                let statement = user.prepare(answer).map_err(failure)?;
                if !statement.readonly() {
                    return Ok(grade_result(
                        "failed",
                        "查询题不允许修改数据",
                        variant,
                        None,
                        None,
                    ));
                }
                let actual = &a.results[0].table;
                let expected = &e.results.last().unwrap().table;
                if !tables_equal(actual, expected, q.ordered, q.check_names) {
                    return Ok(grade_result(
                        "failed",
                        format!(
                            "数据组 {}：期望 {} 行，得到 {} 行；请检查列、重复次数及 NULL",
                            variant + 1,
                            expected.rows.len(),
                            actual.rows.len()
                        ),
                        variant,
                        (variant == 0).then_some(expected.clone()),
                        (variant == 0).then_some(actual.clone()),
                    ));
                }
            } else {
                if !user.is_autocommit() {
                    return Ok(grade_result(
                        "failed",
                        "答案留下未结束事务，请提交或回滚",
                        variant,
                        None,
                        None,
                    ));
                }
                let actual = dump(&user, b)?;
                let expected = dump(&reference, b)?;
                if !same_dump(&actual, &expected) {
                    return Ok(grade_result(
                        "failed",
                        format!("数据组 {} 的最终数据或结构不符", variant + 1),
                        variant,
                        None,
                        None,
                    ));
                }
                for probe in &q.probes {
                    let u = database::snapshot(&user)?;
                    let r = database::snapshot(&reference)?;
                    let a = database::execute(&u, probe, b);
                    let e = database::execute(&r, probe, b);
                    if (a.status == "completed") != (e.status == "completed")
                        || a.status == "completed" && !same_dump(&dump(&u, b)?, &dump(&r, b)?)
                    {
                        return Ok(grade_result(
                            "failed",
                            format!("行为探针未通过：{probe}"),
                            variant,
                            None,
                            None,
                        ));
                    }
                }
            }
        }
        Ok(grade_result(
            "passed",
            "通过 3 组隔离测试数据及行为探针",
            3,
            None,
            None,
        ))
    })();
    run.unwrap_or_else(|e| {
        grade_result(
            if b.check().is_err() || e.contains("超") || e.contains("限制") {
                "incomplete"
            } else {
                "failed"
            },
            e,
            0,
            None,
            None,
        )
    })
}
type Dump = BTreeMap<String, (Vec<(String, String, bool, bool)>, Table, Vec<String>)>;
fn dump(c: &rusqlite::Connection, b: &Budget) -> Result<Dump, String> {
    let s = database::schema(c)?;
    let mut d = BTreeMap::new();
    for r in s.relations {
        let table = database::query(
            c,
            &format!("SELECT * FROM {}", quote_identifier(&r.name)),
            b,
        )?;
        let cols = r
            .columns
            .iter()
            .map(|c| {
                (
                    c.name.clone(),
                    affinity(&c.data_type),
                    c.nullable,
                    c.primary_key,
                )
            })
            .collect();
        let mut structure = vec![
            format!("kind:{}", r.kind),
            serde_json::to_string(&r.foreign_keys).map_err(failure)?,
            serde_json::to_string(&r.columns.iter().map(|c| &c.default).collect::<Vec<_>>())
                .map_err(failure)?,
        ];
        for index in r
            .indexes
            .iter()
            .filter(|s| !s.starts_with("sqlite_autoindex"))
        {
            let fields = database::query(
                c,
                &format!("PRAGMA index_xinfo({})", quote_identifier(index)),
                b,
            )?;
            structure.push(format!(
                "{index}:{}",
                serde_json::to_string(&fields).map_err(failure)?
            ));
        }
        structure.sort();
        d.insert(r.name, (cols, table, structure));
    }
    Ok(d)
}
fn affinity(s: &str) -> String {
    let s = s.to_uppercase();
    if s.contains("INT") {
        "INTEGER"
    } else if s.contains("CHAR") || s.contains("TEXT") || s.contains("CLOB") {
        "TEXT"
    } else if s.contains("REAL") || s.contains("FLOAT") || s.contains("DOUBLE") {
        "REAL"
    } else if s.is_empty() || s.contains("BLOB") {
        "BLOB"
    } else {
        "NUMERIC"
    }
    .into()
}
fn same_dump(a: &Dump, b: &Dump) -> bool {
    a.len() == b.len()
        && a.iter().all(|(k, (cols, t, idx))| {
            b.get(k)
                .is_some_and(|(c, u, j)| cols == c && idx == j && tables_equal(t, u, false, true))
        })
}
pub fn validate_grading_sql(sql: &str) -> Result<(), String> {
    let tokens = tokenize(sql);
    for t in &tokens {
        if !t.quoted
            && matches!(
                t.text.to_uppercase().as_str(),
                "DATABASE"
                    | "SCHEMA"
                    | "GRANT"
                    | "REVOKE"
                    | "USER"
                    | "OUTFILE"
                    | "DUMPFILE"
                    | "LOAD_FILE"
                    | "LOAD"
                    | "GLOBAL"
                    | "USE"
                    | "DEFINER"
                    | "PREPARE"
                    | "EXECUTE"
                    | "SHUTDOWN"
                    | "INSTALL"
                    | "UNINSTALL"
                    | "SONAME"
            )
        {
            return Err("判题沙箱不允许跨库、动态 SQL、文件或服务器管理操作".into());
        }
    }
    Ok(())
}
pub async fn grade_mysql(
    cfg: &database::mysql::MysqlConfig,
    id: &str,
    answer: &str,
    seed: u64,
    b: &Budget,
) -> Grade {
    use mysql_async::prelude::Queryable;
    let Some(q) = catalog(seed).into_iter().find(|q| q.id == id) else {
        return grade_result("incomplete", "题目不存在", 0, None, None);
    };
    if let Err(e) = validate_grading_sql(answer) {
        return grade_result("failed", e, 0, None, None);
    }
    if q.kind != "routine" {
        return grade_result(
            "incomplete",
            "本题使用内置 SQLite 判题；MySQL 例程题使用独立服务器沙箱",
            0,
            None,
            None,
        );
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let names = [
        format!("sqlkiller_grade_{stamp}_user"),
        format!("sqlkiller_grade_{stamp}_ref"),
    ];
    let account = format!("sqlk_{stamp}");
    let password = format!("SqlK!{:032x}", rand::random::<u128>());
    let mut setup = cfg.clone();
    setup.database = String::new();
    let run = async {
        let mut admin = database::mysql::connect(&setup).await?;
        for name in &names {
            database::mysql::create_sandbox(&mut admin, name, "student").await?;
        }
        admin
            .query_drop(format!(
                "CREATE USER '{account}'@'%' IDENTIFIED BY '{password}'"
            ))
            .await
            .map_err(|_| "MySQL 判题需要创建临时低权限用户的权限；未执行答案".to_string())?;
        admin
            .query_drop(format!(
                "GRANT ALL ON {}.* TO '{account}'@'%'",
                mysql_identifier(&names[0])
            ))
            .await
            .map_err(failure)?;
        let mut uc = cfg.clone();
        uc.database = names[0].clone();
        uc.user = account.clone();
        uc.password = password.clone();
        let mut rc = cfg.clone();
        rc.database = names[1].clone();
        let mut u = database::mysql::connect(&uc).await?;
        let mut r = database::mysql::connect(&rc).await?;
        // Installing a function may require server-level privileges when binlogging is
        // enabled. Only CREATE routine DDL is installed by the administrator; its
        // explicit definer is the sandbox-only account, so calls cannot escape it.
        let statements = crate::script::split_script(answer);
        let mut installation = String::new();
        for statement in statements {
            let tokens = tokenize(&statement.sql);
            if tokens.len() < 4
                || tokens[3].text != "("
                || !tokens[0].text.eq_ignore_ascii_case("CREATE")
                || !matches!(
                    tokens[1].text.to_uppercase().as_str(),
                    "FUNCTION" | "PROCEDURE"
                )
            {
                return Ok(grade_result(
                    "failed",
                    "例程题请仅提交当前沙箱内 CREATE PROCEDURE／FUNCTION 定义",
                    0,
                    None,
                    None,
                ));
            }
            installation.push_str(&format!(
                "CREATE DEFINER='{account}'@'%' {};\n",
                &statement.sql[tokens[1].start..]
            ));
        }
        admin
            .query_drop(format!("USE {}", mysql_identifier(&names[0])))
            .await
            .map_err(failure)?;
        let a = database::mysql::execute(&mut admin, &installation, b).await;
        let e = database::mysql::execute(&mut r, &q.solution, b).await;
        if e.status != "completed" {
            return Err(format!("参考例程执行失败：{:?}", e.error));
        }
        if a.status != "completed" {
            return Ok(grade_result(
                "failed",
                a.error.unwrap_or_default(),
                0,
                None,
                None,
            ));
        }
        for variant in 0..3 {
            let seed_sql=match variant {1=>"INSERT INTO Student VALUES('x1','张三','男',22,'IS'),('x2','张三','女',16,'IS'); INSERT INTO SC VALUES('x1','1',NULL),('x2','1',88);",2=>"DELETE FROM SC; DELETE FROM Student;",_=>""};
            if !seed_sql.is_empty() {
                for conn in [&mut u, &mut r] {
                    let result = database::mysql::execute(conn, seed_sql, b).await;
                    if result.status != "completed" {
                        return Err("MySQL 边界数据初始化失败".into());
                    }
                }
            }
            for (i, probe) in q.probes.iter().enumerate() {
                let a = database::mysql::execute(&mut u, probe, b).await;
                let e = database::mysql::execute(&mut r, probe, b).await;
                if a.status != "completed" || a.results.len() != e.results.len() {
                    return Ok(grade_result(
                        "failed",
                        format!("例程调用失败：{probe} {:?}", a.error),
                        i,
                        None,
                        None,
                    ));
                }
                for (a, e) in a.results.iter().zip(&e.results) {
                    if !tables_equal(&a.table, &e.table, q.ordered, q.check_names) {
                        return Ok(grade_result(
                            "failed",
                            format!("例程测试未通过：{probe}"),
                            i,
                            (variant == 0).then_some(e.table.clone()),
                            (variant == 0).then_some(a.table.clone()),
                        ));
                    }
                }
                let schema = database::mysql::schema(&mut r).await?;
                if database::mysql::schema(&mut u).await?.relations.len() != schema.relations.len()
                {
                    return Ok(grade_result(
                        "failed",
                        "例程改变了受保护的数据库结构",
                        i,
                        None,
                        None,
                    ));
                }
                for relation in schema.relations {
                    let sql = format!("SELECT * FROM {}", mysql_identifier(&relation.name));
                    let actual = database::mysql::query(&mut u, &sql, b).await?;
                    let expected = database::mysql::query(&mut r, &sql, b).await?;
                    if !tables_equal(&actual, &expected, false, true) {
                        return Ok(grade_result(
                            "failed",
                            format!("例程副作用不符合要求：{}", relation.name),
                            i,
                            (variant == 0).then_some(expected),
                            (variant == 0).then_some(actual),
                        ));
                    }
                }
            }
        }
        u.disconnect().await.map_err(failure)?;
        r.disconnect().await.map_err(failure)?;
        admin.disconnect().await.map_err(failure)?;
        Ok::<Grade, String>(grade_result(
            "passed",
            "通过 MySQL 三组独立数据中的例程与副作用测试",
            q.probes.len() * 3,
            None,
            None,
        ))
    };
    let result = match tokio::time::timeout(std::time::Duration::from_secs(30), run).await {
        Ok(Ok(g)) => g,
        Ok(Err(e)) => grade_result("incomplete", e, 0, None, None),
        Err(_) => grade_result("incomplete", "MySQL 判题超时", 0, None, None),
    };
    if let Ok(mut c) = database::mysql::connect(&setup).await {
        let _ = c
            .query_drop(format!("DROP USER IF EXISTS '{account}'@'%'"))
            .await;
        for name in names {
            let _ = c
                .query_drop(format!(
                    "DROP DATABASE IF EXISTS {}",
                    mysql_identifier(&name)
                ))
                .await;
        }
        let _ = c.disconnect().await;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bag_null_precision() {
        let t = |rows| Table {
            columns: vec!["x".into()],
            rows,
            truncated: false,
        };
        assert!(!tables_equal(
            &t(vec![vec![Cell::Null]]),
            &t(vec![vec![Cell::Text("".into())]]),
            false,
            false
        ));
        assert!(!tables_equal(
            &t(vec![vec![Cell::Integer("9007199254740993".into())]]),
            &t(vec![vec![Cell::Integer("9007199254740992".into())]]),
            false,
            false
        ));
    }
    #[test]
    fn references_pass() {
        for q in catalog(0).into_iter().filter(|q| q.engine == "sqlite") {
            let g = grade(&q.id, &q.solution, 0, &Budget::default());
            assert_eq!(g.status, "passed", "{}: {}", q.id, g.message);
        }
    }
    #[test]
    fn wrong_and_equivalent() {
        assert_eq!(
            grade(
                "where",
                "SELECT Sno,Sname,Sage FROM Student WHERE NOT Sage<17",
                0,
                &Budget::default()
            )
            .status,
            "passed"
        );
        assert_eq!(
            grade(
                "null",
                "SELECT Sno,Cno FROM SC WHERE Grade=NULL",
                0,
                &Budget::default()
            )
            .status,
            "failed"
        );
        assert_eq!(
            grade(
                "update",
                "UPDATE Student SET Sage=Sage+1",
                0,
                &Budget::default()
            )
            .status,
            "failed"
        );
    }
}
