//! Tauri 命令聚合。

use crate::db::Db;
use crate::llm::{
    llm_complete, load_llm_settings_inner, resolve_llm_settings_for_command, ChatMessage,
    LlmSettings,
};
use crate::metrics::recompute_daily_rollup;
use crate::nlp_layout::apply_natural_language;
use crate::summary_service::try_daily_summary;
use crate::tools::run_tool;
use chrono::{Local, NaiveDate};
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusyBlockDto {
    pub id: String,
    pub start_at: String,
    pub end_at: String,
    pub label: Option<String>,
    pub source_message_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItemDto {
    pub id: String,
    pub day: String,
    pub title: String,
    pub start_at: String,
    pub end_at: String,
    pub status: String,
    pub source_conversation_id: Option<String>,
}

#[tauri::command]
pub fn time_now_iso() -> String {
    Local::now().to_rfc3339()
}

#[tauri::command]
pub fn calendar_list_day(db: tauri::State<Db>, day: String) -> Result<(Vec<BusyBlockDto>, Vec<PlanItemDto>), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut busy = Vec::new();
    {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, start_at, end_at, label, source_message_id FROM busy_blocks WHERE substr(start_at,1,10) = ?1 ORDER BY start_at",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![day], |r| {
                Ok(BusyBlockDto {
                    id: r.get(0)?,
                    start_at: r.get(1)?,
                    end_at: r.get(2)?,
                    label: r.get(3)?,
                    source_message_id: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            busy.push(row.map_err(|e| e.to_string())?);
        }
    }
    let mut plans = Vec::new();
    {
        let mut stmt = conn
            .prepare_cached(
                "SELECT id, day, title, start_at, end_at, status, source_conversation_id FROM plan_items WHERE day = ?1 ORDER BY start_at",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![day], |r| {
                Ok(PlanItemDto {
                    id: r.get(0)?,
                    day: r.get(1)?,
                    title: r.get(2)?,
                    start_at: r.get(3)?,
                    end_at: r.get(4)?,
                    status: r.get(5)?,
                    source_conversation_id: r.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            plans.push(row.map_err(|e| e.to_string())?);
        }
    }
    Ok((busy, plans))
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn nlp_apply_plan(
    app: AppHandle,
    db: tauri::State<'_, Db>,
    settings: LlmSettings,
    day: String,
    userText: String,
    traceId: Option<String>,
) -> Result<crate::nlp_layout::ApplyResult, String> {
    let settings = resolve_llm_settings_for_command(&app, settings)?;
    let db = (*db).clone();
    let tid = traceId.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    apply_natural_language(&db, &settings, &day, &userText, &tid).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayOverview {
    pub day: String,
    pub plan_count: i64,
    pub busy_count: i64,
}

#[tauri::command]
pub fn calendar_month_overview(
    db: tauri::State<Db>,
    year: i32,
    month: u32,
) -> Result<Vec<DayOverview>, String> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| "无效年月".to_string())?;
    let last = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
            .unwrap()
            .pred_opt()
            .unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
            .unwrap()
            .pred_opt()
            .unwrap()
    };
    let start_s = first.format("%Y-%m-%d").to_string();
    let end_s = last.format("%Y-%m-%d").to_string();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut map: HashMap<String, (i64, i64)> = HashMap::new();
    let mut d = first;
    loop {
        let ds = d.format("%Y-%m-%d").to_string();
        map.insert(ds, (0, 0));
        if d >= last {
            break;
        }
        d = d.succ_opt().ok_or_else(|| "日期溢出".to_string())?;
    }
    let mut stmt = conn
        .prepare_cached(
            "SELECT day, COUNT(*) FROM plan_items WHERE day >= ?1 AND day <= ?2 AND status != 'skipped' GROUP BY day",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![start_s, end_s], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (day, c) = row.map_err(|e| e.to_string())?;
        map.entry(day).and_modify(|e| e.0 = c);
    }
    let mut stmt2 = conn
        .prepare_cached(
            "SELECT substr(start_at,1,10) AS d, COUNT(*) FROM busy_blocks \
             WHERE substr(start_at,1,10) >= ?1 AND substr(start_at,1,10) <= ?2 GROUP BY d",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt2
        .query_map(params![start_s, end_s], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (day, c) = row.map_err(|e| e.to_string())?;
        map.entry(day).and_modify(|e| e.1 = c);
    }
    let mut out: Vec<DayOverview> = map
        .into_iter()
        .map(|(day, (plan_count, busy_count))| DayOverview {
            day,
            plan_count,
            busy_count,
        })
        .collect();
    out.sort_by(|a, b| a.day.cmp(&b.day));
    Ok(out)
}

#[tauri::command]
#[allow(non_snake_case)]
pub fn skip_plan_item(db: tauri::State<Db>, taskId: String) -> Result<(), String> {
    let day: String = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let day: String = conn
            .query_row(
                "SELECT day FROM plan_items WHERE id = ?1 AND status = 'pending'",
                params![taskId],
                |r| r.get(0),
            )
            .map_err(|_| "任务不存在或已完成".to_string())?;
        let n = conn
            .execute(
                "UPDATE plan_items SET status = 'skipped' WHERE id = ?1 AND status = 'pending'",
                params![taskId],
            )
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("无法跳过".into());
        }
        day
    };
    {
        let g = db.0.lock().map_err(|e| e.to_string())?;
        recompute_daily_rollup(&*g, &day)?;
    }
    Ok(())
}

#[tauri::command]
pub fn export_summaries_markdown(app: AppHandle, db: tauri::State<Db>) -> Result<String, String> {
    let collected: Vec<(String, String, String, String, String)> = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare_cached(
                "SELECT kind, period_key, model_text, created_at, trigger_kind FROM summaries ORDER BY created_at DESC LIMIT 200",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row.map_err(|e| e.to_string())?);
        }
        v
    };
    let mut md = String::from("# AI 日程引擎 — 总结导出\n\n");
    for (kind, pk, text, _created, trig) in collected {
        md.push_str(&format!("## {} · {} · {}\n\n{}\n\n---\n\n", kind, pk, trig, text));
    }
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("exports");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = format!(
        "summaries_{}.md",
        Local::now().format("%Y%m%d_%H%M%S")
    );
    let path = dir.join(&name);
    fs::write(&path, &md).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn system_timezone() -> String {
    iana_time_zone::get_timezone()
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "unknown".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteResult {
    pub completed_at: String,
    pub was_on_time: bool,
    pub daily_summary_triggered: bool,
    /// 与鼓励文案一致，供前端无需依赖事件即可展示
    pub task_title: String,
    pub feedback_text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackEventPayload {
    pub task_title: String,
    pub text: String,
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn complete_plan_item(app: AppHandle, db: tauri::State<'_, Db>, taskId: String) -> Result<CompleteResult, String> {
    // 先落库：完成状态不应依赖 LLM 是否可用（否则前端无反馈、数据也不变）
    let now = Local::now();
    let now_s = now.to_rfc3339();

    let (day, title, start_at_s, end_at_s, was_on_time, early_min, late_min) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let row: (String, String, String, String) = conn.query_row(
            "SELECT day, title, start_at, end_at FROM plan_items WHERE id = ?1",
            params![taskId],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE plan_items SET status = 'done' WHERE id = ?1",
            params![taskId],
        )
        .map_err(|e| e.to_string())?;
        let end_at = chrono::DateTime::parse_from_rfc3339(&row.3).map_err(|e| e.to_string())?;
        let end_local = end_at.with_timezone(&Local);
        let was_on_time = now <= end_local;
        let early_min: Option<i64> = if now < end_local {
            Some(end_local.signed_duration_since(now).num_minutes().max(0))
        } else {
            None
        };
        let late_min: Option<i64> = if now > end_local {
            Some(now.signed_duration_since(end_local).num_minutes().max(0))
        } else {
            None
        };
        conn.execute(
            "INSERT INTO completions (task_id, completed_at, was_on_time) VALUES (?1, ?2, ?3)
             ON CONFLICT(task_id) DO UPDATE SET completed_at = excluded.completed_at, was_on_time = excluded.was_on_time",
            params![taskId, now_s.clone(), if was_on_time { 1 } else { 0 }],
        )
        .map_err(|e| e.to_string())?;
        (row.0, row.1, row.2, row.3, was_on_time, early_min, late_min)
    };

    {
        let g = db.0.lock().map_err(|e| e.to_string())?;
        recompute_daily_rollup(&*g, &day)?;
    }

    let pending: i64 = {
        let c = db.0.lock().map_err(|e| e.to_string())?;
        c.query_row(
            "SELECT COUNT(*) FROM plan_items WHERE day = ?1 AND status = 'pending'",
            params![day],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
    };

    let mut daily_summary_triggered = false;
    if pending == 0 {
        let dbc = (*db).clone();
        let _ = try_daily_summary(&app, &dbc, &day, "early_complete").await;
        daily_summary_triggered = true;
    }

    let short = match load_llm_settings_inner(&app) {
        Ok(Some(settings)) => {
            let fb = feedback_prompt(
                &title,
                &start_at_s,
                &end_at_s,
                &now_s,
                was_on_time,
                early_min,
                late_min,
            );
            llm_complete(
                &settings,
                vec![ChatMessage {
                    role: "user".into(),
                    content: fb,
                }],
            )
            .await
            .unwrap_or_else(|_| {
                feedback_fallback_local(&title, was_on_time, early_min, late_min)
            })
        }
        Ok(None) | Err(_) => feedback_fallback_local(&title, was_on_time, early_min, late_min),
    };

    let _ = app.emit(
        "engine/feedback",
        FeedbackEventPayload {
            task_title: title.clone(),
            text: short.clone(),
        },
    );

    Ok(CompleteResult {
        completed_at: now_s,
        was_on_time,
        daily_summary_triggered,
        task_title: title,
        feedback_text: short,
    })
}

/// 无模型或请求失败时的本地鼓励（与计划截止的相对关系 + 任务名）
fn feedback_fallback_local(
    title: &str,
    was_on_time: bool,
    early_min: Option<i64>,
    late_min: Option<i64>,
) -> String {
    if let Some(m) = early_min {
        if m > 0 {
            return format!(
                "「{}」比计划截止早约 {} 分钟完成，节奏很好。",
                title, m
            );
        }
    }
    if let Some(m) = late_min {
        if m > 0 {
            return format!(
                "「{}」比计划截止晚约 {} 分钟完成；仍算落地，下次可再掐紧一点。",
                title, m
            );
        }
    }
    if was_on_time {
        format!("「{}」在计划截止时间前（或当时）完成，不错。", title)
    } else {
        format!("「{}」已标记完成，继续推进下一件事吧。", title)
    }
}

fn feedback_prompt(
    title: &str,
    start_at: &str,
    end_at: &str,
    completed_at: &str,
    was_on_time: bool,
    early_min: Option<i64>,
    late_min: Option<i64>,
) -> String {
    let timing = match (early_min, late_min, was_on_time) {
        (Some(e), _, _) if e > 0 => format!("比计划截止早约 {} 分钟（提前完成）", e),
        (_, Some(l), _) if l > 0 => format!("比计划截止晚约 {} 分钟（超时完成）", l),
        _ if was_on_time => "在截止时间前或当时完成（按时）".to_string(),
        _ => "时间关系见上（按本地时钟比较计划截止时刻与实际点选完成时刻）".to_string(),
    };
    format!(
        "你是温和的日程教练。用户刚在应用里点「完成」一项计划。\n\
         - 任务标题：{}\n\
         - 计划时段（本地）：{} ～ {}\n\
         - 用户点「完成」的时刻（本地）：{}\n\
         - 与计划截止的关系：{}\n\n\
         请只输出**一句**中文鼓励（不超过 45 字），自然口语；必须体现「是否赶在截止前」和任务内容，不要引号或 Markdown。",
        title, start_at, end_at, completed_at, timing
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRow {
    pub id: String,
    pub kind: String,
    pub period_key: String,
    pub model_text: String,
    pub created_at: String,
    pub trigger_kind: String,
}

/// 日聚合快照（`v_analytics_snapshot`），按日一行。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSnapshotRow {
    pub day: String,
    pub planned_count: i64,
    pub done_count: i64,
    pub completion_rate: f64,
    pub focus_minutes: i64,
    pub busy_minutes: i64,
    pub first_task_at: Option<String>,
    pub last_completion_at: Option<String>,
}

#[tauri::command]
pub fn list_analytics_snapshot(
    db: tauri::State<Db>,
    limit: Option<i64>,
) -> Result<Vec<AnalyticsSnapshotRow>, String> {
    let lim = limit.unwrap_or(120).min(500);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT day, planned_count, done_count, completion_rate, focus_minutes, busy_minutes, \
             first_task_at, last_completion_at \
             FROM v_analytics_snapshot ORDER BY day DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![lim], |r| {
            Ok(AnalyticsSnapshotRow {
                day: r.get(0)?,
                planned_count: r.get(1)?,
                done_count: r.get(2)?,
                completion_rate: r.get(3)?,
                focus_minutes: r.get(4)?,
                busy_minutes: r.get(5)?,
                first_task_at: r.get(6)?,
                last_completion_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn list_summaries(db: tauri::State<Db>, limit: Option<i64>) -> Result<Vec<SummaryRow>, String> {
    let lim = limit.unwrap_or(50).min(200);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare_cached("SELECT id, kind, period_key, model_text, created_at, trigger_kind FROM summaries ORDER BY created_at DESC LIMIT ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![lim], |r| {
            Ok(SummaryRow {
                id: r.get(0)?,
                kind: r.get(1)?,
                period_key: r.get(2)?,
                model_text: r.get(3)?,
                created_at: r.get(4)?,
                trigger_kind: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub fn engine_run_tool(db: tauri::State<Db>, name: String, args: Value) -> Result<Value, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    run_tool(&conn, &name, &args)
}
