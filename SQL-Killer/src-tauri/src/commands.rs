use crate::state::AppState;
use serde_json::{json, Value};
use sql_killer_core::{
    algebra, database, exercises, language, model::*, mysql_async::prelude::Queryable, project,
    schedule, trace,
};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or("")
}
fn decode<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(failure)
}
fn encode<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(failure)
}
#[tauri::command]
pub async fn rpc(
    state: tauri::State<'_, Arc<AppState>>,
    command: String,
    args: Value,
) -> Result<Value, String> {
    let state = state.inner().clone();
    if command == "cancel" {
        if let Some(b) = state.jobs.lock().unwrap().get(text(&args, "job_id")) {
            b.cancelled.store(true, Ordering::SeqCst);
        }
        return Ok(json!(true));
    }
    if command == "invalidate" {
        state.invalidate();
        return Ok(json!(true));
    }
    let id = text(&args, "job_id").to_string();
    let budget = Budget::default();
    if !id.is_empty() {
        let mut jobs = state.jobs.lock().unwrap();
        if jobs.contains_key(&id) {
            return Err("任务 ID 重复".into());
        }
        jobs.insert(id.clone(), budget.clone());
    }
    let generation = state.generation.load(Ordering::SeqCst);
    let result = dispatch(state.clone(), &command, args, budget).await;
    if !id.is_empty() {
        state.jobs.lock().unwrap().remove(&id);
        if generation != state.generation.load(Ordering::SeqCst) {
            return Err("文档已切换，旧任务结果已丢弃".into());
        }
    }
    result
}
async fn dispatch(
    state: Arc<AppState>,
    command: &str,
    args: Value,
    budget: Budget,
) -> Result<Value, String> {
    let engine = text(&args, "engine");
    let session = if text(&args, "session").is_empty() {
        "main"
    } else {
        text(&args, "session")
    };
    match command {
        "case_repair_mysql" => {
            sql_killer_core::cases::check_context(
                text(&args, "case_id"),
                args["version"].as_u64().unwrap_or(0) as u32,
            )?;
            let cfg = state
                .configs
                .lock()
                .unwrap()
                .get("main")
                .cloned()
                .ok_or("请先连接 MySQL；验证将新建并清理专用临时数据库")?;
            encode(
                sql_killer_core::cases_mysql::validate(&cfg, text(&args, "strategy"), &budget)
                    .await?,
            )
        }
        "mysql_connect" => {
            let cfg: database::mysql::MysqlConfig = decode(&args["config"])?;
            let conn = database::mysql::connect(&cfg).await?;
            let id = conn.id();
            if args["remember"].as_bool() == Some(true) {
                keyring::Entry::new(
                    "SQL-Killer",
                    &format!("{}:{}:{}", cfg.host, cfg.port, cfg.user),
                )
                .map_err(failure)?
                .set_password(&cfg.password)
                .map_err(failure)?;
            }
            state.configs.lock().unwrap().insert(session.into(), cfg);
            state.mysql.lock().await.insert(
                session.into(),
                Arc::new(tokio::sync::Mutex::new(Some(conn))),
            );
            Ok(json!({"connection_id":id,"session":session}))
        }
        "credential" => {
            let cfg: database::mysql::MysqlConfig = decode(&args["config"])?;
            encode(
                keyring::Entry::new(
                    "SQL-Killer",
                    &format!("{}:{}:{}", cfg.host, cfg.port, cfg.user),
                )
                .map_err(failure)?
                .get_password()
                .unwrap_or_default(),
            )
        }
        "mysql_sandbox" => {
            let cfg = state
                .configs
                .lock()
                .unwrap()
                .get(session)
                .cloned()
                .ok_or("请先连接 MySQL")?;
            let name = format!(
                "sqlkiller_lab_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            let mut c = database::mysql::connect(&cfg).await?;
            database::mysql::create_sandbox(&mut c, &name, text(&args, "fixture")).await?;
            let mut new = cfg.clone();
            new.database = name.clone();
            state
                .configs
                .lock()
                .unwrap()
                .insert(session.into(), new.clone());
            state
                .mysql
                .lock()
                .await
                .insert(session.into(), Arc::new(tokio::sync::Mutex::new(Some(c))));
            {
                let mut owned = state.owned.lock().unwrap();
                owned.push((cfg.host, cfg.port, name));
                std::fs::write(
                    state.directory.join("owned.json"),
                    serde_json::to_vec(&*owned).map_err(failure)?,
                )
                .map_err(failure)?;
            }
            encode(new.redacted())
        }
        "execute" | "schema" | "explain" if engine == "mysql" => {
            let cfg = state
                .configs
                .lock()
                .unwrap()
                .get(session)
                .cloned()
                .ok_or("该会话尚未连接 MySQL")?;
            let handle = state
                .mysql
                .lock()
                .await
                .get(session)
                .cloned()
                .ok_or("MySQL 会话不存在")?;
            let mut guard = handle.lock().await;
            let conn = guard.as_mut().ok_or("连接已关闭，请重新连接")?;
            let id = conn.id();
            if command == "schema" {
                return encode(database::mysql::schema(conn).await?);
            }
            let sql = if command == "explain" {
                trace::plan(text(&args, "sql"), "mysql")?;
                format!("EXPLAIN FORMAT=JSON {}", text(&args, "sql"))
            } else {
                text(&args, "sql").to_string()
            };
            let result = {
                let run = database::mysql::execute(conn, &sql, &budget);
                tokio::pin!(run);
                loop {
                    tokio::select! {r=&mut run=>break Ok(r),_=tokio::time::sleep(std::time::Duration::from_millis(50))=>{if let Err(e)=budget.check(){let cancel=database::mysql::cancel(&cfg,id).await;break Err(format!("{e}；{}。写入可能已经提交，请核对事务状态后重连。",if cancel.is_ok(){"已请求服务器终止查询"}else{"连接状态无法确认"}));}}}
                }
            };
            if result.is_err() {
                guard.take();
            }
            result.and_then(encode)
        }
        "trace" if engine == "mysql" => {
            let cfg = state
                .configs
                .lock()
                .unwrap()
                .get(session)
                .cloned()
                .ok_or("请先连接 MySQL")?;
            encode(trace::trace_mysql(&cfg, text(&args, "sql"), &budget).await)
        }
        "grade" if engine == "mysql" => {
            let cfg = state
                .configs
                .lock()
                .unwrap()
                .get(session)
                .cloned()
                .ok_or("请先连接 MySQL")?;
            encode(
                exercises::grade_mysql(
                    &cfg,
                    text(&args, "id"),
                    text(&args, "answer"),
                    args["seed"].as_u64().unwrap_or(0),
                    &budget,
                )
                .await,
            )
        }
        "mysql_reset" => {
            let cfg = state
                .configs
                .lock()
                .unwrap()
                .get(session)
                .cloned()
                .ok_or("请先连接 MySQL")?;
            if !state.owned.lock().unwrap().contains(&(
                cfg.host.clone(),
                cfg.port,
                cfg.database.clone(),
            )) {
                return Err("只允许重置 SQL-Killer 创建的教学沙箱".into());
            }
            state.invalidate();
            let sessions = state
                .configs
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, c)| {
                    c.host == cfg.host && c.port == cfg.port && c.database == cfg.database
                })
                .map(|(s, _)| s.clone())
                .collect::<Vec<_>>();
            for s in &sessions {
                if let Some(handle) = state.mysql.lock().await.remove(s) {
                    if let Some(c) = handle.lock().await.take() {
                        let _ =
                            tokio::time::timeout(std::time::Duration::from_secs(3), c.disconnect())
                                .await;
                    }
                }
            }
            let mut c = database::mysql::connect(&cfg).await?;
            c.query_drop(format!("DROP DATABASE {}", mysql_identifier(&cfg.database)))
                .await
                .map_err(failure)?;
            database::mysql::create_sandbox(&mut c, &cfg.database, text(&args, "fixture")).await?;
            let _ = c.disconnect().await;
            for s in sessions {
                let c = database::mysql::connect(&cfg).await?;
                state
                    .mysql
                    .lock()
                    .await
                    .insert(s, Arc::new(tokio::sync::Mutex::new(Some(c))));
            }
            Ok(json!(true))
        }
        _ => {
            let cmd = command.to_string();
            tokio::task::spawn_blocking(move || sync_dispatch(&state, &cmd, &args, &budget))
                .await
                .map_err(failure)?
        }
    }
}
fn sync_dispatch(
    state: &AppState,
    command: &str,
    args: &Value,
    b: &Budget,
) -> Result<Value, String> {
    match command {
        "resolve_reference" => encode(language::resolve_reference(
            text(args, "sql"),
            args["cursor"].as_u64().unwrap_or(0) as usize,
            &decode(&args["schema"])?,
            text(args, "engine"),
            text(args, "relation"),
            text(args, "column"),
        )),
        "predict" => encode(sql_killer_core::learning::predict(
            text(args, "id"),
            args["choice"].as_u64().unwrap_or(usize::MAX as u64) as usize,
            b,
        )?),
        "compare_queries" => encode(sql_killer_core::cases::compare_queries(
            &state.db.lock().unwrap(),
            text(args, "left"),
            text(args, "right"),
            b,
        )?),
        "case_catalog" => encode(sql_killer_core::cases::spec()),
        "case_schema" | "case_query" | "case_trace" | "case_grade" | "case_reproduction"
        | "case_repair" | "case_close" => {
            use sql_killer_core::cases;
            cases::check_context(
                text(args, "case_id"),
                args["version"].as_u64().unwrap_or(0) as u32,
            )?;
            let seed = args["seed"].as_u64().unwrap_or(0);
            match command {
                "case_schema" => encode(database::schema(&cases::fixture(seed, 0)?)?),
                "case_query" => encode(
                    json!({"table":cases::query(&cases::fixture(seed,0)?,text(args,"sql"),b)?,"snapshot":cases::snapshot(seed)}),
                ),
                "case_trace" => {
                    let c = cases::fixture(seed, 0)?;
                    cases::query(&c, text(args, "sql"), b)?;
                    encode(trace::trace_query(&c, text(args, "sql"), b))
                }
                "case_grade" => {
                    encode(cases::grade(text(args, "task"), text(args, "sql"), seed, b))
                }
                "case_reproduction" => {
                    let (source, simulation) = cases::reproduction(b)?;
                    encode(json!({"source":source,"simulation":simulation}))
                }
                "case_repair" => encode(cases::validate_repair(text(args, "strategy"), b)?),
                "case_close" => encode(cases::close_case(&decode(&args["progress"])?, b)?),
                _ => unreachable!(),
            }
        }
        "case_templates" => {
            sql_killer_core::cases::check_context(
                text(args, "case_id"),
                args["version"].as_u64().unwrap_or(0) as u32,
            )?;
            encode(include_str!("../../curriculum/case-repair.mysql.sql"))
        }
        "fixture_preview" => {
            let c = exercises::fixture(text(args, "fixture"), 0)?;
            encode(database::query(
                &c,
                &format!(
                    "SELECT * FROM {} LIMIT 100",
                    quote_identifier(text(args, "relation"))
                ),
                b,
            )?)
        }
        "fixture_schema" => encode(database::schema(&database::memory(text(args, "fixture"))?)?),
        "exercise_trace" => {
            let q = exercises::catalog(args["seed"].as_u64().unwrap_or(0))
                .into_iter()
                .find(|q| q.id == text(args, "id"))
                .ok_or("题目不存在")?;
            let c = exercises::fixture(&q.fixture, 0)?;
            match q.kind.as_str() {
                "query" => encode(
                    json!({"kind":"trace","trace":trace::trace_query(&c,text(args,"answer"),b)}),
                ),
                "algebra" => encode(
                    json!({"kind":"trace","trace":algebra::evaluate_algebra(&c,text(args,"answer"),false,b)?.trace}),
                ),
                "schedule" => encode(
                    json!({"kind":"schedule","source":"T1 read A\nT2 write A 200\nT1 write B 300\nT2 read B","simulation":schedule::simulate_schedule("T1 read A\nT2 write A 200\nT1 write B 300\nT2 read B",std::collections::BTreeMap::from([("A".into(),300),("B".into(),300)]),"raw","ru",b)?}),
                ),
                _ => Err("本题没有公开查询过程；请使用试运行查看真实结果".into()),
            }
        }
        "exercise_run" => {
            let q = exercises::catalog(args["seed"].as_u64().unwrap_or(0))
                .into_iter()
                .find(|q| q.id == text(args, "id"))
                .ok_or("题目不存在")?;
            if q.engine == "mysql" {
                return Err("MySQL 例程请提交判题，在独立沙箱中运行全部调用探针".into());
            }
            encode(database::execute(
                &exercises::fixture(&q.fixture, 0)?,
                text(args, "answer"),
                b,
            ))
        }
        "bootstrap" => {
            let workspace = state.workspace.lock().unwrap().clone();
            let questions = exercises::catalog(workspace.exercise_seed);
            Ok(
                json!({"workspace":workspace,"schema":database::schema(&state.db.lock().unwrap())?,"questions":questions,"recovery":state.directory.join("recovery.sqlkproj").exists(),"data_directory":state.directory,"version":env!("CARGO_PKG_VERSION"),"lessons":sql_killer_core::learning::lessons(),"predictions":sql_killer_core::learning::predictions()}),
            )
        }
        "execute" => encode(database::execute(
            &state.db.lock().unwrap(),
            text(args, "sql"),
            b,
        )),
        "schema" => encode(database::schema(&state.db.lock().unwrap())?),
        "analyze" => {
            let schema: Schema = decode(&args["schema"])?;
            encode(language::analyze_sql(
                text(args, "sql"),
                text(args, "engine"),
                &schema,
            ))
        }
        "complete" => {
            let schema: Schema = decode(&args["schema"])?;
            encode(language::complete_sql(
                text(args, "sql"),
                args["cursor"].as_u64().unwrap_or(0) as usize,
                &schema,
            ))
        }
        "statement" => {
            let sql = text(args, "sql");
            let cursor = args["cursor"].as_u64().unwrap_or(0) as usize;
            let statement = sql_killer_core::script::split_script(sql)
                .into_iter()
                .find(|s| s.range.start <= cursor && s.range.end >= cursor)
                .or_else(|| {
                    sql_killer_core::script::split_script(sql)
                        .into_iter()
                        .find(|s| s.range.start >= cursor)
                });
            encode(statement.map(|s| json!({"sql":s.sql,"range":s.range})))
        }
        "trace" => encode(trace::trace_query(
            &state.db.lock().unwrap(),
            text(args, "sql"),
            b,
        )),
        "explain" => {
            trace::plan(text(args, "sql"), "sqlite")?;
            encode(database::query(
                &state.db.lock().unwrap(),
                &format!("EXPLAIN QUERY PLAN {}", text(args, "sql")),
                b,
            )?)
        }
        "algebra" => encode(algebra::evaluate_algebra(
            &state.db.lock().unwrap(),
            text(args, "expression"),
            args["optimize"].as_bool().unwrap_or(true),
            b,
        )?),
        "simulate" => encode(schedule::simulate_schedule(
            text(args, "source"),
            decode(&args["initial"])?,
            text(args, "mode"),
            text(args, "isolation"),
            b,
        )?),
        "analyze_schedule" => encode(schedule::analyze_schedule(text(args, "source"), true, b)?),
        "catalog" => encode(exercises::catalog(args["seed"].as_u64().unwrap_or(0))),
        "grade" => encode(exercises::grade(
            text(args, "id"),
            text(args, "answer"),
            args["seed"].as_u64().unwrap_or(0),
            b,
        )),
        "reset" => {
            state.invalidate();
            let fixture = text(args, "fixture");
            let new = database::memory(fixture)?;
            *state.db.lock().unwrap() = new;
            state.workspace.lock().unwrap().fixture = fixture.into();
            encode(database::schema(&state.db.lock().unwrap())?)
        }
        "save_project" | "autosave" => {
            let workspace: project::Workspace = decode(&args["workspace"])?;
            let path = if command == "autosave" {
                state.directory.join("recovery.sqlkproj")
            } else {
                PathBuf::from(text(args, "path"))
            };
            project::save(&path, &workspace, &state.db.lock().unwrap())?;
            *state.workspace.lock().unwrap() = workspace;
            Ok(json!(true))
        }
        "load_project" | "recover" => {
            let path = if command == "recover" {
                state.directory.join("recovery.sqlkproj")
            } else {
                PathBuf::from(text(args, "path"))
            };
            let (workspace, db) = project::load(&path, &state.directory)?;
            state.invalidate();
            *state.db.lock().unwrap() = db;
            *state.workspace.lock().unwrap() = workspace.clone();
            Ok(json!({"workspace":workspace,"schema":database::schema(&state.db.lock().unwrap())?}))
        }
        "new_project" => {
            state.invalidate();
            *state.db.lock().unwrap() = database::memory("student")?;
            let workspace = project::Workspace::default();
            *state.workspace.lock().unwrap() = workspace.clone();
            encode(workspace)
        }
        "read_sql" => {
            let bytes = std::fs::read(text(args, "path")).map_err(failure)?;
            if bytes.len() > 2 * 1024 * 1024 {
                return Err("SQL 文档超过 2 MiB".into());
            }
            encode(String::from_utf8(bytes).map_err(failure)?)
        }
        "export" => {
            let path = PathBuf::from(text(args, "path"));
            if args.get("table").is_some() {
                let table: Table = decode(&args["table"])?;
                let data = if text(args, "format") == "csv" {
                    project::csv(&table)
                } else {
                    serde_json::to_string_pretty(&table).map_err(failure)?
                };
                std::fs::write(path, data).map_err(failure)?;
            } else if let Some(data) = args["bytes"].as_array() {
                if data.len() > 100 * 1024 * 1024 {
                    return Err("导出内容过大".into());
                }
                let bytes = data
                    .iter()
                    .map(|x| x.as_u64().unwrap_or(0) as u8)
                    .collect::<Vec<_>>();
                std::fs::write(path, bytes).map_err(failure)?;
            } else {
                std::fs::write(path, text(args, "content")).map_err(failure)?;
            }
            Ok(json!(true))
        }
        _ => Err(format!("未知命令：{command}")),
    }
}
