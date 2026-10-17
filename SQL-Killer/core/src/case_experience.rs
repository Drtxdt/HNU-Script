//! Presentation state and authoritative investigation requirements.
use crate::{
    cases::{self, CaseProgress, CaseTask, EvidenceSnapshot},
    database,
    model::*,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;
pub const CLOSURE_REVISION: u32 = 2;
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CardPresentation {
    pub x: f64,
    pub y: f64,
    pub view: String,
    pub category: usize,
    pub value: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct EvidenceRelation {
    pub from: String,
    pub to: String,
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct EvidenceBoard {
    pub cards: BTreeMap<String, CardPresentation>,
    pub relations: Vec<EvidenceRelation>,
    pub zoom: f64,
    pub x: f64,
    pub y: f64,
}
impl Default for EvidenceBoard {
    fn default() -> Self {
        Self {
            cards: BTreeMap::new(),
            relations: vec![],
            zoom: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseConclusion {
    pub round: u32,
    pub cause: String,
    pub model_strategy: String,
    pub mysql_strategy: String,
    pub closed: bool,
    pub closure_revision: u32,
    pub evidence_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseContext {
    pub mode: String,
    pub stage: usize,
    pub title: String,
    pub narrative: String,
    pub tasks: Vec<CaseTask>,
    pub schema: Schema,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseRequirement {
    pub id: String,
    pub title: String,
    pub passed: bool,
    pub task: String,
    pub action: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct CaseReview {
    pub items: Vec<CaseRequirement>,
    pub can_close: bool,
    pub revision: u32,
}
pub fn stage(p: &CaseProgress, b: &Budget) -> Result<usize, String> {
    let mut stage = 0;
    for task in ["capacity", "requests", "timeline", "reproduce"] {
        if cases::grade(
            task,
            p.answers.get(task).map(String::as_str).unwrap_or(""),
            p.seed,
            b,
        )
        .status
            != "passed"
        {
            break;
        }
        stage += 1;
    }
    b.check()?;
    Ok(stage)
}
pub fn accessible_fixture(p: &CaseProgress, b: &Budget) -> Result<rusqlite::Connection, String> {
    let c = cases::fixture(p.seed, 0)?;
    if p.mode != "expert" && stage(p, b)? == 0 {
        c.execute_batch("DROP TABLE RequestLog").map_err(failure)?;
    }
    Ok(c)
}
pub fn context(p: &CaseProgress, b: &Budget) -> Result<CaseContext, String> {
    let stage = stage(p, b)?;
    let mode = if p.mode == "expert" {
        "expert"
    } else {
        "guided"
    };
    let tasks = cases::spec()
        .tasks
        .into_iter()
        .enumerate()
        .filter(|(i, _)| mode == "expert" || *i <= stage)
        .map(|(_, mut t)| {
            if mode == "expert" {
                t.starter.clear();
            }
            t
        })
        .collect();
    let narrative = [
        "教务处收到选课记录与系统显示不一致的投诉。管理员认为数据正常，请先核对记录。",
        "人数核验发现异常。管理员提供了请求日志，请核实相关请求及学生身份。",
        "已定位需要核实的请求。请用事件序号重建读取、写入和提交之间的关系。",
        "日志和请求已经能够互相印证。现在用并发回放检验你的解释。",
        "用修复实验检查容量、计数一致性和重复请求，再形成可复核的解释链。",
    ][stage.min(4)];
    Ok(CaseContext {
        mode: mode.into(),
        stage,
        title: if stage > 0 {
            "选课系统里的第 41 个人"
        } else {
            "选课异常调查"
        }
        .into(),
        narrative: narrative.into(),
        tasks,
        schema: database::schema(&accessible_fixture(p, b)?)?,
    })
}
pub fn require_task(p: &CaseProgress, task: &str, b: &Budget) -> Result<(), String> {
    if !context(p, b)?.tasks.iter().any(|t| t.id == task) {
        return Err("该调查阶段尚未开放，请完成当前核验，或切换自主调查。".into());
    }
    Ok(())
}
pub fn validate_board(p: &CaseProgress) -> Result<(), String> {
    let board = &p.board;
    if board.relations.len() > 400
        || board.cards.len() > 20
        || p.history.len() > 50
        || !["", "guided", "expert"].contains(&p.mode.as_str())
    {
        return Err("案件板面或模式无效".into());
    }
    for n in [board.x, board.y, board.zoom]
        .into_iter()
        .chain(board.cards.values().flat_map(|c| [c.x, c.y]))
    {
        if !n.is_finite() || n.abs() > 1_000_000.0 {
            return Err("板面坐标无效".into());
        }
    }
    if !(0.25..=3.0).contains(&board.zoom) {
        return Err("板面缩放无效".into());
    }
    let ids: std::collections::BTreeSet<_> = p.evidence.iter().map(|e| e.id.as_str()).collect();
    if board.cards.keys().any(|id| !ids.contains(id.as_str())) {
        return Err("板面包含不存在的证据".into());
    }
    let mut relations = std::collections::BTreeSet::new();
    for e in &board.relations {
        if e.from == e.to
            || !ids.contains(e.from.as_str())
            || !ids.contains(e.to.as_str())
            || !["related", "explains"].contains(&e.kind.as_str())
            || !relations.insert((&e.from, &e.to, &e.kind))
        {
            return Err("证据关系无效或重复".into());
        }
    }
    Ok(())
}
pub fn migrate(p: &mut CaseProgress) {
    if p.mode.is_empty() {
        p.mode = "guided".into();
    }
    for (i, e) in p.evidence.iter().enumerate() {
        p.board
            .cards
            .entry(e.id.clone())
            .or_insert(CardPresentation {
                x: (i % 3) as f64 * 350.0,
                y: (i / 3) as f64 * 300.0,
                view: "table".into(),
                ..Default::default()
            });
        for target in &e.links {
            if !p
                .board
                .relations
                .iter()
                .any(|r| r.from == e.id && r.to == *target)
            {
                p.board.relations.push(EvidenceRelation {
                    from: e.id.clone(),
                    to: target.clone(),
                    kind: "related".into(),
                });
            }
        }
    }
}
fn valid_card(e: &EvidenceSnapshot, p: &CaseProgress, b: &Budget) -> Result<bool, String> {
    if e.case_version != cases::VERSION
        || e.seed != p.seed
        || e.snapshot != cases::snapshot(p.seed)
        || e.table.rows.is_empty()
    {
        return Ok(false);
    }
    if cases::grade(&e.task, &e.sql, p.seed, b).status != "passed" {
        return Ok(false);
    }
    let fresh = cases::query(&cases::fixture(p.seed, 0)?, &e.sql, b)?;
    if fresh.columns != e.table.columns {
        return Ok(false);
    }
    let mut available = fresh.rows;
    for row in &e.table.rows {
        if let Some(i) = available.iter().position(|r| r == row) {
            available.remove(i);
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}
pub fn review(p: &CaseProgress, b: &Budget) -> Result<CaseReview, String> {
    cases::check_context(cases::ID, p.version)?;
    cases::validate_evidence(&p.evidence)?;
    validate_board(p)?;
    let mut items = vec![];
    let mut add = |id: &str, title: &str, passed: bool, task: &str, action: &str| {
        items.push(CaseRequirement {
            id: id.into(),
            title: title.into(),
            passed,
            task: task.into(),
            action: action.into(),
        })
    };
    for (task, title) in [
        ("capacity", "人数核验通过"),
        ("requests", "请求身份核验通过"),
        ("timeline", "事件顺序核验通过"),
        ("reproduce", "并发复现解释通过"),
    ] {
        let passed = cases::grade(
            task,
            p.answers.get(task).map(String::as_str).unwrap_or(""),
            p.seed,
            b,
        )
        .status
            == "passed";
        add(task, title, passed, task, "answer");
    }
    let mut valid = BTreeMap::new();
    for card in &p.evidence {
        valid.insert(card.id.as_str(), valid_card(card, p, b)?);
    }
    for (task, title) in [
        ("capacity", "固定有效人数证据"),
        ("requests", "固定有效请求证据"),
        ("timeline", "固定有效日志证据"),
    ] {
        add(
            &format!("evidence-{task}"),
            title,
            p.evidence
                .iter()
                .any(|e| e.task == task && valid[&e.id.as_str()]),
            task,
            "evidence",
        );
    }
    let edge = |a: &str, c: &str| {
        p.board
            .relations
            .iter()
            .any(|r| r.from == a && r.to == c && r.kind == "explains")
    };
    let chain = p
        .evidence
        .iter()
        .filter(|a| a.task == "capacity" && valid[&a.id.as_str()])
        .any(|a| {
            p.evidence
                .iter()
                .filter(|r| r.task == "requests" && valid[&r.id.as_str()] && edge(&a.id, &r.id))
                .any(|r| {
                    p.evidence.iter().any(|t| {
                        t.task == "timeline"
                            && valid[&t.id.as_str()]
                            && edge(&r.id, &t.id)
                            && t.hypothesis == "stale_read"
                            && p.cause == "stale_read"
                    })
                })
        });
    add(
        "chain",
        "人数 → 请求 → 日志 → 根因解释链",
        chain,
        "timeline",
        "board",
    );
    let repair = if p.model_strategy.is_empty() {
        false
    } else {
        cases::validate_repair(&p.model_strategy, b)?.status == "passed"
    };
    add(
        "repair",
        "修复策略通过模型边界检查",
        repair,
        "repair",
        "repair",
    );
    b.check()?;
    Ok(CaseReview {
        can_close: items.iter().all(|i| i.passed),
        items,
        revision: CLOSURE_REVISION,
    })
}
