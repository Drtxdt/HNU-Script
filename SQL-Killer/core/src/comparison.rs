//! Conservative operation alignment; differences are observations, not causal proof.
use crate::{
    exercises::{compare_tables, TableDiff},
    model::*,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct ComparisonStep {
    pub key: String,
    pub kind: String,
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub diff: Option<TableDiff>,
    pub changed_condition: bool,
    pub message: String,
    pub left_range: Option<SourceRange>,
    pub right_range: Option<SourceRange>,
}
fn normalized(sql: &str) -> String {
    crate::script::tokenize(sql)
        .into_iter()
        .map(|t| {
            if t.quoted {
                t.text
            } else {
                t.text.to_uppercase()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn range_for(sql: &str, step: &TraceStep) -> Option<SourceRange> {
    let r = step.source_range.clone()?;
    if step.kind != "join" {
        return Some(r);
    }
    let tokens = crate::script::tokenize(sql);
    if let Some(i) = tokens
        .iter()
        .rposition(|t| !t.quoted && t.text.eq_ignore_ascii_case("JOIN") && t.start <= r.start)
    {
        let mut start = i;
        while start > 0
            && [
                "LEFT", "RIGHT", "INNER", "FULL", "CROSS", "OUTER", "NATURAL",
            ]
            .iter()
            .any(|k| tokens[start - 1].text.eq_ignore_ascii_case(k))
        {
            start -= 1;
        }
        return Some(source_range(sql, tokens[start].start, r.end));
    }
    Some(r)
}
fn fragment(sql: &str, step: &TraceStep) -> String {
    range_for(sql, step)
        .as_ref()
        .and_then(|r| sql.get(r.start..r.end))
        .filter(|s| !s.is_empty())
        .map(normalized)
        .unwrap_or_else(|| normalized(&step.sql))
}
pub fn align(left: &Trace, right: &Trace, left_sql: &str, right_sql: &str) -> Vec<ComparisonStep> {
    let mut used = std::collections::BTreeSet::new();
    let mut out = vec![];
    for a in &left.steps {
        let left_count = left.steps.iter().filter(|s| s.kind == a.kind).count();
        let candidates: Vec<_> = right
            .steps
            .iter()
            .filter(|s| s.kind == a.kind && !used.contains(&s.id))
            .collect();
        let matched = if left_count == 1 && candidates.len() == 1 {
            candidates.first().copied()
        } else {
            candidates
                .into_iter()
                .find(|b| normalized(&a.sql) == normalized(&b.sql))
        };
        if let Some(b) = matched {
            used.insert(b.id);
            let diff = compare_tables(&a.table, &b.table, false, false);
            let changed = fragment(left_sql, a) != fragment(right_sql, b)
                || (a.kind == "join" && normalized(&a.sql) != normalized(&b.sql));
            let message = if changed {
                "对应操作的条件或表达式不同；可观察中间结果，尚不能单凭此证明因果。"
            } else if !diff.equal {
                "对应操作的中间结果不同；差异可能来自上游输入。"
            } else {
                "对应操作的当前结果相同。"
            };
            out.push(ComparisonStep {
                key: format!("{}:{}:{}", a.kind, a.id, b.id),
                kind: a.kind.clone(),
                left: Some(a.id),
                right: Some(b.id),
                diff: Some(diff),
                changed_condition: changed,
                message: message.into(),
                left_range: range_for(left_sql, a),
                right_range: range_for(right_sql, b),
            });
        } else {
            out.push(ComparisonStep {
                key: format!("left:{}", a.id),
                kind: a.kind.clone(),
                left: Some(a.id),
                right: None,
                diff: None,
                changed_condition: false,
                message: "没有可靠的对应操作，单独展示查询 A 的步骤。".into(),
                left_range: range_for(left_sql, a),
                right_range: None,
            });
        }
    }
    for b in right.steps.iter().filter(|s| !used.contains(&s.id)) {
        out.push(ComparisonStep {
            key: format!("right:{}", b.id),
            kind: b.kind.clone(),
            left: None,
            right: Some(b.id),
            diff: None,
            changed_condition: false,
            message: "没有可靠的对应操作，单独展示查询 B 的步骤。".into(),
            left_range: None,
            right_range: range_for(right_sql, b),
        });
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cases, database};
    #[test]
    fn join_change_highlights_first_comparable_difference() {
        let c = database::memory("student").unwrap();
        c.execute(
            "INSERT INTO Student(Sno,Sname) VALUES('unmatched','Unmatched')",
            [],
        )
        .unwrap();
        let r = cases::compare_queries(
            &c,
            "SELECT S.Sno,C.Cno FROM Student S LEFT JOIN SC C ON S.Sno=C.Sno",
            "SELECT S.Sno,C.Cno FROM Student S INNER JOIN SC C ON S.Sno=C.Sno",
            &Budget::default(),
        )
        .unwrap();
        assert!(!r.diff.equal);
        assert!(r
            .steps
            .iter()
            .any(|s| s.kind == "join" && s.changed_condition));
        assert!(r.first_difference.is_some());
    }
    #[test]
    fn formatting_is_not_a_condition_change() {
        let c = database::memory("student").unwrap();
        let r = cases::compare_queries(
            &c,
            "select Sno from Student where Sage>18",
            "SELECT Sno FROM Student WHERE Sage > 18",
            &Budget::default(),
        )
        .unwrap();
        assert!(r.diff.equal);
        assert!(r.steps.iter().all(|s| !s.changed_condition));
    }
}
