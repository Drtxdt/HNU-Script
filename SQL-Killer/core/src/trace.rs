use crate::{database, exercises::tables_equal, model::*};
use sqlparser::{
    ast::*,
    dialect::{MySqlDialect, SQLiteDialect},
    parser::Parser,
    tokenizer::{Location, Span},
};
use std::ops::ControlFlow;

pub struct PlannedStep {
    pub span: Span,
    pub kind: String,
    pub title: String,
    pub explanation: String,
    pub sql: String,
    pub details: Vec<String>,
    pub parents: Option<Vec<usize>>,
}
fn planned(kind: &str, title: impl ToString, sql: impl ToString, explanation: &str) -> PlannedStep {
    PlannedStep {
        span: Span::empty(),
        kind: kind.into(),
        title: title.to_string(),
        sql: sql.to_string(),
        explanation: explanation.into(),
        details: vec![],
        parents: None,
    }
}
struct Inspector {
    bad: Option<String>,
    subqueries: Vec<Expr>,
}
impl Visitor for Inspector {
    type Break = ();
    fn pre_visit_query(&mut self, q: &Query) -> ControlFlow<()> {
        if q.with.as_ref().is_some_and(|w| w.recursive) || !q.locks.is_empty() {
            self.bad = Some("递归 CTE 或加锁查询请使用真实执行／原生计划".into());
        }
        ControlFlow::Continue(())
    }
    fn pre_visit_expr(&mut self, e: &Expr) -> ControlFlow<()> {
        match e {
            Expr::Function(f) => {
                let name = f.name.to_string().to_uppercase();
                let allowed = [
                    "COUNT",
                    "SUM",
                    "AVG",
                    "MIN",
                    "MAX",
                    "COALESCE",
                    "NULLIF",
                    "ABS",
                    "ROUND",
                    "LOWER",
                    "UPPER",
                    "LENGTH",
                    "CHAR_LENGTH",
                    "SUBSTR",
                    "SUBSTRING",
                    "TRIM",
                    "LTRIM",
                    "RTRIM",
                    "CAST",
                    "IFNULL",
                    "CONCAT",
                    "REPLACE",
                    "INSTR",
                    "MOD",
                ];
                if f.over.is_some() || !allowed.contains(&name.as_str()) {
                    self.bad = Some(format!(
                        "{name} 不在确定性教学函数范围内；可正常运行 SQL 查看真实结果。"
                    ));
                }
            }
            Expr::Subquery(_) | Expr::Exists { .. } | Expr::InSubquery { .. } => {
                self.subqueries.push(e.clone())
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
fn span_range(source: &str, span: Span) -> Option<SourceRange> {
    fn offset(source: &str, point: Location) -> Option<usize> {
        if point.line == 0 || point.column == 0 {
            return None;
        }
        let mut base = 0;
        for (i, line) in source.split_inclusive('\n').enumerate() {
            if i + 1 == point.line as usize {
                return Some(
                    base + line
                        .char_indices()
                        .nth(point.column as usize - 1)
                        .map(|(n, _)| n)
                        .unwrap_or(line.len()),
                );
            }
            base += line.len();
        }
        Some(source.len())
    }
    let start = offset(source, span.start)?;
    let end = offset(source, span.end)?;
    (end > start).then(|| source_range(source, start, end))
}
pub fn plan(source: &str, engine: &str) -> Result<Vec<PlannedStep>, String> {
    let dialect: Box<dyn sqlparser::dialect::Dialect> = if engine == "mysql" {
        Box::new(MySqlDialect {})
    } else {
        Box::new(SQLiteDialect {})
    };
    let stmts = Parser::parse_sql(dialect.as_ref(), source).map_err(failure)?;
    if stmts.len() != 1 {
        return Err("请选择一条 SELECT 查询生成过程".into());
    }
    let Statement::Query(q) = &stmts[0] else {
        return Err("教学过程仅支持 SELECT 查询".into());
    };
    let mut inspector = Inspector {
        bad: None,
        subqueries: vec![],
    };
    let _ = q.visit(&mut inspector);
    if let Some(bad) = inspector.bad {
        return Err(bad);
    }
    let mut steps = Vec::new();
    build_query(q, "", &mut steps, 0)?;
    Ok(steps)
}
fn build_query(
    q: &Query,
    inherited: &str,
    steps: &mut Vec<PlannedStep>,
    depth: usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err("查询嵌套超过 16 层教学限制".into());
    }
    let prefix = q
        .with
        .as_ref()
        .map(|w| format!("{w} "))
        .unwrap_or_else(|| inherited.to_string());
    if let Some(w) = &q.with {
        for cte in &w.cte_tables {
            steps.push(planned(
                "cte",
                format!("CTE {}", cte.alias.name),
                format!("{prefix}SELECT * FROM {}", cte.alias.name),
                "公共表表达式是命名查询结果；此处展示逻辑结果，不声称引擎一定物化。",
            ));
            steps.last_mut().unwrap().span = cte.span();
        }
    }
    match q.body.as_ref() {
        SetExpr::Select(s) => {
            if s.into.is_some() || s.qualify.is_some() || !s.named_window.is_empty() {
                return Err("该查询含过程视图暂不支持的扩展子句".into());
            }
            let from = if s.from.is_empty() {
                String::new()
            } else {
                format!(
                    " FROM {}",
                    s.from
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            if !s.from.is_empty() {
                let mut from_parents = Vec::new();
                for f in &s.from {
                    let left = format!("{prefix}SELECT * FROM {}", f.relation);
                    steps.push(planned(
                        "source",
                        format!("读取 {}", f.relation),
                        left.clone(),
                        "读取该关系的快照。",
                    ));
                    steps.last_mut().unwrap().parents = Some(vec![]);
                    steps.last_mut().unwrap().span = f.relation.span();
                    let mut left_id = steps.len() - 1;
                    let mut partial = f.relation.to_string();
                    for join in &f.joins {
                        let input = format!("{prefix}SELECT * FROM {partial}");
                        let mut right = planned(
                            "source",
                            format!("读取 {}", join.relation),
                            format!("{prefix}SELECT * FROM {}", join.relation),
                            "读取连接右侧的冻结输入关系。",
                        );
                        right.parents = Some(vec![]);
                        right.span = join.relation.span();
                        let right_id = steps.len();
                        steps.push(right);
                        partial.push(' ');
                        partial.push_str(&join.to_string());
                        let mut p=planned("join",format!("连接 {}",join.relation),format!("{prefix}SELECT * FROM {partial}"),"按连接条件匹配。外连接会保留相应侧的未匹配记录并补 NULL；重复匹配仍保留。" );
                        p.details = vec![input, format!("{prefix}SELECT * FROM {}", join.relation)];
                        p.parents = Some(vec![left_id, right_id]);
                        p.span = join.span();
                        left_id = steps.len();
                        steps.push(p);
                    }
                    from_parents.push(left_id);
                }
                if s.from.len() > 1 {
                    steps.push(planned(
                        "join",
                        "FROM 笛卡尔积",
                        format!("{prefix}SELECT *{from}"),
                        "逗号连接先组合关系；WHERE 再筛选。",
                    ));
                    steps.last_mut().unwrap().parents = Some(from_parents);
                }
            }
            let mut inspect = Inspector {
                bad: None,
                subqueries: vec![],
            };
            let _ = s.selection.visit(&mut inspect);
            let _ = s.projection.visit(&mut inspect);
            for (index, expr) in inspect.subqueries.iter().enumerate() {
                let query = format!(
                    "{prefix}SELECT {}, {expr} AS __subquery_value{from}",
                    if from.is_empty() {
                        "1 AS __outer_row"
                    } else {
                        "*"
                    }
                );
                steps.push(planned("subquery",format!("子查询 {}",index+1),query,"逐个外层行查看子查询值／成员或存在性判断；这是逻辑解释，优化器可能将其改写为连接。"));
                steps.last_mut().unwrap().span = expr.span();
            }
            let where_clause = s
                .selection
                .as_ref()
                .map(|e| format!(" WHERE {e}"))
                .unwrap_or_default();
            if !where_clause.is_empty() {
                let mut filter = planned(
                    "filter",
                    format!("WHERE {}", s.selection.as_ref().unwrap()),
                    format!("{prefix}SELECT *{from}{where_clause}"),
                    "只保留谓词为 TRUE 的行；FALSE 与 UNKNOWN 均被过滤。",
                );
                let predicate = s.selection.as_ref().unwrap();
                filter.details.push(format!("{prefix}SELECT *, CASE WHEN ({predicate}) THEN 1 WHEN NOT ({predicate}) THEN 0 ELSE NULL END AS __sqlk_truth{from}"));
                filter.span = predicate.span();
                steps.push(filter);
            }
            let group_key = |key: &Expr| {
                if let Ok(index) = key.to_string().parse::<usize>() {
                    if let Some(
                        SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. },
                    ) = index.checked_sub(1).and_then(|i| s.projection.get(i))
                    {
                        return expr.to_string();
                    }
                }
                key.to_string()
            };
            let grouped = match &s.group_by {
                GroupByExpr::Expressions(v, _) => !v.is_empty(),
                _ => true,
            };
            if let GroupByExpr::Expressions(keys, _) = &s.group_by {
                if !keys.is_empty() {
                    let keys = keys.iter().map(&group_key).collect::<Vec<_>>().join(", ");
                    let mut p=planned("group",format!("GROUP BY {keys}"),format!("{prefix}SELECT {keys}, COUNT(*) AS __group_rows, DENSE_RANK() OVER (ORDER BY {keys}) AS __sqlk_group{from}{where_clause} GROUP BY {keys}"),"相同分组键构成一组，NULL 分组键归入同一组。下表可查看每组的原始成员。" );
                    p.details.push(format!(
                        "{prefix}SELECT *, DENSE_RANK() OVER (ORDER BY {keys}) AS __sqlk_group{from}{where_clause} ORDER BY {keys}"
                    ));
                    p.span = s.group_by.span();
                    steps.push(p);
                }
            }
            let mut intermediate = s.as_ref().clone();
            intermediate.distinct = None;
            intermediate.having = None;
            let aggregates = s.projection.iter().any(|item| {
                let t = item.to_string().to_uppercase();
                ["COUNT(", "SUM(", "AVG(", "MIN(", "MAX("]
                    .iter()
                    .any(|f| t.contains(f))
            });
            let mut projection=planned(if grouped||aggregates{"aggregate"}else{"project"},if grouped{"计算分组聚合"}else{"SELECT 投影／聚合"},format!("{prefix}{intermediate}"),"计算输出表达式；COUNT(*) 计行，COUNT(列) 不计 NULL；无 GROUP BY 的聚合在空输入上仍产生一行。");
            if aggregates {
                let partition = match &s.group_by {
                    GroupByExpr::Expressions(keys, _) if !keys.is_empty() => format!(
                        "PARTITION BY {} ",
                        keys.iter().map(&group_key).collect::<Vec<_>>().join(", ")
                    ),
                    _ => String::new(),
                };
                let running = s.projection.iter().filter_map(|item| {
                    let expr = match item { SelectItem::UnnamedExpr(e) | SelectItem::ExprWithAlias {expr:e,..} => e, _ => return None };
                    if let Expr::Function(f) = expr {
                        let text = f.to_string();
                        if ["COUNT", "SUM", "AVG", "MIN", "MAX"].contains(&f.name.to_string().to_uppercase().as_str()) && !text.to_uppercase().contains("DISTINCT") {
                            return Some(format!("{text} OVER ({partition}ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS {}", quote_identifier(&format!("__sqlk_acc:{text}"))));
                        }
                    }
                    None
                }).collect::<Vec<_>>();
                let extra = if running.is_empty() {
                    String::new()
                } else {
                    format!(", {}", running.join(", "))
                };
                projection
                    .details
                    .push(format!("{prefix}SELECT *{extra}{from}{where_clause}"));
            }
            projection.span = Span::union_iter(s.projection.iter().map(Spanned::span));
            steps.push(projection);
            if let Some(having) = &s.having {
                intermediate.having = s.having.clone();
                let mut h = planned(
                    "having",
                    format!("HAVING {having}"),
                    format!("{prefix}{intermediate}"),
                    "对聚合后的组进行筛选。",
                );
                let mut predicate = having.clone();
                let _ = visit_expressions_mut(&mut predicate, |expr| {
                    if let Expr::Identifier(name) = expr {
                        if let Some(value) = s.projection.iter().find_map(|item| match item {
                            SelectItem::ExprWithAlias { expr, alias }
                                if alias.value.eq_ignore_ascii_case(&name.value) =>
                            {
                                Some(expr.clone())
                            }
                            _ => None,
                        }) {
                            *expr = value;
                        }
                    }
                    ControlFlow::<()>::Continue(())
                });
                let helper = format!("SELECT CASE WHEN ({predicate}) THEN 1 WHEN NOT ({predicate}) THEN 0 ELSE NULL END AS __sqlk_truth");
                if let Ok(mut statements) = Parser::parse_sql(&SQLiteDialect {}, &helper) {
                    if let Statement::Query(q) = statements.remove(0) {
                        if let SetExpr::Select(select) = *q.body {
                            let mut probe = intermediate.clone();
                            probe.having = None;
                            probe.projection.extend(select.projection);
                            h.details.push(format!("{prefix}{probe}"));
                        }
                    }
                }
                h.span = having.span();
                steps.push(h);
            }
            if s.distinct.is_some() {
                intermediate.distinct = s.distinct.clone();
                steps.push(planned(
                    "distinct",
                    "DISTINCT 去重",
                    format!("{prefix}{intermediate}"),
                    "仅在明确要求时去除重复输出行。",
                ));
            }
        }
        SetExpr::SetOperation {
            left,
            right,
            op,
            set_quantifier,
        } => {
            let mut branch_ids = vec![];
            for body in [left, right] {
                let mut branch = q.clone();
                branch.with = None;
                branch.body = body.clone();
                branch.order_by = None;
                branch.limit_clause = None;
                build_query(&branch, &prefix, steps, depth + 1)?;
                branch_ids.push(steps.len() - 1);
            }
            steps.push(planned(
                "set",
                format!("{op} {set_quantifier}"),
                format!("{prefix}{}", q.body),
                "集合运算按列位置组合；ALL 保留相应重复次数，普通集合操作去重。",
            ));
            steps.last_mut().unwrap().parents = Some(branch_ids);
            steps.last_mut().unwrap().span = q.body.span();
        }
        SetExpr::Query(inner) => build_query(inner, &prefix, steps, depth + 1)?,
        SetExpr::Values(_) => steps.push(planned(
            "source",
            "VALUES",
            format!("{prefix}{}", q.body),
            "显式值构成输入关系。",
        )),
        _ => return Err("该查询体尚不支持过程分析".into()),
    }
    if let Some(order_by) = &q.order_by {
        let mut sorted = q.clone();
        sorted.limit_clause = None;
        sorted.fetch = None;
        let sql = if sorted.with.is_none() {
            format!("{inherited}{sorted}")
        } else {
            sorted.to_string()
        };
        steps.push(planned(
            "sort",
            "ORDER BY 排序",
            sql,
            "只对声明的排序键保证顺序；相同键之间未声明的次序没有保证。",
        ));
        steps.last_mut().unwrap().span = order_by.span();
    }
    if q.limit_clause.is_some() || q.fetch.is_some() {
        steps.push(planned(
            "limit",
            "分页／取前若干行",
            format!("{inherited}{q}"),
            "在排序之后应用行数和偏移；没有 ORDER BY 时选中行不具有稳定顺序保证。",
        ));
        steps.last_mut().unwrap().span = q
            .limit_clause
            .as_ref()
            .map(Spanned::span)
            .unwrap_or_else(Span::empty);
    }
    Ok(())
}
pub fn trace_query(c: &rusqlite::Connection, source: &str, budget: &Budget) -> Trace {
    let mut trace = Trace {
        snapshot: "SQLite 一致性副本；逻辑过程，不是物理执行轨迹".into(),
        ..Trace::default()
    };
    let run = (|| -> Result<(), String> {
        let snapshot = database::snapshot(c)?;
        let b = budget.teaching();
        let plan = plan(source, "sqlite")?;
        for p in plan {
            b.check()?;
            if trace.steps.len() >= b.max_steps {
                return Err("过程步骤超过限制".into());
            }
            let table = database::query(&snapshot, &p.sql, &b)?;
            if table.truncated {
                return Err("中间结果超过教学上限；未生成完整过程".into());
            }
            let mut details = Vec::new();
            for q in p.details {
                let t = database::query(&snapshot, &q, &b)?;
                if t.truncated {
                    return Err("明细超过教学上限".into());
                }
                details.push(t);
            }
            let id = trace.steps.len();
            trace.steps.push(TraceStep {
                id,
                kind: p.kind,
                title: p.title,
                explanation: p.explanation,
                sql: p.sql,
                table,
                parents: p
                    .parents
                    .unwrap_or_else(|| if id > 0 { vec![id - 1] } else { vec![] }),
                details,
                events: vec![],
                source_range: span_range(source, p.span),
            });
        }
        crate::animation::enrich(&mut trace, source, &b)?;
        trace.result = database::query(&snapshot, source, &b)?;
        if trace.result.truncated {
            return Err("结果超过教学上限".into());
        }
        trace.verified = trace
            .steps
            .last()
            .map(|s| tables_equal(&s.table, &trace.result, false, false))
            .unwrap_or(false);
        if !trace.verified {
            return Err("过程最终结果与原查询不一致，已停止展示完整结论".into());
        }
        Ok(())
    })();
    match run {
        Ok(()) => {
            trace.status = "completed".into();
            trace.message = "已与同一快照下的原 SQL 结果核对".into();
        }
        Err(e) => {
            trace.status = if budget.check().is_err() || e.contains("超过") || e.contains("上限")
            {
                "incomplete"
            } else {
                "unsupported"
            }
            .into();
            trace.message = e;
            trace.verified = false;
            trace.steps.clear();
        }
    }
    trace
}
pub async fn trace_mysql(
    cfg: &database::mysql::MysqlConfig,
    source: &str,
    budget: &Budget,
) -> Trace {
    use mysql_async::prelude::Queryable;
    let mut trace = Trace {
        snapshot: "MySQL 独立 REPEATABLE READ 只读事务；不含编辑会话的未提交写入".into(),
        ..Trace::default()
    };
    let run = async {
        let planned = plan(source, "mysql")?;
        let mut c = database::mysql::connect(cfg).await?;
        c.query_drop("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .await
            .map_err(failure)?;
        c.query_drop("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
            .await
            .map_err(failure)?;
        let b = budget.teaching();
        for p in planned {
            b.check()?;
            if trace.steps.len() >= b.max_steps {
                return Err("过程步骤超过限制".into());
            }
            let table = database::mysql::query(&mut c, &p.sql, &b).await?;
            if table.truncated {
                return Err("中间结果超过教学限制".to_string());
            }
            let mut details = vec![];
            for sql in p.details {
                let detail = database::mysql::query(&mut c, &sql, &b).await?;
                if detail.truncated {
                    return Err("明细超过教学上限".to_string());
                }
                details.push(detail);
            }
            let id = trace.steps.len();
            trace.steps.push(TraceStep {
                id,
                kind: p.kind,
                title: p.title,
                explanation: p.explanation,
                sql: p.sql,
                table,
                parents: p
                    .parents
                    .unwrap_or_else(|| if id > 0 { vec![id - 1] } else { vec![] }),
                details,
                events: vec![],
                source_range: span_range(source, p.span),
            });
        }
        crate::animation::enrich(&mut trace, source, &b)?;
        trace.result = database::mysql::query(&mut c, source, &b).await?;
        trace.verified = trace.steps.last().is_some_and(|s| {
            !trace.result.truncated && tables_equal(&s.table, &trace.result, false, false)
        });
        c.query_drop("ROLLBACK").await.map_err(failure)?;
        c.disconnect().await.map_err(failure)?;
        if !trace.verified {
            return Err("过程与原查询不一致".into());
        }
        Ok::<(), String>(())
    };
    match tokio::time::timeout(std::time::Duration::from_secs(10), run).await {
        Ok(Ok(())) => {
            trace.status = "completed".into();
            trace.message = "同一只读事务中核对完成".into()
        }
        other => {
            trace.status = "incomplete".into();
            trace.message = match other {
                Ok(Err(e)) => e,
                _ => "MySQL 过程分析超时".into(),
            };
            trace.verified = false;
            trace.steps.clear();
        }
    }
    trace
}
