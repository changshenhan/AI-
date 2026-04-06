//! SQLite：引擎数据（WAL + 预编译语句在调用处使用）。

use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

#[derive(Clone)]
pub struct Db(pub Arc<Mutex<Connection>>);

pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("engine.sqlite3"))
}

pub fn open_and_migrate(app: &AppHandle) -> Result<Db, String> {
    let path = db_path(app)?;
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA cache_size = -64000;
            ",
        )
        .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(Db(Arc::new(Mutex::new(conn))))
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS busy_blocks (
            id TEXT PRIMARY KEY,
            start_at TEXT NOT NULL,
            end_at TEXT NOT NULL,
            label TEXT,
            source_message_id TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS plan_items (
            id TEXT PRIMARY KEY,
            day TEXT NOT NULL,
            title TEXT NOT NULL,
            start_at TEXT NOT NULL,
            end_at TEXT NOT NULL,
            status TEXT NOT NULL,
            source_conversation_id TEXT
        );
        CREATE TABLE IF NOT EXISTS completions (
            task_id TEXT PRIMARY KEY,
            completed_at TEXT NOT NULL,
            was_on_time INTEGER
        );
        CREATE TABLE IF NOT EXISTS daily_rollups (
            date TEXT PRIMARY KEY,
            planned_count INTEGER NOT NULL,
            done_count INTEGER NOT NULL,
            completion_rate REAL NOT NULL,
            total_planned_minutes INTEGER NOT NULL,
            total_busy_minutes INTEGER NOT NULL,
            first_task_at TEXT,
            last_completion_at TEXT
        );
        CREATE TABLE IF NOT EXISTS weekly_rollups (
            week_start TEXT PRIMARY KEY,
            aggregates TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS summaries (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            period_key TEXT NOT NULL,
            model_text TEXT NOT NULL,
            created_at TEXT NOT NULL,
            trigger_kind TEXT NOT NULL,
            UNIQUE(kind, period_key)
        );
        CREATE TABLE IF NOT EXISTS scheduler_state (
            k TEXT PRIMARY KEY,
            v TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_plan_day ON plan_items(day);
        CREATE INDEX IF NOT EXISTS idx_plan_status ON plan_items(status);
        CREATE INDEX IF NOT EXISTS idx_busy_start ON busy_blocks(start_at);
        CREATE VIEW IF NOT EXISTS v_analytics_snapshot AS
        SELECT
            date AS day,
            planned_count,
            done_count,
            completion_rate,
            total_planned_minutes AS focus_minutes,
            total_busy_minutes AS busy_minutes,
            first_task_at,
            last_completion_at
        FROM daily_rollups;
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_scheduler_val(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare_cached("SELECT v FROM scheduler_state WHERE k = ?1")
        .map_err(|e| e.to_string())?;
    stmt.query_row(params![key], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())
}

pub fn set_scheduler_val(conn: &Connection, key: &str, val: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO scheduler_state (k, v) VALUES (?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        params![key, val],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
