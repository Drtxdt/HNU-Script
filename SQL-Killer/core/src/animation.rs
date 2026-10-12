//! Immutable teaching events refer to frozen relation occurrences, never physical row IDs.
use crate::{model::*, script::tokenize};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct RowRef {
    pub table: String,
    pub index: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct AnimationEvent {
    pub id: String,
    pub step: usize,
    pub kind: String,
    pub message: String,
    pub sources: Vec<RowRef>,
    pub targets: Vec<RowRef>,
    pub certainty: String,
    pub truth: Option<String>,
    pub columns: Vec<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct StateChange {
    pub target: String,
    pub before: String,
    pub after: String,
}
fn key(row: &[Cell]) -> String {
    serde_json::to_string(row).expect("cells serialize")
}
fn reference(table: &str, index: usize) -> RowRef {
    RowRef {
        table: table.into(),
        index,
    }
}
pub fn source_span(source: &str, kind: &str, title: &str) -> Option<SourceRange> {
    let tokens = tokenize(source);
    // Algebra expressions: match tokens, not substrings (whitespace and Unicode are preserved).
    let expression = tokenize(title.strip_prefix("读取 ").unwrap_or(title));
    if !expression.is_empty()
        && (kind == "source"
            || source
                .trim_start()
                .chars()
                .next()
                .is_some_and(char::is_lowercase))
    {
        if let Some(w) = tokens.windows(expression.len()).find(|w| {
            w.iter()
                .zip(&expression)
                .all(|(a, b)| a.text.eq_ignore_ascii_case(&b.text))
        }) {
            return Some(source_range(source, w[0].start, w.last()?.end));
        }
    }
    let word = match kind {
        "source" => "FROM",
        "cte" => "WITH",
        "filter" => "WHERE",
        "join" => "JOIN",
        "group" => "GROUP",
        "having" => "HAVING",
        "sort" => "ORDER",
        "limit" => "LIMIT",
        "distinct" => "DISTINCT",
        "set" => title.split_whitespace().next()?,
        "subquery" => return None,
        _ => "SELECT",
    };
    let (i, t) = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.text.eq_ignore_ascii_case(word))
        .min_by_key(|(_, t)| t.depth)?;
    let end = tokens
        .iter()
        .skip(i + 1)
        .find(|x| {
            x.depth < t.depth
                || (x.depth == t.depth
                    && [
                        "FROM",
                        "WHERE",
                        "JOIN",
                        "LEFT",
                        "RIGHT",
                        "FULL",
                        "GROUP",
                        "HAVING",
                        "ORDER",
                        "LIMIT",
                        "UNION",
                        "EXCEPT",
                        "INTERSECT",
                    ]
                    .contains(&x.text.to_uppercase().as_str()))
        })
        .map(|x| x.start)
        .unwrap_or(source.len());
    Some(source_range(source, t.start, end))
}
pub fn enrich(trace: &mut Trace, source: &str, budget: &Budget) -> Result<(), String> {
    trace.source = source.into();
    let mut count = 0;
    for at in 0..trace.steps.len() {
        budget.check()?;
        let s = &trace.steps[at];
        let output = format!("s{}", s.id);
        let inputs: Vec<(String, &Table)> = if !s.details.is_empty() {
            s.details
                .iter()
                .enumerate()
                .map(|(i, t)| (format!("s{}:d{i}", s.id), t))
                .collect()
        } else {
            s.parents
                .iter()
                .filter_map(|id| {
                    trace
                        .steps
                        .iter()
                        .find(|p| p.id == *id)
                        .map(|p| (format!("s{}", p.id), &p.table))
                })
                .collect()
        };
        let mut events = Vec::new();
        let base = |n: usize, kind: &str, message: String| AnimationEvent {
            id: format!("{output}:e{n}"),
            step: s.id,
            kind: kind.into(),
            message,
            sources: vec![],
            targets: vec![],
            certainty: "exact".into(),
            truth: None,
            columns: vec![],
        };
        // Division obligations are checked by the native engine, including NULL-safe
        // attribute equality and the vacuous truth of an empty divisor.
        if let Some((name, input)) = inputs
            .first()
            .filter(|(_, t)| t.columns.last().is_some_and(|n| n == "__sqlk_division"))
        {
            let width = s.table.columns.len();
            for (i, row) in input.rows.iter().enumerate() {
                budget.check()?;
                let satisfied = row.last().is_some_and(|c| c.display() == "1");
                let mut e = base(
                    i,
                    if satisfied { "keep" } else { "drop" },
                    format!(
                        "候选 ({}) · 除数逐项检查：{}",
                        row[..width]
                            .iter()
                            .map(Cell::display)
                            .collect::<Vec<_>>()
                            .join(", "),
                        if satisfied {
                            "满足本项（空除数为真）"
                        } else {
                            "缺少对应组合，候选淘汰"
                        }
                    ),
                );
                e.truth = Some(if satisfied { "TRUE" } else { "FALSE" }.into());
                e.sources.push(reference(name, i));
                if let Some(target) = s
                    .table
                    .rows
                    .iter()
                    .position(|r| r.as_slice() == &row[..width])
                {
                    e.targets.push(reference(&output, target));
                }
                events.push(e);
            }
        // Native-engine predicate probes include TRUE/FALSE/UNKNOWN for every input occurrence.
        } else if let Some((name, input)) = inputs
            .first()
            .filter(|(_, t)| t.columns.last().is_some_and(|n| n == "__sqlk_truth"))
        {
            let mut outputs = BTreeMap::<String, VecDeque<usize>>::new();
            for (i, r) in s.table.rows.iter().enumerate() {
                outputs.entry(key(r)).or_default().push_back(i);
            }
            for (i, row) in input.rows.iter().enumerate() {
                budget.check()?;
                let value = row.last().map(Cell::display).unwrap_or_default();
                let truth = match value.as_str() {
                    "1" => "TRUE",
                    "0" => "FALSE",
                    _ => "UNKNOWN",
                };
                let mut e = base(
                    i,
                    if truth == "TRUE" { "keep" } else { "drop" },
                    format!(
                        "谓词 {truth}：{}第 {} 条记录",
                        if truth == "TRUE" { "保留" } else { "筛除" },
                        i + 1
                    ),
                );
                e.sources.push(reference(name, i));
                e.truth = Some(truth.into());
                let expression = s
                    .source_range
                    .as_ref()
                    .and_then(|r| source.get(r.start..r.end))
                    .unwrap_or(&s.title);
                let names = tokenize(expression)
                    .into_iter()
                    .map(|t| t.text.to_lowercase())
                    .collect::<Vec<_>>();
                e.columns = input
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        c.as_str() == "__sqlk_truth" || names.contains(&c.to_lowercase())
                    })
                    .map(|(i, _)| i)
                    .collect();
                if truth == "TRUE" {
                    if let Some(n) = outputs
                        .get_mut(&key(&row[..row.len() - 1]))
                        .and_then(VecDeque::pop_front)
                    {
                        e.targets.push(reference(&output, n));
                    } else {
                        e.certainty = "candidate".into();
                        e.message.push_str("；输出存在去重或表达式变化");
                    }
                }
                events.push(e);
            }
        } else if s.kind == "group"
            && inputs
                .first()
                .is_some_and(|(_, t)| t.columns.last().is_some_and(|n| n == "__sqlk_group"))
        {
            let (name, input) = &inputs[0];
            let mut seen = BTreeMap::<String, usize>::new();
            let groups = s
                .table
                .rows
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.last().map(|v| (v.display(), i)))
                .collect::<BTreeMap<_, _>>();
            for (i, row) in input.rows.iter().enumerate() {
                budget.check()?;
                let group = row.last().map(Cell::display).unwrap_or_default();
                if let Some(target) = groups.get(&group) {
                    let n = seen.entry(group.clone()).or_default();
                    *n += 1;
                    let mut e = base(
                        i,
                        "group",
                        format!("分组 {group} · 第 {n} 个成员汇入（分组号由数据库计算）"),
                    );
                    e.sources.push(reference(name, i));
                    e.targets.push(reference(&output, *target));
                    events.push(e);
                }
            }
        } else if s.kind == "aggregate"
            && inputs.len() == 1
            && inputs[0]
                .1
                .columns
                .iter()
                .any(|c| c.starts_with("__sqlk_acc:"))
            && !inputs[0].1.rows.is_empty()
        {
            let (name, input) = &inputs[0];
            for (i, row) in input.rows.iter().enumerate() {
                budget.check()?;
                let values = input
                    .columns
                    .iter()
                    .enumerate()
                    .filter_map(|(j, c)| {
                        c.strip_prefix("__sqlk_acc:")
                            .map(|label| format!("{label} = {}", row[j].display()))
                    })
                    .collect::<Vec<_>>()
                    .join(" · ");
                let mut e = base(
                    i,
                    "aggregate",
                    format!("成员 {} 汇入 · {values}（数据库计算的累计值）", i + 1),
                );
                e.sources.push(reference(name, i));
                if s.table.rows.len() == 1 && !s.sql.to_uppercase().contains("GROUP BY") {
                    e.targets.push(reference(&output, 0));
                }
                events.push(e);
            }
        } else if s.kind == "aggregate"
            && s.table.rows.len() == 1
            && inputs.len() == 1
            && !s.sql.to_uppercase().contains("GROUP BY")
        {
            let (name, input) = &inputs[0];
            for (i, _) in input.rows.iter().enumerate() {
                let mut e = base(
                    i,
                    "aggregate",
                    format!(
                        "累计处理第 {} / {} 个成员 · 最终聚合值 {}",
                        i + 1,
                        input.rows.len(),
                        s.table.rows[0]
                            .iter()
                            .map(Cell::display)
                            .collect::<Vec<_>>()
                            .join(" / ")
                    ),
                );
                e.sources.push(reference(name, i));
                e.targets.push(reference(&output, 0));
                events.push(e);
            }
            if input.rows.is_empty() {
                let mut e = base(
                    0,
                    "aggregate",
                    "空输入：COUNT 为 0，SUM／AVG／MIN／MAX 为 NULL；以冻结结果为准".into(),
                );
                e.targets.push(reference(&output, 0));
                events.push(e);
            }
        } else {
            // Index each frozen input by the retained output columns. A value-only
            // relationship is explicitly candidate, including equal duplicate rows.
            let indexes: Vec<_> = inputs
                .iter()
                .enumerate()
                .map(|(side, (name, input))| {
                    let concat = s.kind == "join"
                        && inputs.len() == 2
                        && inputs.iter().map(|(_, t)| t.columns.len()).sum::<usize>()
                            == s.table.columns.len();
                    let offset = if concat && side == 1 {
                        inputs[0].1.columns.len()
                    } else {
                        0
                    };
                    let pairs: Vec<_> = input
                        .columns
                        .iter()
                        .enumerate()
                        .filter_map(|(i, c)| {
                            if concat {
                                Some((i, i + offset))
                            } else {
                                s.table.columns.iter().position(|x| x == c).map(|o| (i, o))
                            }
                        })
                        .collect();
                    let mut map = BTreeMap::<String, Vec<usize>>::new();
                    if !pairs.is_empty() {
                        for (i, r) in input.rows.iter().enumerate() {
                            let values =
                                pairs.iter().map(|(a, _)| r[*a].clone()).collect::<Vec<_>>();
                            map.entry(key(&values)).or_default().push(i);
                        }
                    }
                    (name.clone(), pairs, map)
                })
                .collect();
            let mut occurrence = BTreeMap::<String, usize>::new();
            for (i, row) in s.table.rows.iter().enumerate() {
                budget.check()?;
                let mut e = base(i, &s.kind, format!("{} · 输出记录 {}", s.title, i + 1));
                e.targets.push(reference(&output, i));
                for (name, pairs, map) in &indexes {
                    let values = pairs
                        .iter()
                        .map(|(_, o)| row[*o].clone())
                        .collect::<Vec<_>>();
                    if let Some(matches) = map.get(&key(&values)) {
                        e.sources
                            .extend(matches.iter().take(24).map(|n| reference(name, *n)));
                    }
                }
                if !inputs.is_empty() {
                    e.certainty = "candidate".into();
                }
                // A full concatenated JOIN tuple uniquely identifies its frozen input
                // occurrences only when both sides are unique. NULL-only matches are
                // ambiguous with outer-join padding and deliberately remain candidates.
                if s.kind == "join"
                    && inputs.len() == 2
                    && inputs.iter().map(|(_, t)| t.columns.len()).sum::<usize>() == row.len()
                {
                    let mut offset = 0;
                    let mut verified = true;
                    let mut padded = false;
                    for (_, input) in &inputs {
                        let part = &row[offset..offset + input.columns.len()];
                        offset += input.columns.len();
                        let matches = input.rows.iter().filter(|r| r.as_slice() == part).count();
                        let nulls = part.iter().all(|v| matches!(v, Cell::Null));
                        if matches == 0 && nulls {
                            padded = true;
                        } else if matches != 1 || nulls {
                            verified = false;
                        }
                    }
                    if verified {
                        e.certainty = "exact".into();
                        e.message.push_str(if padded {
                            " · 未匹配侧补 NULL"
                        } else {
                            " · 两侧唯一冻结记录匹配"
                        });
                    }
                }
                // Whole-row reorder/subset is an occurrence correspondence, not physical lineage.
                if [
                    "sort",
                    "limit",
                    "distinct",
                    "rename",
                    "union",
                    "intersect",
                    "diff",
                    "set",
                ]
                .contains(&s.kind.as_str())
                    && inputs.len() == 1
                    && inputs[0].1.columns.len() == row.len()
                {
                    let k = key(row);
                    let n = occurrence.entry(k.clone()).or_default();
                    if let Some(matches) = indexes[0].2.get(&k) {
                        let which = matches[(*n).min(matches.len() - 1)];
                        e.sources = vec![reference(&inputs[0].0, which)];
                        *n += 1;
                        e.certainty = "occurrence".into();
                    }
                }
                e.columns = s
                    .table
                    .columns
                    .iter()
                    .enumerate()
                    .filter(|(_, name)| inputs.iter().any(|(_, t)| t.columns.contains(name)))
                    .map(|(i, _)| i)
                    .collect();
                if row.iter().any(|v| matches!(v, Cell::Null)) {
                    e.message.push_str(" · 含 NULL（外连接请结合条件核对）");
                }
                events.push(e);
            }
        }
        if events.is_empty() {
            events.push(base(
                0,
                "empty",
                format!("{} · 空关系，无记录流入输出", s.title),
            ));
        }
        count += events.len();
        if count > budget.max_steps {
            return Err("动画事件超过 20,000 条教学限制；未展示截断过程".into());
        }
        let range = source_span(source, &s.kind, &s.title);
        trace.steps[at].events = events;
        if trace.steps[at].source_range.is_none() {
            trace.steps[at].source_range = range;
        }
    }
    Ok(())
}
