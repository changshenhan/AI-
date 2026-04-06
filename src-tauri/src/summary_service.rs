//! 日/周总结：幂等 + 工具快照 + LLM；触发弹窗事件。

use crate::db::Db;
use crate::llm::{llm_complete, load_llm_settings_or_err, ChatMessage, LlmSettings};
use crate::metrics::{previous_week_monday, recompute_weekly_rollup};
use crate::tools::{snapshot_for_daily_summary, snapshot_for_weekly_summary};
use chrono::{Datelike, Duration, Local};
use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Clone, Serialize)]
pub struct SummaryEventPayload {
    pub kind: String,
    pub period_key: String,
    pub text: String,
    pub trigger: String,
}

pub fn summary_exists(conn: &Connection, kind: &str, period_key: &str) -> Result<bool, String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM summaries WHERE kind = ?1 AND period_key = ?2",
            params![kind, period_key],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

pub async fn try_daily_summary(
    app: &AppHandle,
    db: &Db,
    day: &str,
    trigger: &str,
) -> Result<(), String> {
    let settings = match load_llm_settings_or_err(app) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if summary_exists(&conn, "daily", day)? {
            return Ok(());
        }
    }

    let snapshot = {
        let c = db.0.lock().map_err(|e| e.to_string())?;
        snapshot_for_daily_summary(&c, day)?
    };

    let prompt = format!(
        "你是日程教练。根据以下**本地工具已算好的量化数据**写一段简洁的中文日总结（Markdown），\
         包含完成率、主要任务、可改进点；勿编造数据中不存在的数字。\n\n{}",
        snapshot
    );
    let text = run_summary_llm(&settings, &prompt).await?;

    let id = format!("s{}", Uuid::new_v4());
    let now = Local::now().to_rfc3339();
    let text_for_emit = text.clone();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO summaries (id, kind, period_key, model_text, created_at, trigger_kind)
         VALUES (?1, 'daily', ?2, ?3, ?4, ?5)",
        params![id, day, text, now, trigger],
    )
    .map_err(|e| e.to_string())?;
    drop(conn);

    let _ = app.emit(
        "engine/summary",
        SummaryEventPayload {
            kind: "daily".into(),
            period_key: day.into(),
            text: text_for_emit,
            trigger: trigger.into(),
        },
    );
    Ok(())
}

pub async fn try_weekly_summary(
    app: &AppHandle,
    db: &Db,
    week_start: &str,
    trigger: &str,
) -> Result<(), String> {
    let settings = match load_llm_settings_or_err(app) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if summary_exists(&conn, "weekly", week_start)? {
            return Ok(());
        }
    }

    {
        let c = db.0.lock().map_err(|e| e.to_string())?;
        recompute_weekly_rollup(&c, week_start)?;
    }

    let snapshot = {
        let c = db.0.lock().map_err(|e| e.to_string())?;
        snapshot_for_weekly_summary(&c, week_start)?
    };

    let prompt = format!(
        "你是日程分析师。根据以下**周聚合与序列数据**写中文周总结（Markdown），\
         分析趋势、节奏与下周建议；勿编造。\n\n{}",
        snapshot
    );
    let text = run_summary_llm(&settings, &prompt).await?;

    let id = format!("s{}", Uuid::new_v4());
    let now = Local::now().to_rfc3339();
    let text_for_emit = text.clone();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO summaries (id, kind, period_key, model_text, created_at, trigger_kind)
         VALUES (?1, 'weekly', ?2, ?3, ?4, ?5)",
        params![id, week_start, text, now, trigger],
    )
    .map_err(|e| e.to_string())?;
    drop(conn);

    let _ = app.emit(
        "engine/summary",
        SummaryEventPayload {
            kind: "weekly".into(),
            period_key: week_start.into(),
            text: text_for_emit,
            trigger: trigger.into(),
        },
    );
    Ok(())
}

async fn run_summary_llm(settings: &LlmSettings, user_prompt: &str) -> Result<String, String> {
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: user_prompt.into(),
    }];
    llm_complete(settings, messages).await
}

pub fn catchup_summaries(app: &AppHandle, db: &Db) {
    let app = app.clone();
    let db = db.clone();
    tauri::async_runtime::spawn(async move {
        let today = Local::now().date_naive();
        let yesterday = today - Duration::days(1);
        let y = yesterday.format("%Y-%m-%d").to_string();

        let need_daily = {
            let c = match db.0.lock() {
                Ok(x) => x,
                Err(_) => return,
            };
            match summary_exists(&c, "daily", &y) {
                Ok(ex) => !ex,
                Err(_) => false,
            }
        };
        if need_daily {
            let _ = try_daily_summary(&app, &db, &y, "catchup_midnight").await;
        }

        if today.weekday() == chrono::Weekday::Mon {
            let ws = previous_week_monday(today).format("%Y-%m-%d").to_string();
            let need_weekly = {
                let c = match db.0.lock() {
                    Ok(x) => x,
                    Err(_) => return,
                };
                match summary_exists(&c, "weekly", &ws) {
                    Ok(ex) => !ex,
                    Err(_) => false,
                }
            };
            if need_weekly {
                let _ = try_weekly_summary(&app, &db, &ws, "catchup_weekly_monday").await;
            }
        }
    });
}
