//! 本地 ToolRuntime：只读 SQLite + 确定性计算，无网络。

use crate::metrics::{local_today_string, naive_date_from_str};
use chrono::Local;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub fn run_tool(conn: &Connection, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "get_local_time" => {
            let n = Local::now();
            Ok(json!({
                "iso": n.to_rfc3339(),
                "date": n.format("%Y-%m-%d").to_string(),
                "time": n.format("%H:%M:%S").to_string(),
                "timezone": iana_time_zone::get_timezone().map(|s| s.to_string()).unwrap_or_else(|_| "unknown".into()),
            }))
        }
        "get_daily_rollup" => tool_get_daily_rollup(conn, args),
        "get_weekly_rollup" => tool_get_weekly_rollup(conn, args),
        "completion_rate_series" => tool_completion_rate_series(conn, args),
        "busy_vs_focus_ratio" => tool_busy_vs_focus_ratio(conn, args),
        "overdue_count" => tool_overdue_count(conn, args),
        "list_plan_items" => tool_list_plan_items(conn, args),
        _ => Err(format!("未知工具: {}", name)),
    }
}

fn tool_get_daily_rollup(conn: &Connection, args: &Value) -> Result<Value, String> {
    let date = args
        .get("date")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(local_today_string);
    let row = conn
        .query_row(
            "SELECT date, planned_count, done_count, completion_rate,
                    total_planned_minutes, total_busy_minutes, first_task_at, last_completion_at
             FROM daily_rollups WHERE date = ?1",
            params![date],
            |r| {
                Ok(json!({
                    "date": r.get::<_, String>(0)?,
                    "plannedCount": r.get::<_, i64>(1)?,
                    "doneCount": r.get::<_, i64>(2)?,
                    "completionRate": r.get::<_, f64>(3)?,
                    "totalPlannedMinutes": r.get::<_, i64>(4)?,
                    "totalBusyMinutes": r.get::<_, i64>(5)?,
                    "firstTaskAt": r.get::<_, Option<String>>(6)?,
                    "lastCompletionAt": r.get::<_, Option<String>>(7)?,
                }))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(row.unwrap_or(json!({"error": "no rollup", "date": date})))
}

fn tool_get_weekly_rollup(conn: &Connection, args: &Value) -> Result<Value, String> {
    let week_start = args
        .get("weekStart")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "需要 weekStart".to_string())?;
    let agg: Option<String> = conn
        .query_row(
            "SELECT aggregates FROM weekly_rollups WHERE week_start = ?1",
            params![week_start],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match agg {
        Some(s) => {
            let v: Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            Ok(v)
        }
        None => Ok(json!({"error": "no weekly rollup", "weekStart": week_start})),
    }
}

fn tool_completion_rate_series(conn: &Connection, args: &Value) -> Result<Value, String> {
    let start = args
        .get("startDate")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "需要 startDate".to_string())?;
    let end = args
        .get("endDate")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "需要 endDate".to_string())?;
    let mut stmt = conn
        .prepare_cached(
            "SELECT date, completion_rate FROM daily_rollups
             WHERE date >= ?1 AND date <= ?2 ORDER BY date",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![start, end], |r| {
            Ok(json!({"date": r.get::<_, String>(0)?, "rate": r.get::<_, f64>(1)?}))
        })
        .map_err(|e| e.to_string())?;
    let mut arr = Vec::new();
    for row in rows {
        arr.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "series": arr }))
}

fn tool_busy_vs_focus_ratio(conn: &Connection, args: &Value) -> Result<Value, String> {
    let start = args.get("startDate").and_then(|v| v.as_str()).unwrap_or("");
    let end = args.get("endDate").and_then(|v| v.as_str()).unwrap_or("");
    if start.is_empty() || end.is_empty() {
        return Err("需要 startDate 与 endDate".into());
    }
    let busy: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(total_busy_minutes),0) FROM daily_rollups WHERE date >= ?1 AND date <= ?2",
            params![start, end],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let focus: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(total_planned_minutes),0) FROM daily_rollups WHERE date >= ?1 AND date <= ?2",
            params![start, end],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let ratio = if focus > 0 {
        busy as f64 / focus as f64
    } else {
        0.0
    };
    Ok(json!({
        "busyMinutes": busy,
        "plannedFocusMinutes": focus,
        "ratioBusyOverFocus": ratio
    }))
}

