//! Offline course content and native-engine prediction checkpoints.
use crate::{database, exercises::Question, model::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct LearningProgress {
    pub attempts: u32,
    pub hints_used: usize,
    pub solution_viewed: bool,
    pub assisted: bool,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct LearningState {
    pub mode: String,
    pub last_question: String,
    pub attempts: BTreeMap<String, LearningProgress>,
    pub predictions: BTreeMap<String, bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct LessonSpec {
    pub id: String,
    pub title: String,
    pub explanation: String,
    pub question: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct PredictionSpec {
    pub id: String,
    pub title: String,
    pub prompt: String,
    pub choices: Vec<String>,
    pub sql: String,
    pub explanation: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct PredictionResult {
    pub correct: bool,
    pub table: Table,
    pub explanation: String,
}
#[derive(Deserialize)]
struct Teaching {
    hints: Vec<String>,
    explanation: String,
    knowledge: String,
}
pub fn enrich(questions: &mut [Question]) {
    let content: BTreeMap<String, Teaching> =
        serde_json::from_str(include_str!("../../curriculum/teaching.json"))
            .expect("validated bundled teaching content");
    for q in questions {
        let t = content
            .get(&q.id)
            .expect("every question has teaching content");
        q.hints = t.hints.clone();
        q.hint = q.hints[0].clone();
        q.explanation = t.explanation.clone();
        q.knowledge = t.knowledge.clone();
        q.answer_ui = if matches!(q.kind.as_str(), "concept" | "schedule") {
            "structured"
        } else {
            "editor"
        }
        .into();
    }
}
pub fn lessons() -> Vec<LessonSpec> {
    serde_json::from_str(include_str!("../../curriculum/lessons.json")).expect("validated lessons")
}
pub fn predictions() -> Vec<PredictionSpec> {
    serde_json::from_str(include_str!("../../curriculum/predictions.json"))
        .expect("validated predictions")
}
pub fn predict(id: &str, choice: usize, b: &Budget) -> Result<PredictionResult, String> {
    let p = predictions()
        .into_iter()
        .find(|p| p.id == id)
        .ok_or("预测检查点不存在")?;
    let table = database::query(&database::memory("student")?, &p.sql, b)?;
    if table.truncated {
        return Err("预测结果不完整".into());
    }
    let actual = table
        .rows
        .first()
        .and_then(|r| r.first())
        .ok_or("预测没有结果")?
        .display();
    Ok(PredictionResult {
        correct: p.choices.get(choice) == Some(&actual),
        table,
        explanation: p.explanation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_question_has_distinct_three_level_teaching() {
        let q = crate::exercises::catalog(0);
        assert_eq!(q.len(), 33);
        for q in q {
            assert_eq!(q.hints.len(), 3, "{}", q.id);
            assert!(!q.knowledge.is_empty());
            assert!(q.explanation.contains("边界"), "{}", q.id);
        }
        assert_eq!(lessons().len(), 6);
        for p in predictions() {
            assert!(
                p.choices
                    .iter()
                    .enumerate()
                    .any(|(i, _)| predict(&p.id, i, &Budget::default()).unwrap().correct),
                "{}",
                p.id
            );
        }
    }
}
