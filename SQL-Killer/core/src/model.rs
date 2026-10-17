use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[ts(export, export_to = "../../src/generated/")]
pub enum Cell {
    Null,
    Integer(String),
    Unsigned(String),
    Decimal(String),
    Real(f64),
    Text(String),
    Binary(String),
    Date(String),
    Time(String),
}
impl Cell {
    pub fn display(&self) -> String {
        match self {
            Self::Null => "NULL".into(),
            Self::Real(v) => v.to_string(),
            Self::Integer(v)
            | Self::Unsigned(v)
            | Self::Decimal(v)
            | Self::Text(v)
            | Self::Binary(v)
            | Self::Date(v)
            | Self::Time(v) => v.clone(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct ForeignKey {
    pub columns: Vec<String>,
    pub target: String,
    pub target_columns: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Relation {
    pub name: String,
    pub kind: String,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKey>,
    pub definition: String,
    pub indexes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Schema {
    pub relations: Vec<Relation>,
    pub routines: Vec<String>,
    pub version: u64,
    pub engine: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}
pub fn source_range(source: &str, start: usize, end: usize) -> SourceRange {
    fn pos(s: &str, n: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;
        for (i, c) in s.char_indices() {
            if i >= n {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 1
            } else {
                col += c.len_utf16()
            }
        }
        (line, col)
    }
    let (line, column) = pos(source, start);
    let (end_line, end_column) = pos(source, end);
    SourceRange {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Diagnostic {
    pub severity: String,
    pub category: String,
    pub message: String,
    pub range: SourceRange,
    pub replacement: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Completion {
    pub label: String,
    pub insert_text: String,
    pub kind: String,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    pub ast: Vec<String>,
    pub statements: Vec<SourceRange>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct StatementResult {
    pub sql: String,
    pub table: Table,
    pub affected: u64,
    pub elapsed_ms: u64,
    pub range: SourceRange,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Execution {
    #[serde(default)]
    pub modified: bool,
    pub results: Vec<StatementResult>,
    pub status: String,
    pub error: Option<String>,
    pub error_range: Option<SourceRange>,
    pub transaction: String,
    pub elapsed_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct TraceStep {
    pub id: usize,
    pub kind: String,
    pub title: String,
    pub explanation: String,
    pub sql: String,
    pub table: Table,
    pub parents: Vec<usize>,
    pub details: Vec<Table>,
    #[serde(default)]
    pub events: Vec<crate::animation::AnimationEvent>,
    #[serde(default)]
    pub source_range: Option<SourceRange>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Trace {
    #[serde(default)]
    pub source: String,
    pub steps: Vec<TraceStep>,
    pub result: Table,
    pub status: String,
    pub message: String,
    pub snapshot: String,
    pub verified: bool,
}
#[derive(Clone)]
pub struct Budget {
    pub cancelled: Arc<AtomicBool>,
    pub deadline: Instant,
    pub max_rows: usize,
    pub max_bytes: usize,
    pub max_steps: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(10),
            max_rows: 50_000,
            max_bytes: 32 * 1024 * 1024,
            max_steps: 20_000,
        }
    }
}
impl Budget {
    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err("任务已取消".into())
        } else if Instant::now() > self.deadline {
            Err("任务超过时间限制".into())
        } else {
            Ok(())
        }
    }
    pub fn teaching(&self) -> Self {
        Self {
            max_rows: self.max_rows.min(10_000),
            ..self.clone()
        }
    }
}
pub fn quote_identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
pub fn mysql_identifier(s: &str) -> String {
    format!("`{}`", s.replace('`', "``"))
}
pub fn failure(message: impl ToString) -> String {
    message.to_string()
}
