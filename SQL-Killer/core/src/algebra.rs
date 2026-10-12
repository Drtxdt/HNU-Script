use crate::{database, exercises::tables_equal, model::*, script::tokenize};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct AlgebraNode {
    pub op: String,
    pub parameter: String,
    pub children: Vec<AlgebraNode>,
}
impl AlgebraNode {
    pub fn text(&self) -> String {
        if self.op == "table" {
            self.parameter.clone()
        } else {
            format!(
                "{}{}({})",
                self.op,
                if self.parameter.is_empty() {
                    String::new()
                } else {
                    format!("{{{}}}", self.parameter)
                },
                self.children
                    .iter()
                    .map(Self::text)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct AlgebraResult {
    pub expression: String,
    pub optimized: String,
    pub tree: AlgebraNode,
    pub trace: Trace,
    pub optimized_trace: Option<Trace>,
    pub rules: Vec<String>,
    pub equivalent: bool,
}
pub fn parse(source: &str) -> Result<AlgebraNode, String> {
    parse_inner(source.trim(), 0)
}
fn parse_inner(s: &str, depth: usize) -> Result<AlgebraNode, String> {
    if depth > 32 {
        return Err("代数表达式嵌套超过 32 层".into());
    }
    let end = s
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(s.len());
    if end == 0 {
        return Err("请输入关系名或 select{条件}(关系) 等表达式".into());
    }
    let name = &s[..end];
    let mut rest = s[end..].trim();
    if rest.is_empty() {
        return Ok(AlgebraNode {
            op: "table".into(),
            parameter: name.into(),
            children: vec![],
        });
    }
    let mut parameter = String::new();
    if rest.starts_with('{') {
        let mut quoted = false;
        let mut stop = None;
        for (i, c) in rest.char_indices().skip(1) {
            if c == '\'' {
                quoted = !quoted
            }
            if c == '}' && !quoted {
                stop = Some(i);
                break;
            }
        }
        let i = stop.ok_or("缺少参数结束符 }")?;
        parameter = rest[1..i].trim().to_string();
        rest = rest[i + 1..].trim();
    }
    if !rest.starts_with('(') || !rest.ends_with(')') {
        return Err("代数运算需要用括号包围输入关系".into());
    }
    let body = &rest[1..rest.len() - 1];
    let mut args = Vec::new();
    let (mut start, mut nest, mut brace, mut quoted) = (0, 0, 0, false);
    for (i, c) in body.char_indices() {
        if c == '\'' {
            quoted = !quoted
        }
        if quoted {
            continue;
        }
        match c {
            '(' => nest += 1,
            ')' => nest -= 1,
            '{' => brace += 1,
            '}' => brace -= 1,
            ',' if nest == 0 && brace == 0 => {
                args.push(&body[start..i]);
                start = i + 1
            }
            _ => {}
        }
    }
    args.push(&body[start..]);
    let op = name.to_lowercase();
    let arity = match op.as_str() {
        "select" | "project" | "rename" | "group" => 1,
        "union" | "intersect" | "diff" | "product" | "join" | "leftjoin" | "rightjoin"
        | "fulljoin" | "divide" => 2,
        _ => return Err(format!("未知代数运算 {name}")),
    };
    if args.len() != arity {
        return Err(format!("{name} 需要 {arity} 个关系"));
    }
    Ok(AlgebraNode {
        op,
        parameter,
        children: args
            .into_iter()
            .map(|s| parse_inner(s.trim(), depth + 1))
            .collect::<Result<_, _>>()?,
    })
}
struct RelationSql {
    sql: String,
    alias: String,
    columns: Vec<String>,
}
fn factor(r: &RelationSql, default: &str) -> String {
    format!(
        "({}) AS {}",
        r.sql,
        quote_identifier(if r.alias.is_empty() {
            default
        } else {
            &r.alias
        })
    )
}
fn build(
    c: &rusqlite::Connection,
    node: &AlgebraNode,
    b: &Budget,
    steps: &mut Vec<TraceStep>,
) -> Result<RelationSql, String> {
    b.check()?;
    let mut children = Vec::new();
    let mut parents = vec![];
    for n in &node.children {
        children.push(build(c, n, b, steps)?);
        parents.push(steps.len() - 1);
    }
    let p = &node.parameter;
    let sql = match node.op.as_str() {
        "table" => format!("SELECT DISTINCT * FROM {}", quote_identifier(p)),
        "select" => format!(
            "SELECT DISTINCT * FROM {} WHERE {p}",
            factor(&children[0], "R")
        ),
        "project" => format!("SELECT DISTINCT {p} FROM {}", factor(&children[0], "R")),
        "rename" => {
            if let Some((_, mapping)) = p.split_once(';') {
                let mut names = std::collections::BTreeMap::new();
                for part in mapping.split(',') {
                    let (old, new) = part
                        .split_once(':')
                        .ok_or("属性重命名格式：rename{R;旧名:新名}(关系)")?;
                    if !children[0].columns.contains(&old.trim().to_string()) {
                        return Err(format!("没有属性 {}", old.trim()));
                    }
                    names.insert(old.trim(), new.trim());
                }
                let cols = children[0]
                    .columns
                    .iter()
                    .map(|c| {
                        format!(
                            "{} AS {}",
                            quote_identifier(c),
                            quote_identifier(names.get(c.as_str()).copied().unwrap_or(c))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("SELECT {cols} FROM {}", factor(&children[0], "R"))
            } else {
                children[0].sql.clone()
            }
        }
        "group" => {
            let (keys, aggs) = p
                .split_once(';')
                .ok_or("group 参数格式为 键;聚合，例如 group{Sdept;COUNT(*) AS n}(Student)")?;
            let projection = if keys.trim().is_empty() {
                aggs.to_string()
            } else {
                format!("{keys},{aggs}")
            };
            format!(
                "SELECT {projection} FROM {}{}",
                factor(&children[0], "R"),
                if keys.trim().is_empty() {
                    String::new()
                } else {
                    format!(" GROUP BY {keys}")
                }
            )
        }
        "union" | "intersect" | "diff" => {
            if children[0].columns.len() != children[1].columns.len() {
                return Err("集合运算两侧列数必须相同，按列位置对应".into());
            }
            let types = |table: &Table, index: usize| {
                table
                    .rows
                    .iter()
                    .filter_map(|row| match &row[index] {
                        Cell::Null => None,
                        Cell::Integer(_) | Cell::Unsigned(_) | Cell::Decimal(_) | Cell::Real(_) => {
                            Some("number")
                        }
                        Cell::Text(_) => Some("text"),
                        Cell::Binary(_) => Some("binary"),
                        Cell::Date(_) => Some("date"),
                        Cell::Time(_) => Some("time"),
                    })
                    .collect::<std::collections::BTreeSet<_>>()
            };
            for index in 0..children[0].columns.len() {
                let a = types(&steps[parents[0]].table, index);
                let b = types(&steps[parents[1]].table, index);
                if !a.is_empty() && !b.is_empty() && a != b {
                    return Err(format!(
                        "第 {} 列的属性域不兼容：{:?} / {:?}",
                        index + 1,
                        a,
                        b
                    ));
                }
            }
            format!(
                "SELECT * FROM {} {} SELECT * FROM {}",
                factor(&children[0], "L"),
                match node.op.as_str() {
                    "union" => "UNION",
                    "intersect" => "INTERSECT",
                    _ => "EXCEPT",
                },
                factor(&children[1], "R")
            )
        }
        "product" => format!(
            "SELECT DISTINCT * FROM {} CROSS JOIN {}",
            factor(&children[0], "L"),
            factor(&children[1], "R")
        ),
        "join" | "leftjoin" | "rightjoin" | "fulljoin" => {
            let mode = match node.op.as_str() {
                "leftjoin" => "LEFT",
                "rightjoin" => "RIGHT",
                "fulljoin" => "FULL",
                _ => "INNER",
            };
            format!(
                "SELECT DISTINCT * FROM {} {}{mode} JOIN {}{}",
                factor(&children[0], "L"),
                if p.is_empty() { "NATURAL " } else { "" },
                factor(&children[1], "R"),
                if p.is_empty() {
                    String::new()
                } else {
                    format!(" ON {p}")
                }
            )
        }
        "divide" => {
            let left = &children[0];
            let right = &children[1];
            if right.columns.iter().any(|col| !left.columns.contains(col)) {
                return Err("除数的所有属性必须属于被除数".into());
            }
            let keys = left
                .columns
                .iter()
                .filter(|col| !right.columns.contains(col))
                .collect::<Vec<_>>();
            if keys.is_empty() {
                return Err("除法结果需要至少一个保留属性".into());
            }
            let projection = keys
                .iter()
                .map(|s| format!("L.{}", quote_identifier(s)))
                .collect::<Vec<_>>()
                .join(", ");
            let tests = keys
                .iter()
                .map(|s| format!("D.{0} IS L.{0}", quote_identifier(s)))
                .chain(
                    right
                        .columns
                        .iter()
                        .map(|s| format!("D.{0} IS R.{0}", quote_identifier(s))),
                )
                .collect::<Vec<_>>()
                .join(" AND ");
            format!("SELECT DISTINCT {projection} FROM ({}) L WHERE NOT EXISTS(SELECT 1 FROM ({}) R WHERE NOT EXISTS(SELECT 1 FROM ({}) D WHERE {tests}))",left.sql,right.sql,left.sql)
        }
        _ => return Err("未知代数节点".into()),
    };
    let table = database::query(c, &sql, b)?;
    if table.truncated {
        return Err("代数中间结果超过教学上限".into());
    }
    let columns = table.columns.clone();
    if steps.len() >= b.max_steps {
        return Err("代数步骤超限".into());
    }
    let id = steps.len();
    let mut details = Vec::new();
    if node.op == "select" {
        details.push(database::query(c, &format!("SELECT *, CASE WHEN ({p}) THEN 1 WHEN NOT ({p}) THEN 0 ELSE NULL END AS __sqlk_truth FROM {}", factor(&children[0], "R")), b)?);
    } else if node.op == "divide" {
        let left = &children[0];
        let right = &children[1];
        let keys = left
            .columns
            .iter()
            .filter(|k| !right.columns.contains(k))
            .collect::<Vec<_>>();
        let projection = keys
            .iter()
            .map(|k| quote_identifier(k))
            .collect::<Vec<_>>()
            .join(", ");
        let checks = keys
            .iter()
            .map(|k| format!("D.{0} IS X.{0}", quote_identifier(k)))
            .chain(
                right
                    .columns
                    .iter()
                    .map(|k| format!("D.{0} IS R.{0}", quote_identifier(k))),
            )
            .collect::<Vec<_>>()
            .join(" AND ");
        details.push(database::query(c, &format!("SELECT X.*, R.*, CASE WHEN NOT EXISTS(SELECT 1 FROM ({r})) OR EXISTS(SELECT 1 FROM ({l}) D WHERE {checks}) THEN 1 ELSE 0 END AS __sqlk_division FROM (SELECT DISTINCT {projection} FROM ({l})) X LEFT JOIN ({r}) R ON 1=1", l=left.sql,r=right.sql), b)?);
    }
    if details.iter().any(|t| t.truncated) {
        return Err("代数动画明细超过教学上限".into());
    }
    steps.push(TraceStep {
        id,
        kind: node.op.clone(),
        title: node.text(),
        explanation: if node.op == "divide" {
            "对每个候选保留元组，检查其是否关联除数的全部元组；除数为空时保留全部候选。"
        } else {
            "独立关系代数采用集合语义：重复元组去重。谓词保留 TRUE，过滤 FALSE 和 UNKNOWN。"
        }
        .into(),
        sql: sql.clone(),
        table,
        parents,
        details,
        events: vec![],
        source_range: None,
    });
    Ok(RelationSql {
        sql,
        columns,
        alias: if node.op == "rename" {
            p.split(';').next().unwrap_or(p).trim().into()
        } else {
            String::new()
        },
    })
}
fn evaluate_tree(
    c: &rusqlite::Connection,
    tree: &AlgebraNode,
    b: &Budget,
) -> Result<Trace, String> {
    let mut steps = vec![];
    build(c, tree, &b.teaching(), &mut steps)?;
    let result = steps.last().map(|s| s.table.clone()).unwrap_or_default();
    let mut trace = Trace {
        source: tree.text(),
        steps,
        result,
        status: "completed".into(),
        message: "集合语义；NULL 按 SQL 扩展规则处理".into(),
        snapshot: "SQLite 一致性副本".into(),
        verified: true,
    };
    crate::animation::enrich(&mut trace, &tree.text(), b)?;
    Ok(trace)
}
pub fn evaluate_algebra(
    c: &rusqlite::Connection,
    source: &str,
    optimize: bool,
    b: &Budget,
) -> Result<AlgebraResult, String> {
    let tree = parse(source)?;
    let snapshot = database::snapshot(c)?;
    let mut trace = evaluate_tree(&snapshot, &tree, b)?;
    for step in &mut trace.steps {
        step.source_range = None;
    }
    crate::animation::enrich(&mut trace, source, b)?;
    let mut optimized = tree.clone();
    let mut rules = vec![];
    if optimize {
        rewrite(&snapshot, &mut optimized, b, &mut rules)?;
    }
    let optimized_trace = if optimize {
        Some(evaluate_tree(&snapshot, &optimized, b)?)
    } else {
        None
    };
    let equivalent = optimized_trace
        .as_ref()
        .is_none_or(|other| tables_equal(&trace.result, &other.result, false, false));
    if !equivalent {
        return Err("优化前后结果不一致，未采用此次改写".into());
    }
    Ok(AlgebraResult {
        expression: tree.text(),
        optimized: optimized.text(),
        tree,
        trace,
        optimized_trace,
        rules,
        equivalent,
    })
}
fn referenced_columns(p: &str) -> Option<Vec<String>> {
    let t = tokenize(p);
    if t.iter().any(|x| x.text == "." || x.text == "(") {
        return None;
    }
    Some(
        t.into_iter()
            .filter(|t| {
                !t.quoted
                    && t.text
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphabetic() || c == '_')
                    && !matches!(
                        t.text.to_uppercase().as_str(),
                        "AND"
                            | "OR"
                            | "NOT"
                            | "NULL"
                            | "IS"
                            | "TRUE"
                            | "FALSE"
                            | "LIKE"
                            | "BETWEEN"
                            | "IN"
                    )
            })
            .map(|t| t.text)
            .collect(),
    )
}
fn rewrite(
    c: &rusqlite::Connection,
    n: &mut AlgebraNode,
    b: &Budget,
    rules: &mut Vec<String>,
) -> Result<(), String> {
    for child in &mut n.children {
        rewrite(c, child, b, rules)?
    }
    if n.op == "select" && n.children[0].op == "join" && n.children[0].parameter.is_empty() {
        if let Some(cols) = referenced_columns(&n.parameter) {
            let mut join = n.children[0].clone();
            let left = evaluate_tree(c, &join.children[0], b)?.result.columns;
            let right = evaluate_tree(c, &join.children[1], b)?.result.columns;
            let side = if !cols.is_empty() && cols.iter().all(|s| left.contains(s)) {
                Some(0)
            } else if !cols.is_empty() && cols.iter().all(|s| right.contains(s)) {
                Some(1)
            } else {
                None
            };
            if let Some(side) = side {
                join.children[side] = AlgebraNode {
                    op: "select".into(),
                    parameter: n.parameter.clone(),
                    children: vec![join.children[side].clone()],
                };
                *n = join;
                rules.push("选择下推：谓词只依赖内连接的一侧属性，提前过滤该侧。".into());
            }
        }
    }
    if n.op == "project" && n.children[0].op == "join" && n.children[0].parameter.is_empty() {
        if let Some(cols) = referenced_columns(&n.parameter) {
            let join = &mut n.children[0];
            let left = evaluate_tree(c, &join.children[0], b)?.result.columns;
            let right = evaluate_tree(c, &join.children[1], b)?.result.columns;
            let common = left
                .iter()
                .filter(|s| right.contains(s))
                .cloned()
                .collect::<Vec<_>>();
            for (side, available) in [(0, &left), (1, &right)] {
                let keep = available
                    .iter()
                    .filter(|s| cols.contains(s) || common.contains(s))
                    .cloned()
                    .collect::<Vec<_>>();
                if !keep.is_empty() && keep.len() < available.len() {
                    join.children[side] = AlgebraNode {
                        op: "project".into(),
                        parameter: keep
                            .iter()
                            .map(|s| quote_identifier(s))
                            .collect::<Vec<_>>()
                            .join(","),
                        children: vec![join.children[side].clone()],
                    };
                    rules.push("投影下推：保留上层输出属性及全部自然连接键，减少中间列。".into());
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sets_and_rewrite() {
        let c = database::memory("student").unwrap();
        let r = evaluate_algebra(
            &c,
            "project{Sno}(select{Grade > 90}(join(SC,Student)))",
            true,
            &Budget::default(),
        )
        .unwrap();
        assert!(r.equivalent);
        assert!(!r.rules.is_empty());
        assert_eq!(r.trace.result.rows.len(), 3);
    }
    #[test]
    fn empty_divisor() {
        let c = database::memory("empty").unwrap();
        c.execute_batch(
            "CREATE TABLE R(x,y);CREATE TABLE S(y);INSERT INTO R VALUES(1,2),(1,3),(2,2);",
        )
        .unwrap();
        let a = evaluate_algebra(&c, "divide(R,S)", false, &Budget::default()).unwrap();
        assert_eq!(a.trace.result.rows.len(), 2);
        c.execute("INSERT INTO S VALUES(3)", []).unwrap();
        assert_eq!(
            evaluate_algebra(&c, "divide(R,S)", false, &Budget::default())
                .unwrap()
                .trace
                .result
                .rows
                .len(),
            1
        );
    }
}
