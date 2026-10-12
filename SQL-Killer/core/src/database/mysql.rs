use crate::{model::*, script::split_script};
use base64::{engine::general_purpose::STANDARD, Engine};
use mysql_async::{consts::ColumnType, prelude::Queryable, Conn, OptsBuilder, Value};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Serialize, Deserialize)]
pub struct MysqlConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub database: String,
    #[serde(default)]
    pub tls: bool,
}
impl MysqlConfig {
    pub fn redacted(&self) -> Self {
        Self {
            password: String::new(),
            ..self.clone()
        }
    }
}
pub async fn connect(cfg: &MysqlConfig) -> Result<Conn, String> {
    let opts = OptsBuilder::default()
        .ip_or_hostname(cfg.host.clone())
        .tcp_port(cfg.port)
        .user(Some(cfg.user.clone()))
        .pass(Some(cfg.password.clone()))
        .db_name(if cfg.database.is_empty() {
            None
        } else {
            Some(cfg.database.clone())
        })
        .ssl_opts(if cfg.tls {
            Some(mysql_async::SslOpts::default())
        } else {
            None
        });
    let mut c = tokio::time::timeout(Duration::from_secs(10), Conn::new(opts))
        .await
        .map_err(|_| "MySQL 连接超时".to_string())?
        .map_err(failure)?;
    c.query_drop("SET NAMES utf8mb4").await.map_err(failure)?;
    c.query_drop("SET SESSION innodb_lock_wait_timeout=8")
        .await
        .map_err(failure)?;
    Ok(c)
}
fn value(v: Value, column: &mysql_async::Column) -> Cell {
    let ty = column.column_type();
    match v {
        Value::NULL => Cell::Null,
        Value::Int(n) => Cell::Integer(n.to_string()),
        Value::UInt(n) => Cell::Unsigned(n.to_string()),
        Value::Float(n) => Cell::Real(n as f64),
        Value::Double(n) => Cell::Real(n),
        Value::Bytes(b) => {
            let s = String::from_utf8_lossy(&b).into_owned();
            match ty {
                ColumnType::MYSQL_TYPE_TINY
                | ColumnType::MYSQL_TYPE_SHORT
                | ColumnType::MYSQL_TYPE_LONG
                | ColumnType::MYSQL_TYPE_LONGLONG
                | ColumnType::MYSQL_TYPE_INT24
                | ColumnType::MYSQL_TYPE_YEAR => {
                    return if column
                        .flags()
                        .contains(mysql_async::consts::ColumnFlags::UNSIGNED_FLAG)
                    {
                        Cell::Unsigned(s)
                    } else {
                        Cell::Integer(s)
                    }
                }
                ColumnType::MYSQL_TYPE_FLOAT | ColumnType::MYSQL_TYPE_DOUBLE => {
                    return s.parse::<f64>().map(Cell::Real).unwrap_or(Cell::Text(s))
                }
                ColumnType::MYSQL_TYPE_DATE
                | ColumnType::MYSQL_TYPE_DATETIME
                | ColumnType::MYSQL_TYPE_TIMESTAMP => return Cell::Date(s),
                ColumnType::MYSQL_TYPE_TIME => return Cell::Time(s),
                ColumnType::MYSQL_TYPE_BIT => return Cell::Binary(STANDARD.encode(b)),
                _ => {}
            }
            if column.character_set() == 63
                && matches!(
                    ty,
                    ColumnType::MYSQL_TYPE_BLOB
                        | ColumnType::MYSQL_TYPE_LONG_BLOB
                        | ColumnType::MYSQL_TYPE_MEDIUM_BLOB
                        | ColumnType::MYSQL_TYPE_TINY_BLOB
                        | ColumnType::MYSQL_TYPE_STRING
                        | ColumnType::MYSQL_TYPE_VAR_STRING
                )
            {
                return Cell::Binary(STANDARD.encode(b));
            }
            let text = String::from_utf8(b.clone());
            if matches!(
                ty,
                ColumnType::MYSQL_TYPE_DECIMAL | ColumnType::MYSQL_TYPE_NEWDECIMAL
            ) {
                Cell::Decimal(String::from_utf8_lossy(&b).into_owned())
            } else {
                match text {
                    Ok(t) => Cell::Text(t),
                    Err(_) => Cell::Binary(STANDARD.encode(b)),
                }
            }
        }
        Value::Date(y, m, d, h, min, s, us) => Cell::Date(format!(
            "{y:04}-{m:02}-{d:02} {h:02}:{min:02}:{s:02}.{us:06}"
        )),
        Value::Time(neg, d, h, m, s, us) => Cell::Time(format!(
            "{}{hours}:{m:02}:{s:02}.{us:06}",
            if neg { "-" } else { "" },
            hours = d * 24 + u32::from(h)
        )),
    }
}
pub async fn query(c: &mut Conn, sql: &str, budget: &Budget) -> Result<Table, String> {
    let mut all = statement(c, sql, budget).await?;
    Ok(all.pop().map(|s| s.0).unwrap_or_default())
}
pub async fn statement(
    c: &mut Conn,
    sql: &str,
    budget: &Budget,
) -> Result<Vec<(Table, u64)>, String> {
    budget.check()?;
    let mut result = c.query_iter(sql).await.map_err(failure)?;
    let mut sets = Vec::new();
    loop {
        budget.check()?;
        let columns = result.columns_ref().to_vec();
        let affected = result.affected_rows();
        let mut table = Table {
            columns: columns.iter().map(|c| c.name_str().into_owned()).collect(),
            ..Table::default()
        };
        let mut bytes = 0;
        while let Some(row) = result.next().await.map_err(failure)? {
            budget.check()?;
            let cells = row
                .unwrap()
                .into_iter()
                .zip(columns.iter())
                .map(|(v, c)| value(v, c))
                .collect::<Vec<_>>();
            bytes += serde_json::to_vec(&cells).map_err(failure)?.len();
            if table.rows.len() < budget.max_rows && bytes <= budget.max_bytes {
                table.rows.push(cells)
            } else {
                table.truncated = true;
            }
        }
        sets.push((table, affected));
        if result.is_empty() {
            break;
        }
    }
    result.drop_result().await.map_err(failure)?;
    Ok(sets)
}
pub async fn execute(c: &mut Conn, source: &str, budget: &Budget) -> Execution {
    let now = Instant::now();
    let mut results = Vec::new();
    let (mut error, mut error_range) = (None, None);
    let mut status = "completed".to_string();
    for stmt in split_script(source) {
        let started = Instant::now();
        match statement(c, &stmt.sql, budget).await {
            Ok(sets) => {
                for (table, affected) in sets {
                    results.push(StatementResult {
                        sql: stmt.sql.clone(),
                        table,
                        affected,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                        range: stmt.range.clone(),
                    });
                }
            }
            Err(e) => {
                status = if budget.check().is_err() {
                    "limit"
                } else {
                    "error"
                }
                .into();
                error = Some(e);
                error_range = Some(stmt.range);
                break;
            }
        }
    }
    let transaction = match c.last_ok_packet() {
        Some(packet)
            if packet
                .status_flags()
                .contains(mysql_async::consts::StatusFlags::SERVER_STATUS_IN_TRANS) =>
        {
            "事务进行中"
        }
        Some(_) => "无活动事务（以服务器状态标志为准）",
        None => "尚无服务器事务状态",
    }
    .into();
    Execution {
        results,
        status,
        error,
        error_range,
        transaction,
        elapsed_ms: now.elapsed().as_millis() as u64,
    }
}
pub async fn cancel(cfg: &MysqlConfig, id: u32) -> Result<(), String> {
    let mut c = connect(cfg).await?;
    c.query_drop(format!("KILL QUERY {id}"))
        .await
        .map_err(failure)?;
    c.disconnect().await.map_err(failure)
}
pub async fn schema(c: &mut Conn) -> Result<Schema, String> {
    let mut s = Schema {
        engine: c
            .query_first::<String, _>("SELECT CONCAT('MySQL ',VERSION())")
            .await
            .map_err(failure)?
            .unwrap_or_default(),
        ..Schema::default()
    };
    let tables=c.query::<(String,String),_>("SELECT TABLE_NAME,TABLE_TYPE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() ORDER BY TABLE_NAME").await.map_err(failure)?;
    for (name, kind) in tables {
        let mut r = Relation {
            name: name.clone(),
            kind,
            ..Relation::default()
        };
        let cols=c.exec::<(String,String,String,String,Option<String>),_,_>("SELECT COLUMN_NAME,COLUMN_TYPE,IS_NULLABLE,COLUMN_KEY,COLUMN_DEFAULT FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME=? ORDER BY ORDINAL_POSITION",(&name,)).await.map_err(failure)?;
        r.columns = cols
            .into_iter()
            .map(|(name, data_type, n, k, default)| Column {
                name,
                data_type,
                nullable: n == "YES",
                primary_key: k == "PRI",
                default,
            })
            .collect();
        let keys=c.exec::<(String,String,String,String),_,_>("SELECT CONSTRAINT_NAME,COLUMN_NAME,REFERENCED_TABLE_NAME,REFERENCED_COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME=? AND REFERENCED_TABLE_NAME IS NOT NULL ORDER BY CONSTRAINT_NAME,ORDINAL_POSITION",(&name,)).await.map_err(failure)?;
        let mut groups = std::collections::BTreeMap::<String, ForeignKey>::new();
        for (id, col, target, tcol) in keys {
            let k = groups.entry(id).or_insert(ForeignKey {
                columns: vec![],
                target,
                target_columns: vec![],
            });
            k.columns.push(col);
            k.target_columns.push(tcol);
        }
        r.foreign_keys = groups.into_values().collect();
        r.indexes=c.exec("SELECT DISTINCT INDEX_NAME FROM information_schema.STATISTICS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME=?",(&name,)).await.map_err(failure)?;
        let create = c
            .query_first::<(String, String), _>(format!(
                "SHOW CREATE TABLE {}",
                mysql_identifier(&name)
            ))
            .await
            .map_err(failure)?;
        r.definition = create.map(|(_, v)| v).unwrap_or_default();
        s.relations.push(r);
    }
    s.routines=c.query("SELECT CONCAT(ROUTINE_TYPE,' ',ROUTINE_NAME) FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA=DATABASE() UNION ALL SELECT CONCAT('TRIGGER ',TRIGGER_NAME) FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA=DATABASE()").await.map_err(failure)?;
    Ok(s)
}
pub fn mysql_seed(name: &str) -> Result<String, String> {
    let sql = super::preset(name)?;
    use sqlparser::{
        ast::{ColumnOption, Statement, TableConstraint},
        dialect::SQLiteDialect,
        parser::Parser,
    };
    let mut statements = Parser::parse_sql(&SQLiteDialect {}, sql).map_err(failure)?;
    for statement in &mut statements {
        if let Statement::CreateTable(table) = statement {
            for column in &mut table.columns {
                let mut foreign = Vec::new();
                column.options.retain(|option| {
                    if let ColumnOption::ForeignKey(fk) = &option.option {
                        let mut fk = fk.clone();
                        fk.columns = vec![column.name.clone()];
                        foreign.push(TableConstraint::ForeignKey(fk));
                        false
                    } else {
                        true
                    }
                });
                table.constraints.extend(foreign);
            }
        }
    }
    Ok(statements
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(";\n")
        .replace("TEXT", "VARCHAR(255)")
        .replace("REAL", "DOUBLE"))
}

/// Fresh owned schemas are marked in application metadata; never drop an arbitrary user database.
pub async fn create_sandbox(c: &mut Conn, name: &str, preset: &str) -> Result<(), String> {
    if !name.starts_with("sqlkiller_")
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("无效教学沙箱名称".into());
    }
    c.query_drop(format!(
        "CREATE DATABASE {} CHARACTER SET utf8mb4 COLLATE utf8mb4_bin",
        mysql_identifier(name)
    ))
    .await
    .map_err(failure)?;
    c.query_drop(format!("USE {}", mysql_identifier(name)))
        .await
        .map_err(failure)?;
    for s in split_script(&mysql_seed(preset)?) {
        c.query_drop(s.sql).await.map_err(failure)?
    }
    Ok(())
}
