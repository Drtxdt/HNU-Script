pub mod mysql;
use crate::{model::*, script::split_script};
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{
    hooks::{AuthAction, AuthContext, Authorization},
    types::ValueRef,
    Connection,
};
use std::{path::Path, time::Instant};

pub fn preset(name: &str) -> Result<&'static str, String> {
    match name {
        "student" => Ok(include_str!("../../curriculum/student.sql")),
        "hotel" => Ok(include_str!("../../curriculum/hotel.sql")),
        "bank" => Ok(include_str!("../../curriculum/bank.sql")),
        "empty" => Ok(""),
        _ => Err("未知示例数据库".into()),
    }
}
pub fn open(path: &Path) -> Result<Connection, String> {
    let c = Connection::open(path).map_err(failure)?;
    configure(&c)?;
    Ok(c)
}
pub fn memory(name: &str) -> Result<Connection, String> {
    let c = Connection::open_in_memory().map_err(failure)?;
    configure(&c)?;
    c.execute_batch(preset(name)?).map_err(failure)?;
    Ok(c)
}
pub fn configure(c: &Connection) -> Result<(), String> {
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=2000;")
        .map_err(failure)?;
    c.authorizer(Some(|ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Attach { .. } | AuthAction::Detach { .. } => Authorization::Deny,
        AuthAction::Function { function_name }
            if matches!(
                function_name.to_ascii_lowercase().as_str(),
                "load_extension" | "readfile" | "writefile"
            ) =>
        {
            Authorization::Deny
        }
        AuthAction::Pragma {
            pragma_name,
            pragma_value: Some(_),
        } if matches!(
            pragma_name.to_ascii_lowercase().as_str(),
            "writable_schema"
                | "foreign_keys"
                | "trusted_schema"
                | "temp_store_directory"
                | "data_store_directory"
        ) =>
        {
            Authorization::Deny
        }
        _ => Authorization::Allow,
    }));
    Ok(())
}
pub fn snapshot(c: &Connection) -> Result<Connection, String> {
    let mut dest = Connection::open_in_memory().map_err(failure)?;
    if !c.is_autocommit() {
        let bytes = c.serialize("main").map_err(failure)?;
        if bytes.len() > 100 * 1024 * 1024 {
            return Err("快照超过 100 MiB 限制".into());
        }
        dest.deserialize_read_exact("main", &bytes[..], bytes.len(), false)
            .map_err(failure)?;
        configure(&dest)?;
        return Ok(dest);
    }
    {
        let backup = rusqlite::backup::Backup::new(c, &mut dest).map_err(failure)?;
        backup
            .run_to_completion(100, std::time::Duration::from_millis(1), None)
            .map_err(failure)?;
    }
    configure(&dest)?;
    Ok(dest)
}
pub fn query(c: &Connection, sql: &str, budget: &Budget) -> Result<Table, String> {
    budget.check()?;
    let b = budget.clone();
    c.progress_handler(1000, Some(move || b.check().is_err()));
    let result = (|| {
        let mut st = c.prepare(sql).map_err(failure)?;
        if !st.readonly() {
            return Err("过程分析只允许只读查询".into());
        }
        let cols = st
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mut rows = st.query([]).map_err(failure)?;
        let mut t = Table {
            columns: cols,
            ..Table::default()
        };
        let mut bytes = 0;
        while let Some(row) = rows
            .next()
            .map_err(|e| budget.check().err().unwrap_or_else(|| e.to_string()))?
        {
            budget.check()?;
            if t.rows.len() >= budget.max_rows {
                t.truncated = true;
                break;
            }
            let mut values = Vec::new();
            for i in 0..t.columns.len() {
                values.push(cell(row.get_ref(i).map_err(failure)?));
            }
            bytes += serde_json::to_vec(&values).map_err(failure)?.len();
            if bytes > budget.max_bytes {
                t.truncated = true;
                break;
            }
            t.rows.push(values);
        }
        Ok(t)
    })();
    c.progress_handler(0, None::<fn() -> bool>);
    result
}
fn cell(v: ValueRef<'_>) -> Cell {
    match v {
        ValueRef::Null => Cell::Null,
        ValueRef::Integer(n) => Cell::Integer(n.to_string()),
        ValueRef::Real(n) => Cell::Real(n),
        ValueRef::Text(s) => Cell::Text(String::from_utf8_lossy(s).into_owned()),
        ValueRef::Blob(b) => Cell::Binary(STANDARD.encode(b)),
    }
}
pub fn execute(c: &Connection, source: &str, budget: &Budget) -> Execution {
    let before_changes = c.total_changes();
    let before_schema = c
        .query_row("PRAGMA schema_version", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0);
    let started = Instant::now();
    let mut results = Vec::new();
    let mut error = None;
    let mut error_range = None;
    let mut status = "completed".to_string();
    for stmt in split_script(source) {
        let at = Instant::now();
        let b = Budget {
            deadline: budget
                .deadline
                .min(Instant::now() + std::time::Duration::from_secs(10)),
            ..budget.clone()
        };
        let hook = b.clone();
        c.progress_handler(1000, Some(move || hook.check().is_err()));
        let mut failure_offset = None;
        let one = (|| -> Result<(Table, u64), String> {
            b.check()?;
            let mut st = c.prepare(&stmt.sql).map_err(|e| {
                if let rusqlite::Error::SqlInputError { offset, .. } = &e {
                    if *offset >= 0 {
                        failure_offset = Some(*offset as usize)
                    }
                }
                e.to_string()
            })?;
            let readonly = st.readonly();
            let columns = st
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            let mut table = Table {
                columns,
                ..Table::default()
            };
            let mut bytes = 0usize;
            if table.columns.is_empty() {
                st.execute([]).map_err(failure)?;
            } else {
                let mut rows = st.query([]).map_err(failure)?;
                while let Some(row) = rows.next().map_err(failure)? {
                    b.check()?;
                    let values = (0..table.columns.len())
                        .map(|i| row.get_ref(i).map(cell))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(failure)?;
                    bytes += serde_json::to_vec(&values).map_err(failure)?.len();
                    if table.rows.len() < b.max_rows && bytes <= b.max_bytes {
                        table.rows.push(values)
                    } else {
                        table.truncated = true;
                        if readonly {
                            break;
                        }
                    }
                }
            }
            Ok((table, if readonly { 0 } else { c.changes() }))
        })();
        c.progress_handler(0, None::<fn() -> bool>);
        match one {
            Ok((table, affected)) => results.push(StatementResult {
                sql: stmt.sql,
                table,
                affected,
                elapsed_ms: at.elapsed().as_millis() as u64,
                range: stmt.range,
            }),
            Err(e) => {
                let msg = b.check().err().unwrap_or(e);
                status = if msg.contains("取消") {
                    "cancelled"
                } else if msg.contains("时间") {
                    "limit"
                } else {
                    "error"
                }
                .into();
                error = Some(msg);
                error_range = Some(if let Some(offset) = failure_offset {
                    let start = (stmt.range.start + offset).min(source.len());
                    let length = source[start..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(0);
                    source_range(source, start, start + length)
                } else {
                    stmt.range
                });
                break;
            }
        }
    }
    Execution {
        modified: c.total_changes() != before_changes
            || c.query_row("PRAGMA schema_version", [], |r| r.get::<_, i64>(0))
                .unwrap_or(0)
                != before_schema,
        results,
        status,
        error,
        error_range,
        transaction: if c.is_autocommit() {
            "自动提交"
        } else {
            "事务进行中"
        }
        .into(),
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}
pub fn schema(c: &Connection) -> Result<Schema, String> {
    let mut result = Schema {
        engine: format!("SQLite {}", rusqlite::version()),
        ..Schema::default()
    };
    let mut stmt=c.prepare("SELECT name,type,coalesce(sql,'') FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' AND type IN ('table','view') ORDER BY name").map_err(failure)?;
    let list = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    for (name, kind, definition) in list {
        let mut r = Relation {
            name: name.clone(),
            kind,
            definition,
            ..Relation::default()
        };
        let mut st = c
            .prepare(&format!("PRAGMA table_xinfo({})", quote_identifier(&name)))
            .map_err(failure)?;
        r.columns = st
            .query_map([], |row| {
                let pk = row.get::<_, i64>(5)? != 0;
                Ok(Column {
                    name: row.get(1)?,
                    data_type: row.get(2)?,
                    nullable: row.get::<_, i64>(3)? == 0 && !pk,
                    primary_key: pk,
                    default: row.get(4)?,
                })
            })
            .map_err(failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?;
        let mut st = c
            .prepare(&format!(
                "PRAGMA foreign_key_list({})",
                quote_identifier(&name)
            ))
            .map_err(failure)?;
        let keys = st
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                ))
            })
            .map_err(failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?;
        let mut groups = std::collections::BTreeMap::<i64, ForeignKey>::new();
        for (id, target, from, to) in keys {
            let fk = groups.entry(id).or_insert(ForeignKey {
                columns: vec![],
                target,
                target_columns: vec![],
            });
            fk.columns.push(from);
            fk.target_columns.push(to);
        }
        r.foreign_keys = groups.into_values().collect();
        let mut st = c
            .prepare("SELECT name FROM sqlite_schema WHERE type='index' AND tbl_name=?1")
            .map_err(failure)?;
        r.indexes = st
            .query_map([&name], |row| row.get(0))
            .map_err(failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?;
        result.relations.push(r);
    }
    let mut st = c
        .prepare("SELECT name||': '||coalesce(sql,'') FROM sqlite_schema WHERE type='trigger'")
        .map_err(failure)?;
    result.routines = st
        .query_map([], |r| r.get(0))
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    result.version = c
        .query_row("PRAGMA schema_version", [], |r| r.get(0))
        .map_err(failure)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn execution_and_snapshot() {
        let c = memory("student").unwrap();
        assert_eq!(schema(&c).unwrap().relations.len(), 5);
        let r=execute(&c,"BEGIN; UPDATE Student SET Sage=20; ROLLBACK; SELECT COUNT(*) FROM Student WHERE Sage=20;",&Budget::default());
        assert_eq!(r.status, "completed");
        assert_eq!(
            r.results.last().unwrap().table.rows[0][0],
            Cell::Integer("0".into())
        );
        assert_eq!(
            query(
                &snapshot(&c).unwrap(),
                "SELECT * FROM Student",
                &Budget::default()
            )
            .unwrap()
            .rows
            .len(),
            5
        );
    }
    #[test]
    fn preserves_types_and_limit() {
        let c = memory("empty").unwrap();
        let r = query(
            &c,
            "SELECT 9223372036854775807, NULL, '', x'ff', 1.5",
            &Budget::default(),
        )
        .unwrap();
        assert_eq!(r.rows[0][0], Cell::Integer("9223372036854775807".into()));
        let b = Budget {
            max_rows: 2,
            ..Budget::default()
        };
        assert!(query(&c,"WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<5) SELECT * FROM n",&b).unwrap().truncated);
    }
    #[test]
    fn foreign_key_and_attach() {
        let c = memory("student").unwrap();
        assert_eq!(
            execute(
                &c,
                "INSERT INTO SC VALUES('absent','1',1)",
                &Budget::default()
            )
            .status,
            "error"
        );
        assert_eq!(
            execute(&c, "ATTACH ':memory:' AS x", &Budget::default()).status,
            "error"
        );
    }
}
