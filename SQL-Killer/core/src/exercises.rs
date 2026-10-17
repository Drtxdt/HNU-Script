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
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub hints: Vec<String>,
    #[serde(default)]
    pub knowledge: String,
    #[serde(default)]
    pub answer_ui: String,
    pub solution: String,
    #[serde(default)]
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
    pub diff: Option<TableDiff>,
    pub diagnostic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct TableDiff {
    pub equal: bool,
    pub expected: Vec<usize>,
    pub actual: Vec<usize>,
    pub columns_mismatch: bool,
}
pub fn catalog(seed: u64) -> Vec<Question> {
    let mut questions: Vec<Question> =
        serde_json::from_str(include_str!("../../curriculum/questions.json"))
            .expect("validated bundled question catalogue");
    let age = (17 + seed % 4).to_string();
    for q in &mut questions {
        q.seed = seed;
        if q.id == "where" {
            q.prompt = q.prompt.replace("{{age}}", &age);
            q.solution = q.solution.replace("{{age}}", &age);
        }
    }
    crate::learning::enrich(&mut questions);
    questions
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
pub fn compare_tables(a: &Table, b: &Table, ordered: bool, names: bool) -> TableDiff {
    let columns_mismatch = a.columns.len() != b.columns.len() || names && a.columns != b.columns;
    let mut diff = TableDiff {
        equal: !a.truncated && !b.truncated && !columns_mismatch,
        columns_mismatch,
        ..TableDiff::default()
    };
    let mut left: Vec<_> = a.rows.iter().enumerate().collect();
    let mut right: Vec<_> = b.rows.iter().enumerate().collect();
    if !ordered {
        left.sort_by(|x, y| row_order(x.1, y.1));
        right.sort_by(|x, y| row_order(x.1, y.1));
    }
    for i in 0..left.len().max(right.len()) {
        let same = match (left.get(i), right.get(i)) {
            (Some((_, x)), Some((_, y))) => {
                x.len() == y.len() && x.iter().zip(y.iter()).all(|(x, y)| equal(x, y))
            }
            _ => false,
        };
        if !same || columns_mismatch {
            diff.equal = false;
            if let Some((n, _)) = left.get(i) {
                diff.expected.push(*n)
            }
            if let Some((n, _)) = right.get(i) {
                diff.actual.push(*n)
            }
        }
    }
    diff
}
pub fn tables_equal(a: &Table, b: &Table, ordered: bool, names: bool) -> bool {
    compare_tables(a, b, ordered, names).equal
}

fn grade_result(
    status: &str,
    message: impl ToString,
    cases: usize,
    expected: Option<Table>,
    actual: Option<Table>,
) -> Grade {
    let diff = expected
        .as_ref()
        .zip(actual.as_ref())
        .map(|(a, b)| compare_tables(a, b, false, false));
    Grade {
        diff,
        diagnostic: if status == "passed" {
            "passed"
        } else if status == "incomplete" {
            "incomplete"
        } else if cases > 0 {
            "boundary_data"
        } else {
            "public_data"
        }
        .into(),
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
    let mut result = run.unwrap_or_else(|e| {
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
    });
    result.diff = result
        .expected
        .as_ref()
        .zip(result.actual.as_ref())
        .map(|(a, b)| compare_tables(a, b, q.ordered, q.check_names));
    if result.cases > 0 && result.status == "failed" {
        result.message = "隐藏边界数据未通过；请检查 NULL、重复次数、作用范围与排序约定".into();
    }
    result
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