fn tool_overdue_count(conn: &Connection, _args: &Value) -> Result<Value, String> {
    let now = Local::now().to_rfc3339();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM plan_items WHERE status = 'pending' AND end_at < ?1",
            params![now],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(json!({ "overduePending": n, "now": now }))
}

fn tool_list_plan_items(conn: &Connection, args: &Value) -> Result<Value, String> {
    let day = args.get("day").and_then(|v| v.as_str());
    let status = args.get("status").and_then(|v| v.as_str());
    let (sql, p1, p2): (&str, String, String) = match (day, status) {
        (Some(d), Some(s)) => (
            "SELECT id, day, title, start_at, end_at, status FROM plan_items WHERE day = ?1 AND status = ?2 ORDER BY start_at",
            d.to_string(),
            s.to_string(),
        ),
        (Some(d), None) => (
            "SELECT id, day, title, start_at, end_at, status FROM plan_items WHERE day = ?1 ORDER BY start_at",
            d.to_string(),
            String::new(),
        ),
        (None, Some(s)) => (
            "SELECT id, day, title, start_at, end_at, status FROM plan_items WHERE status = ?1 ORDER BY start_at LIMIT 200",
            s.to_string(),
            String::new(),
        ),
        (None, None) => (
            "SELECT id, day, title, start_at, end_at, status FROM plan_items ORDER BY start_at DESC LIMIT 200",
            String::new(),
            String::new(),
        ),
    };
    let mut stmt = conn.prepare_cached(sql).map_err(|e| e.to_string())?;
    let rows = match (day, status) {
        (Some(_), Some(_)) => stmt.query_map(params![p1, p2], row_plan),
        (Some(_), None) => stmt.query_map(params![p1], row_plan),
        (None, Some(_)) => stmt.query_map(params![p1], row_plan),
        (None, None) => stmt.query_map([], row_plan),
    }
    .map_err(|e| e.to_string())?;
    fn row_plan(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "day": r.get::<_, String>(1)?,
            "title": r.get::<_, String>(2)?,
            "startAt": r.get::<_, String>(3)?,
            "endAt": r.get::<_, String>(4)?,
            "status": r.get::<_, String>(5)?,
        }))
    }
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": out }))
}

/// 为总结 Prompt 注入工具 JSON（不经过模型 tool 循环，等价于已执行工具）。
pub fn snapshot_for_daily_summary(conn: &Connection, day: &str) -> Result<String, String> {
    let rollup = run_tool(
        conn,
        "get_daily_rollup",
        &json!({ "date": day }),
    )?;
    let items = run_tool(
        conn,
        "list_plan_items",
        &json!({ "day": day }),
    )?;
    let overdue = run_tool(conn, "overdue_count", &json!({}))?;
    Ok(format!(
        "## 工具数据\n### get_daily_rollup\n{}\n### list_plan_items\n{}\n### overdue_count\n{}",
        serde_json::to_string_pretty(&rollup).unwrap_or_default(),
        serde_json::to_string_pretty(&items).unwrap_or_default(),
        serde_json::to_string_pretty(&overdue).unwrap_or_default()
    ))
}

pub fn snapshot_for_weekly_summary(conn: &Connection, week_start: &str) -> Result<String, String> {
    let end = naive_date_from_str(week_start)? + chrono::Duration::days(6);
    let end_s = end.format("%Y-%m-%d").to_string();
    let weekly = run_tool(
        conn,
        "get_weekly_rollup",
        &json!({ "weekStart": week_start }),
    )?;
    let series = run_tool(
        conn,
        "completion_rate_series",
        &json!({ "startDate": week_start, "endDate": end_s }),
    )?;
    let ratio = run_tool(
        conn,
        "busy_vs_focus_ratio",
        &json!({ "startDate": week_start, "endDate": end_s }),
    )?;
    Ok(format!(
        "## 工具数据\n### get_weekly_rollup\n{}\n### completion_rate_series\n{}\n### busy_vs_focus_ratio\n{}",
        serde_json::to_string_pretty(&weekly).unwrap_or_default(),
        serde_json::to_string_pretty(&series).unwrap_or_default(),
        serde_json::to_string_pretty(&ratio).unwrap_or_default()
    ))
}
