use crate::{database, model::failure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Document {
    pub id: String,
    pub title: String,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/generated/")]
pub struct Workspace {
    pub format_version: u32,
    pub documents: Vec<Document>,
    pub active_document: String,
    pub fixture: String,
    pub algebra: String,
    pub schedule: String,
    pub initial: BTreeMap<String, i64>,
    pub exercise_answers: BTreeMap<String, String>,
    pub exercise_seed: u64,
    pub progress: BTreeMap<String, String>,
    pub mistakes: Vec<String>,
    pub view: serde_json::Value,
    pub mysql_profile: serde_json::Value,
}
impl Default for Workspace {
    fn default() -> Self {
        Self{format_version:1,documents:vec![Document{id:"main".into(),title:"课堂查询.sql".into(),text:"SELECT S.Sname, C.Cno, C.Grade\nFROM Student S\nLEFT JOIN SC C ON S.Sno = C.Sno\nORDER BY S.Sno, C.Cno;".into()}],active_document:"main".into(),fixture:"student".into(),algebra:"project{Sno,Sname}(select{Sage >= 18}(Student))".into(),schedule:"T1 read A\nT2 read A\nT1 set A A-100\nT1 write A\nT1 commit\nT2 set A A-200\nT2 write A\nT2 commit".into(),initial:BTreeMap::from([("A".into(),300),("B".into(),300)]),exercise_answers:BTreeMap::new(),exercise_seed:0,progress:BTreeMap::new(),mistakes:vec![],view:serde_json::json!({"theme":"system","page":"ide"}),mysql_profile:serde_json::Value::Null}
    }
}
pub fn validate(p: &Workspace) -> Result<(), String> {
    if p.format_version != 1 {
        return Err(format!(
            "不支持工程版本 {}；当前仅支持版本 1",
            p.format_version
        ));
    }
    if p.documents.is_empty() || p.documents.len() > 64 {
        return Err("工程需要 1–64 个 SQL 文档".into());
    }
    if p.documents
        .iter()
        .any(|d| d.text.len() > 2 * 1024 * 1024 || d.title.len() > 256)
    {
        return Err("工程文档超出大小限制".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for d in &p.documents {
        if !ids.insert(&d.id) {
            return Err("文档 ID 重复".into());
        }
    }
    if !ids.contains(&p.active_document) {
        return Err("活动文档不存在".into());
    }
    database::preset(&p.fixture)?;
    if p.algebra.len() > 65536 || p.schedule.len() > 65536 || p.mistakes.len() > 500 {
        return Err("工程实验内容超限".into());
    }
    Ok(())
}
fn scrub(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| {
                !matches!(
                    k.to_lowercase().as_str(),
                    "password" | "pass" | "secret" | "token"
                )
            });
            for value in map.values_mut() {
                scrub(value)
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                scrub(v)
            }
        }
        _ => {}
    }
}
pub fn save(path: &Path, workspace: &Workspace, c: &rusqlite::Connection) -> Result<(), String> {
    validate(workspace)?;
    if !c.is_autocommit() {
        return Err("请先提交或回滚 SQLite 事务再保存工程；工程不能恢复活动事务".into());
    }
    let parent = path.parent().ok_or("缺少保存目录")?;
    std::fs::create_dir_all(parent).map_err(failure)?;
    let db = tempfile::NamedTempFile::new_in(parent).map_err(failure)?;
    c.backup("main", db.path(), None).map_err(failure)?;
    let bytes = std::fs::read(db.path()).map_err(failure)?;
    if bytes.len() > 100 * 1024 * 1024 {
        return Err("教学工程数据库超过 100 MiB".into());
    }
    let mut p = workspace.clone();
    scrub(&mut p.mysql_profile);
    let data = serde_json::to_vec_pretty(&p).map_err(failure)?;
    let mut target = tempfile::NamedTempFile::new_in(parent).map_err(failure)?;
    {
        let mut zip = zip::ZipWriter::new(target.as_file_mut());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", options).map_err(failure)?;
        zip.write_all(&data).map_err(failure)?;
        zip.start_file("database.sqlite", options)
            .map_err(failure)?;
        zip.write_all(&bytes).map_err(failure)?;
        zip.finish().map_err(failure)?;
    }
    target.as_file().sync_all().map_err(failure)?;
    target.persist(path).map_err(failure)?;
    Ok(())
}
pub fn load(path: &Path, directory: &Path) -> Result<(Workspace, rusqlite::Connection), String> {
    let file = std::fs::File::open(path).map_err(failure)?;
    if file.metadata().map_err(failure)?.len() > 110 * 1024 * 1024 {
        return Err("工程文件过大".into());
    }
    let mut zip =
        zip::ZipArchive::new(file).map_err(|_| "工程不是有效的 SQL-Killer 文件".to_string())?;
    if zip.len() != 2 {
        return Err("工程条目数量不正确".into());
    }
    let manifest = {
        let mut f = zip.by_name("manifest.json").map_err(failure)?;
        if f.size() > 8 * 1024 * 1024 {
            return Err("工程元数据过大".into());
        }
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).map_err(failure)?;
        bytes
    };
    let mut p: Workspace =
        serde_json::from_slice(&manifest).map_err(|e| format!("工程元数据损坏：{e}"))?;
    validate(&p)?;
    scrub(&mut p.mysql_profile);
    let db = {
        let mut f = zip.by_name("database.sqlite").map_err(failure)?;
        if f.size() > 100 * 1024 * 1024 {
            return Err("工程数据库过大".into());
        }
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).map_err(failure)?;
        bytes
    };
    std::fs::create_dir_all(directory).map_err(failure)?;
    let mut temp = tempfile::NamedTempFile::new_in(directory).map_err(failure)?;
    temp.write_all(&db).map_err(failure)?;
    let source = database::open(temp.path())?;
    let check: String = source
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(failure)?;
    if check != "ok" {
        return Err("工程数据库完整性检查失败".into());
    }
    let memory = database::snapshot(&source)?;
    Ok((p, memory))
}

pub fn csv(t: &crate::model::Table) -> String {
    fn field(s: &str) -> String {
        format!("\"{}\"", s.replace('"', "\"\""))
    }
    let mut out = "\u{feff}".to_string();
    out += &t
        .columns
        .iter()
        .map(|s| field(s))
        .collect::<Vec<_>>()
        .join(",");
    out += "\r\n";
    for row in &t.rows {
        out += &row
            .iter()
            .map(|v| field(&v.display()))
            .collect::<Vec<_>>()
            .join(",");
        out += "\r\n";
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let base = Path::new("../build/test-project");
        std::fs::create_dir_all(base).unwrap();
        let dir = tempfile::tempdir_in(base).unwrap();
        let path = dir.path().join("中文 工程.sqlkproj");
        let c = database::memory("student").unwrap();
        let mut p = Workspace {
            mysql_profile: serde_json::json!({"host":"localhost","password":"not-exported"}),
            ..Workspace::default()
        };
        save(&path, &p, &c).unwrap();
        let (q, db) = load(&path, dir.path()).unwrap();
        assert!(q.mysql_profile.get("password").is_none());
        assert_eq!(database::schema(&db).unwrap().relations.len(), 5);
        p.format_version = 99;
        assert!(save(&path, &p, &c).is_err());
    }
}
