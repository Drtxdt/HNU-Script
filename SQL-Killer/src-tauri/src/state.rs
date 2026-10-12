use sql_killer_core::{database, model::Budget, mysql_async, project::Workspace, Connection};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
pub struct AppState {
    pub db: Mutex<Connection>,
    pub workspace: Mutex<Workspace>,
    pub directory: PathBuf,
    pub generation: AtomicU64,
    pub jobs: Mutex<BTreeMap<String, Budget>>,
    pub mysql:
        tokio::sync::Mutex<BTreeMap<String, Arc<tokio::sync::Mutex<Option<mysql_async::Conn>>>>>,
    pub configs: Mutex<BTreeMap<String, database::mysql::MysqlConfig>>,
    pub owned: Mutex<Vec<(String, u16, String)>>,
}
impl AppState {
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let path = directory.join("sandbox.sqlite");
        let exists = path.exists();
        let db = database::open(&path)?;
        if !exists {
            db.execute_batch(database::preset("student")?)
                .map_err(|e| e.to_string())?;
        }
        let owned = std::fs::read(directory.join("owned.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ok(Self {
            db: Mutex::new(db),
            workspace: Mutex::new(Workspace::default()),
            directory,
            generation: AtomicU64::new(0),
            jobs: Mutex::new(BTreeMap::new()),
            mysql: tokio::sync::Mutex::new(BTreeMap::new()),
            configs: Mutex::new(BTreeMap::new()),
            owned: Mutex::new(owned),
        })
    }
    pub fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        for b in self.jobs.lock().unwrap().values() {
            b.cancelled.store(true, Ordering::SeqCst);
        }
    }
}
