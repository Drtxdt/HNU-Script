use crate::model::{failure, Budget};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    time::{Duration, Instant},
};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Operation {
    pub tx: String,
    pub kind: String,
    pub item: String,
    pub expression: String,
    pub low: i64,
    pub high: i64,
    pub line: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Conflict {
    pub from: String,
    pub to: String,
    pub item: String,
    pub first: usize,
    pub second: usize,
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct ScheduleAnalysis {
    pub transactions: Vec<String>,
    pub edges: Vec<Conflict>,
    pub cycle: Vec<String>,
    pub serial_order: Vec<String>,
    pub conflict_serializable: bool,
    pub view_serializable: Option<bool>,
    pub view_order: Vec<String>,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct ScheduleStep {
    #[serde(default)]
    pub source_line: usize,
    #[serde(default)]
    pub changes: Vec<crate::animation::StateChange>,
    pub index: usize,
    pub tx: String,
    pub operation: String,
    pub status: String,
    pub message: String,
    pub data: BTreeMap<String, i64>,
    pub locals: BTreeMap<String, BTreeMap<String, i64>>,
    pub locks: Vec<String>,
    pub waits: Vec<(String, String)>,
    pub versions: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Simulation {
    pub steps: Vec<ScheduleStep>,
    pub status: String,
    pub anomalies: Vec<String>,
    pub recoverable: bool,
    pub cascadeless: bool,
    pub analysis: ScheduleAnalysis,
}

pub fn parse(source: &str) -> Result<Vec<Operation>, String> {
    let mut ops = vec![];
    let mut finished = BTreeSet::new();
    for (i, line) in source.lines().enumerate() {
        let line = line.split("//").next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let normalized = line.replace(['(', ')', ','], " ");
        let parts = normalized.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 2 {
            return Err(format!("第 {} 行需要 事务名 操作 参数", i + 1));
        }
        let tx = parts[0].trim_end_matches(':').to_string();
        if finished.contains(&tx) {
            return Err(format!("第 {} 行：事务 {tx} 已结束", i + 1));
        }
        let kind = match parts[1].to_lowercase().as_str() {
            "r" | "read" => "read",
            "w" | "write" => "write",
            "set" => "set",
            "scan" => "scan",
            "insert" => "insert",
            "c" | "commit" => "commit",
            "a" | "rollback" | "abort" => "rollback",
            _ => return Err(format!("第 {} 行：未知操作", i + 1)),
        }
        .to_string();
        let item = parts.get(2).copied().unwrap_or("").to_string();
        if !matches!(kind.as_str(), "commit" | "rollback") && item.is_empty() {
            return Err(format!("第 {} 行缺少数据项", i + 1));
        }
        let expression = if parts.len() > 3 {
            parts[3..].join(" ")
        } else {
            item.clone()
        };
        let mut op = Operation {
            tx: tx.clone(),
            kind,
            item,
            expression,
            low: i64::MIN,
            high: i64::MAX,
            line: i + 1,
        };
        if op.kind == "scan" {
            if parts.len() != 5 {
                return Err(format!("第 {} 行格式：T1 scan 前缀 下界 上界", i + 1));
            }
            op.low = parts[3].parse().map_err(failure)?;
            op.high = parts[4].parse().map_err(failure)?;
            if op.low > op.high {
                return Err("范围下界不能大于上界".into());
            }
        }
        if matches!(op.kind.as_str(), "commit" | "rollback") {
            finished.insert(tx);
        }
        ops.push(op);
    }
    if ops.len() > 256 {
        return Err("调度最多支持 256 个操作".into());
    }
    Ok(ops)
}
pub fn analyze_schedule(source: &str, view: bool, b: &Budget) -> Result<ScheduleAnalysis, String> {
    let ops = parse(source)?;
    analyze(&ops, view, b)
}
fn analyze(ops: &[Operation], view: bool, b: &Budget) -> Result<ScheduleAnalysis, String> {
    let tx = ops
        .iter()
        .map(|o| o.tx.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rw = |o: &Operation| matches!(o.kind.as_str(), "read" | "write" | "insert");
    let mut edges = vec![];
    for (i, a) in ops.iter().enumerate() {
        b.check()?;
        if !rw(a) {
            continue;
        }
        for c in ops.iter().skip(i + 1) {
            if rw(c) && a.tx != c.tx && a.item == c.item && (a.kind != "read" || c.kind != "read") {
                edges.push(Conflict {
                    from: a.tx.clone(),
                    to: c.tx.clone(),
                    item: a.item.clone(),
                    first: a.line - 1,
                    second: c.line - 1,
                    kind: format!("{} → {}", a.kind, c.kind),
                });
            }
        }
    }
    let pairs = edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect::<BTreeSet<_>>();
    let (order, cycle) = topology(&tx, &pairs);
    let mut result = ScheduleAnalysis {
        transactions: tx.clone(),
        edges,
        conflict_serializable: cycle.is_empty(),
        cycle,
        serial_order: order,
        view_serializable: None,
        view_order: vec![],
        message: "冲突判定仅比较不同事务对同一项的读写次序。".into(),
    };
    if !view {
        return Ok(result);
    }
    if tx.len() > 8
        || ops
            .iter()
            .any(|o| matches!(o.kind.as_str(), "scan" | "rollback"))
    {
        result
            .message
            .push_str(" 视图判定未完成：最多 8 个事务，仅支持不含回滚的点读写调度。");
        return Ok(result);
    }
    let target = signature(ops, &(0..ops.len()).collect::<Vec<_>>());
    let deadline = (Instant::now() + Duration::from_secs(5)).min(b.deadline);
    let mut permutation = tx.clone();
    loop {
        if Instant::now() > deadline || b.check().is_err() {
            result
                .message
                .push_str(" 视图枚举已取消或超时，不能判为不等价。");
            return Ok(result);
        }
        let order = permutation
            .iter()
            .flat_map(|t| {
                ops.iter()
                    .enumerate()
                    .filter(move |(_, o)| &o.tx == t)
                    .map(|(i, _)| i)
            })
            .collect::<Vec<_>>();
        if signature(ops, &order) == target {
            result.view_serializable = Some(true);
            result.view_order = permutation;
            return Ok(result);
        }
        if !next_permutation(&mut permutation) {
            break;
        }
    }
    result.view_serializable = Some(false);
    Ok(result)
}
fn signature(
    ops: &[Operation],
    order: &[usize],
) -> (BTreeMap<usize, Option<usize>>, BTreeMap<String, usize>) {
    let mut writes = BTreeMap::new();
    let mut reads = BTreeMap::new();
    for &i in order {
        let o = &ops[i];
        if o.kind == "read" {
            reads.insert(i, writes.get(&o.item).copied());
        } else if matches!(o.kind.as_str(), "write" | "insert") {
            writes.insert(o.item.clone(), i);
        }
    }
    (reads, writes)
}
fn next_permutation<T: Ord>(a: &mut [T]) -> bool {
    let Some(i) = (1..a.len()).rev().find(|&i| a[i - 1] < a[i]).map(|i| i - 1) else {
        return false;
    };
    let j = (i + 1..a.len()).rev().find(|&j| a[j] > a[i]).unwrap();
    a.swap(i, j);
    a[i + 1..].reverse();
    true
}
fn topology(tx: &[String], pairs: &BTreeSet<(String, String)>) -> (Vec<String>, Vec<String>) {
    let mut order = vec![];
    let mut remaining = tx.iter().cloned().collect::<BTreeSet<_>>();
    while let Some(next) = remaining
        .iter()
        .find(|t| !pairs.iter().any(|(a, b)| b == *t && remaining.contains(a)))
        .cloned()
    {
        remaining.remove(&next);
        order.push(next);
    }
    if remaining.is_empty() {
        return (order, vec![]);
    }
    fn dfs(
        node: &str,
        pairs: &BTreeSet<(String, String)>,
        path: &mut Vec<String>,
        done: &mut BTreeSet<String>,
    ) -> Option<Vec<String>> {
        if let Some(i) = path.iter().position(|p| p == node) {
            let mut c = path[i..].to_vec();
            c.push(node.into());
            return Some(c);
        }
        if done.contains(node) {
            return None;
        }
        path.push(node.into());
        for (a, b) in pairs {
            if a == node {
                if let Some(c) = dfs(b, pairs, path, done) {
                    return Some(c);
                }
            }
        }
        path.pop();
        done.insert(node.into());
        None
    }
    let mut done = BTreeSet::new();
    for t in remaining {
        if let Some(c) = dfs(&t, pairs, &mut vec![], &mut done) {
            return (order, c);
        }
    }
    (order, vec![])
}
#[derive(Clone)]
struct Version {
    tx: String,
    value: i64,
    committed: bool,
}
#[derive(Clone)]
struct Lock {
    tx: String,
    item: String,
    exclusive: bool,
    range: Option<(i64, i64)>,
}
pub fn simulate_schedule(
    source: &str,
    initial: BTreeMap<String, i64>,
    mode: &str,
    isolation: &str,
    b: &Budget,
) -> Result<Simulation, String> {
    if !["raw", "locks"].contains(&mode) || !["ru", "rc", "rr", "serializable"].contains(&isolation)
    {
        return Err("未知模拟模式或隔离级别".into());
    }
    let ops = parse(source)?;
    let predicates = ops
        .iter()
        .filter(|o| o.kind == "scan")
        .map(|o| (o.item.clone(), o.low, o.high))
        .collect::<BTreeSet<_>>();
    let analysis = analyze(&ops, true, b)?;
    let mut versions = initial
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                vec![Version {
                    tx: "初始".into(),
                    value: *v,
                    committed: true,
                }],
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut locals = BTreeMap::<String, BTreeMap<String, i64>>::new();
    let mut locks = Vec::<Lock>::new();
    let mut waiting = VecDeque::<usize>::new();
    let mut steps = vec![];
    let mut anomalies = BTreeSet::new();
    let (mut recoverable, mut cascadeless) = (true, true);
    let mut committed = BTreeSet::from(["初始".to_string()]);
    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    let mut last_read = BTreeMap::<(String, String), (i64, String)>::new();
    let mut last_scan = BTreeMap::<(String, String, i64, i64), Vec<(String, i64)>>::new();
    let mut queue = (0..ops.len()).collect::<VecDeque<_>>();
    let mut waits = BTreeSet::<(String, String)>::new();
    let mut executed = Vec::new();
    while let Some(index) = queue.pop_front() {
        b.check()?;
        if steps.len() >= b.max_steps {
            return Err("事务过程超过步骤限制".into());
        }
        let o = &ops[index];
        let own = locals.entry(o.tx.clone()).or_default().clone();
        let queued_behind = waiting.iter().any(|i| ops[*i].tx == o.tx);
        let value = if !queued_behind && matches!(o.kind.as_str(), "write" | "insert" | "set") {
            Some(expression(&o.expression, &own)?)
        } else {
            None
        };
        let mut blockers = BTreeSet::new();
        if queued_behind {
            blockers.insert(o.tx.clone());
        }
        if mode == "locks" && !matches!(o.kind.as_str(), "commit" | "rollback" | "set") {
            let writing = matches!(o.kind.as_str(), "write" | "insert");
            for l in &locks {
                if l.tx == o.tx {
                    continue;
                }
                let conflict = if let Some((low, high)) = l.range {
                    writing
                        && o.item.starts_with(&l.item)
                        && (value.is_some_and(|v| v >= low && v <= high)
                            || versions
                                .get(&o.item)
                                .and_then(|v| v.last())
                                .is_some_and(|v| v.value >= low && v.value <= high))
                } else if o.kind == "scan" {
                    l.item.starts_with(&o.item) && l.exclusive && isolation != "ru"
                } else {
                    l.item == o.item && (writing || l.exclusive && isolation != "ru")
                };
                if conflict {
                    blockers.insert(l.tx.clone());
                }
            }
        }
        let mut message = String::new();
        let status;
        if !blockers.is_empty() {
            status = "waiting";
            for tx in &blockers {
                if tx != &o.tx {
                    waits.insert((o.tx.clone(), tx.clone()));
                }
            }
            waiting.push_back(index);
            message = format!(
                "等待 {}；事务内操作顺序保持不变",
                blockers.into_iter().collect::<Vec<_>>().join(", ")
            );
        } else {
            waits.retain(|(from, _)| from != &o.tx);
            status = "executed";
            executed.push(o.clone());
            match o.kind.as_str() {
                "read" => {
                    let list = versions
                        .get(&o.item)
                        .ok_or_else(|| format!("第 {} 行：数据项 {} 不存在", o.line, o.item))?;
                    let v = list
                        .iter()
                        .rev()
                        .find(|v| mode == "raw" || isolation == "ru" || v.committed || v.tx == o.tx)
                        .ok_or("没有可见版本")?;
                    locals
                        .entry(o.tx.clone())
                        .or_default()
                        .insert(o.item.clone(), v.value);
                    if !v.committed && v.tx != o.tx {
                        cascadeless = false;
                        dependencies
                            .entry(o.tx.clone())
                            .or_default()
                            .insert(v.tx.clone());
                        anomalies.insert(format!(
                            "脏读：{} 读取 {} 尚未提交的 {}",
                            o.tx, v.tx, o.item
                        ));
                    }
                    if let Some((old, _)) = last_read.get(&(o.tx.clone(), o.item.clone())) {
                        if *old != v.value && v.tx != o.tx {
                            anomalies.insert(format!(
                                "不可重复读：{} 的 {} 从 {old} 变为 {}",
                                o.tx, o.item, v.value
                            ));
                        }
                    }
                    last_read.insert((o.tx.clone(), o.item.clone()), (v.value, v.tx.clone()));
                    if mode == "locks" && matches!(isolation, "rr" | "serializable") {
                        locks.push(Lock {
                            tx: o.tx.clone(),
                            item: o.item.clone(),
                            exclusive: false,
                            range: None,
                        });
                    }
                    message = format!(
                        "读取 {} = {}，来源 {}{}",
                        o.item,
                        v.value,
                        v.tx,
                        if v.committed {
                            "（已提交）"
                        } else {
                            "（未提交）"
                        }
                    );
                }
                "set" => {
                    let value = value.unwrap();
                    locals
                        .entry(o.tx.clone())
                        .or_default()
                        .insert(o.item.clone(), value);
                    message = format!("局部变量 {} = {value}；尚未写回数据库", o.item);
                }
                "write" | "insert" => {
                    let old = versions
                        .get(&o.item)
                        .and_then(|vs| vs.last())
                        .map(|v| v.value);
                    for (prefix, low, high) in &predicates {
                        if o.item.starts_with(prefix)
                            && [old, value]
                                .into_iter()
                                .flatten()
                                .any(|v| v >= *low && v <= *high)
                        {
                            executed.push(Operation {
                                kind: "write".into(),
                                item: format!("范围:{prefix}[{low}..{high}]"),
                                ..o.clone()
                            });
                        }
                    }
                    if o.kind == "insert" && versions.get(&o.item).is_some_and(|v| !v.is_empty()) {
                        return Err(format!("第 {} 行：插入项已存在", o.line));
                    }
                    if o.kind == "write" && !versions.contains_key(&o.item) {
                        return Err(format!("第 {} 行：写入项不存在", o.line));
                    }
                    if let (Some((old, writer)), Some(current)) = (
                        last_read.get(&(o.tx.clone(), o.item.clone())),
                        versions.get(&o.item).and_then(|v| v.last()),
                    ) {
                        if current.tx != o.tx && &current.tx != writer && current.value != *old {
                            anomalies.insert(format!(
                                "丢失修改风险：{} 基于旧值 {old} 写回 {}，覆盖 {} 的值 {}",
                                o.tx, o.item, current.tx, current.value
                            ));
                        }
                    }
                    versions.entry(o.item.clone()).or_default().push(Version {
                        tx: o.tx.clone(),
                        value: value.unwrap(),
                        committed: false,
                    });
                    if mode == "locks" {
                        locks.push(Lock {
                            tx: o.tx.clone(),
                            item: o.item.clone(),
                            exclusive: true,
                            range: None,
                        });
                    }
                    message = format!("{} {} = {}（未提交版本）", o.kind, o.item, value.unwrap());
                }
                "scan" => {
                    executed.push(Operation {
                        kind: "read".into(),
                        item: format!("范围:{}[{}..{}]", o.item, o.low, o.high),
                        ..o.clone()
                    });
                    for (k, vs) in versions.iter().filter(|(k, _)| k.starts_with(&o.item)) {
                        if let Some(v) = vs.iter().rev().find(|v| {
                            mode == "raw" || isolation == "ru" || v.committed || v.tx == o.tx
                        }) {
                            if v.value >= o.low && v.value <= o.high && !v.committed && v.tx != o.tx
                            {
                                cascadeless = false;
                                dependencies
                                    .entry(o.tx.clone())
                                    .or_default()
                                    .insert(v.tx.clone());
                                anomalies.insert(format!(
                                    "脏读：{} 的范围查询读取 {} 尚未提交的 {k}",
                                    o.tx, v.tx
                                ));
                            }
                        }
                    }
                    let found = versions
                        .iter()
                        .filter(|(k, _)| k.starts_with(&o.item))
                        .filter_map(|(k, vs)| {
                            vs.iter()
                                .rev()
                                .find(|v| {
                                    mode == "raw"
                                        || isolation == "ru"
                                        || v.committed
                                        || v.tx == o.tx
                                })
                                .filter(|v| v.value >= o.low && v.value <= o.high)
                                .map(|v| (k.clone(), v.value))
                        })
                        .collect::<Vec<_>>();
                    let key = (o.tx.clone(), o.item.clone(), o.low, o.high);
                    for (item, _) in &found {
                        executed.push(Operation {
                            kind: "read".into(),
                            item: item.clone(),
                            ..o.clone()
                        });
                    }
                    if let Some(old) = last_scan.get(&key) {
                        if old.iter().map(|x| &x.0).collect::<Vec<_>>()
                            != found.iter().map(|x| &x.0).collect::<Vec<_>>()
                        {
                            anomalies.insert(format!("幻读：{} 的范围查询成员发生变化", o.tx));
                        }
                    }
                    last_scan.insert(key, found.clone());
                    if mode == "locks" && isolation == "serializable" {
                        locks.push(Lock {
                            tx: o.tx.clone(),
                            item: o.item.clone(),
                            exclusive: false,
                            range: Some((o.low, o.high)),
                        });
                    } else if mode == "locks" && isolation == "rr" {
                        for (k, _) in &found {
                            locks.push(Lock {
                                tx: o.tx.clone(),
                                item: k.clone(),
                                exclusive: false,
                                range: None,
                            });
                        }
                    }
                    message = format!("范围 {} [{}..{}] → {:?}", o.item, o.low, o.high, found);
                }
                "commit" => {
                    if dependencies
                        .get(&o.tx)
                        .is_some_and(|deps| deps.iter().any(|t| !committed.contains(t)))
                    {
                        recoverable = false;
                        anomalies.insert(format!("不可恢复：{} 在其读取来源事务提交前提交", o.tx));
                    }
                    for vs in versions.values_mut() {
                        for v in vs.iter_mut().filter(|v| v.tx == o.tx) {
                            v.committed = true;
                        }
                    }
                    committed.insert(o.tx.clone());
                    locks.retain(|l| l.tx != o.tx);
                    message = "提交本事务的版本，释放全部锁".into();
                }
                "rollback" => {
                    for vs in versions.values_mut() {
                        vs.retain(|v| v.tx != o.tx);
                    }
                    versions.retain(|_, v| !v.is_empty());
                    locks.retain(|l| l.tx != o.tx);
                    for (t, deps) in &dependencies {
                        if deps.contains(&o.tx) {
                            anomalies
                                .insert(format!("级联回滚需求：{t} 读过已回滚事务 {} 的值", o.tx));
                        }
                    }
                    message = "移除本事务写入版本并释放锁".into();
                }
                _ => {}
            }
            if matches!(o.kind.as_str(), "commit" | "rollback") && !waiting.is_empty() {
                waits.retain(|(_, to)| to != &o.tx);
                let pending = waiting.drain(..).collect::<Vec<_>>();
                for i in pending.into_iter().rev() {
                    queue.push_front(i);
                }
            }
        }
        let data: BTreeMap<String, i64> = versions
            .iter()
            .filter_map(|(k, vs)| vs.last().map(|v| (k.clone(), v.value)))
            .collect();
        let version_text = versions
            .iter()
            .flat_map(|(k, vs)| {
                vs.iter().map(move |v| {
                    format!(
                        "{k}={} · {} · {}",
                        v.value,
                        v.tx,
                        if v.committed {
                            "已提交"
                        } else {
                            "未提交"
                        }
                    )
                })
            })
            .collect();
        let previous = steps
            .last()
            .map(|s: &ScheduleStep| &s.data)
            .unwrap_or(&initial);
        let changes = previous
            .keys()
            .chain(data.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|k| previous.get(*k) != data.get(*k))
            .map(|k| crate::animation::StateChange {
                target: k.clone(),
                before: previous
                    .get(k)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "不存在".into()),
                after: data
                    .get(k)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "已撤销".into()),
            })
            .collect();
        steps.push(ScheduleStep {
            source_line: o.line,
            changes,
            index: steps.len(),
            tx: o.tx.clone(),
            operation: format!("{} {} {}", o.kind, o.item, o.expression),
            status: status.into(),
            message,
            data,
            locals: locals.clone(),
            locks: locks
                .iter()
                .map(|l| {
                    format!(
                        "{} {} {}{}",
                        l.tx,
                        if l.exclusive { "X" } else { "S" },
                        l.item,
                        l.range
                            .map(|(a, b)| format!("[{a}..{b}]"))
                            .unwrap_or_default()
                    )
                })
                .collect(),
            waits: waits.iter().cloned().collect(),
            versions: version_text,
        });
    }
    let (_, deadlock) = topology(&analysis.transactions, &waits);
    let status = if !deadlock.is_empty() {
        anomalies.insert(format!("死锁：{}", deadlock.join(" → ")));
        "deadlock"
    } else if !waiting.is_empty() {
        "waiting"
    } else {
        "completed"
    };
    let transactions = analysis.transactions;
    let mut analysis = analyze(&executed, true, b)?;
    analysis.transactions = transactions;
    analysis
        .message
        .push_str(" 图中操作编号对应原始调度行号；判定采用实际执行次序。");
    if status != "completed" {
        analysis.view_serializable = None;
        analysis
            .message
            .push_str(" 调度尚未执行完；冲突图仅描述已执行前缀，不能据此证明完整调度。");
    }
    Ok(Simulation {
        steps,
        status: status.into(),
        anomalies: anomalies.into_iter().collect(),
        recoverable,
        cascadeless,
        analysis,
    })
}
fn expression(s: &str, vars: &BTreeMap<String, i64>) -> Result<i64, String> {
    let tokens = crate::script::tokenize(s);
    fn atom(
        t: &[crate::script::Token],
        i: &mut usize,
        v: &BTreeMap<String, i64>,
    ) -> Result<i64, String> {
        let Some(tok) = t.get(*i) else {
            return Err("缺少算术操作数".into());
        };
        *i += 1;
        if tok.text == "-" {
            return atom(t, i, v)?.checked_neg().ok_or("数值溢出".into());
        }
        if tok.text == "(" {
            let n = sum(t, i, v)?;
            if t.get(*i).is_none_or(|t| t.text != ")") {
                return Err("缺少 )".into());
            }
            *i += 1;
            return Ok(n);
        }
        tok.text.parse().or_else(|_| {
            v.get(&tok.text)
                .copied()
                .ok_or_else(|| format!("局部变量 {} 未读取或定义", tok.text))
        })
    }
    fn mul(
        t: &[crate::script::Token],
        i: &mut usize,
        v: &BTreeMap<String, i64>,
    ) -> Result<i64, String> {
        let mut n = atom(t, i, v)?;
        while let Some(op) = t.get(*i).filter(|t| matches!(t.text.as_str(), "*" | "/")) {
            *i += 1;
            let r = atom(t, i, v)?;
            n = if op.text == "*" {
                n.checked_mul(r)
            } else {
                n.checked_div(r)
            }
            .ok_or("除零或数值溢出")?;
        }
        Ok(n)
    }
    fn sum(
        t: &[crate::script::Token],
        i: &mut usize,
        v: &BTreeMap<String, i64>,
    ) -> Result<i64, String> {
        let mut n = mul(t, i, v)?;
        while let Some(op) = t.get(*i).filter(|t| matches!(t.text.as_str(), "+" | "-")) {
            *i += 1;
            let r = mul(t, i, v)?;
            n = if op.text == "+" {
                n.checked_add(r)
            } else {
                n.checked_sub(r)
            }
            .ok_or("数值溢出")?;
        }
        Ok(n)
    }
    let mut i = 0;
    let n = sum(&tokens, &mut i, vars)?;
    if i != tokens.len() {
        return Err("无法识别算术表达式".into());
    }
    if !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n) {
        return Err("教学变量超出精确整数范围 ±(2^53−1)".into());
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pasted_example_has_no_cycle() {
        let a = analyze_schedule(
            "T1 read A\nT2 write A\nT1 write B\nT2 read B",
            true,
            &Budget::default(),
        )
        .unwrap();
        assert!(a.conflict_serializable);
        assert_eq!(a.serial_order, vec!["T1", "T2"]);
    }
    #[test]
    fn blind_writes_view_not_conflict() {
        let a = analyze_schedule(
            "T1 write A\nT2 write A\nT1 write A",
            true,
            &Budget::default(),
        )
        .unwrap();
        assert!(!a.conflict_serializable);
        assert_eq!(a.view_serializable, Some(true));
        assert_eq!(a.view_order, vec!["T2", "T1"]);
    }
    #[test]
    fn lost_update_and_lock() {
        let s="T1 read A\nT2 read A\nT1 set A A-100\nT1 write A\nT1 commit\nT2 set A A-200\nT2 write A\nT2 commit";
        let r = simulate_schedule(
            s,
            BTreeMap::from([("A".into(), 300)]),
            "raw",
            "ru",
            &Budget::default(),
        )
        .unwrap();
        assert_eq!(r.steps.last().unwrap().data["A"], 100);
        assert!(r.anomalies.iter().any(|s| s.contains("丢失")));
    }
    #[test]
    fn dirty_rollback() {
        let s = "T1 write A 200\nT2 read A\nT2 commit\nT1 rollback";
        let r = simulate_schedule(
            s,
            BTreeMap::from([("A".into(), 100)]),
            "raw",
            "ru",
            &Budget::default(),
        )
        .unwrap();
        assert!(!r.recoverable);
        assert!(!r.cascadeless);
        assert_eq!(r.steps.last().unwrap().data["A"], 100);
    }
    #[test]
    fn deadlock() {
        let s = "T1 read A\nT2 read B\nT1 write B 1\nT2 write A 2";
        let r = simulate_schedule(
            s,
            BTreeMap::from([("A".into(), 10), ("B".into(), 20)]),
            "locks",
            "rr",
            &Budget::default(),
        )
        .unwrap();
        assert_eq!(r.status, "deadlock");
    }
}
