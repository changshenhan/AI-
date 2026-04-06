//! 日/周聚合 — 可重算。

use chrono::{Datelike, Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

pub fn monday_of_week(d: NaiveDate) -> NaiveDate {
    let off = d.weekday().num_days_from_monday();
    d - Duration::days(off as i64)
}

pub fn recompute_daily_rollup(conn: &Connection, day: &str) -> Result<(), String> {
    let planned: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM plan_items WHERE day = ?1",
            params![day],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let done: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM plan_items WHERE day = ?1 AND status = 'done'",
            params![day],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let rate = if planned > 0 {
        done as f64 / planned as f64
    } else {
        0.0
    };

    let total_planned_minutes: i64 = {
        let mut stmt = conn
            .prepare_cached(
                "SELECT start_at, end_at FROM plan_items WHERE day = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![day], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut sum = 0i64;
        for row in rows {
            let (s, e) = row.map_err(|e| e.to_string())?;
            if let (Ok(ts), Ok(te)) = (
                chrono::DateTime::parse_from_rfc3339(&s),
                chrono::DateTime::parse_from_rfc3339(&e),
            ) {
                sum += (te - ts).num_minutes().max(0);
            }
        }
        sum
    };

    let total_busy_minutes: i64 = {
        let mut stmt = conn
            .prepare_cached(
                "SELECT start_at, end_at FROM busy_blocks WHERE substr(start_at,1,10) = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![day], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut sum = 0i64;
        for row in rows {
            let (s, e) = row.map_err(|e| e.to_string())?;
            if let (Ok(ts), Ok(te)) = (
                chrono::DateTime::parse_from_rfc3339(&s),
                chrono::DateTime::parse_from_rfc3339(&e),
            ) {
                sum += (te - ts).num_minutes().max(0);
            }
        }
        sum
    };

    let first_task_at: Option<String> = conn
        .query_row(
            "SELECT MIN(start_at) FROM plan_items WHERE day = ?1",
            params![day],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();

    let last_completion_at: Option<String> = conn
        .query_row(
            "SELECT MAX(c.completed_at) FROM completions c
             JOIN plan_items p ON p.id = c.task_id WHERE p.day = ?1",
            params![day],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();

    conn.execute(
        "INSERT INTO daily_rollups (
            date, planned_count, done_count, completion_rate,
            total_planned_minutes, total_busy_minutes, first_task_at, last_completion_at
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
        ON CONFLICT(date) DO UPDATE SET
            planned_count = excluded.planned_count,
            done_count = excluded.done_count,
            completion_rate = excluded.completion_rate,
            total_planned_minutes = excluded.total_planned_minutes,
            total_busy_minutes = excluded.total_busy_minutes,
            first_task_at = excluded.first_task_at,
            last_completion_at = excluded.last_completion_at",
        params![
            day,
            planned,
            done,
            rate,
            total_planned_minutes,
            total_busy_minutes,
            first_task_at,
            last_completion_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn recompute_weekly_rollup(conn: &Connection, week_start: &str) -> Result<(), String> {
    let start = NaiveDate::parse_from_str(week_start, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let end = start + Duration::days(7);
    let mut days = Vec::new();
    let mut d = start;
    while d < end {
        days.push(d.format("%Y-%m-%d").to_string());
        d += Duration::days(1);
    }

    let mut rates = Vec::new();
    let mut total_planned = 0i64;
    let mut total_done = 0i64;
    for day in &days {
        recompute_daily_rollup(conn, day)?;
        let (p, dn): (i64, i64) = conn
            .query_row(
                "SELECT planned_count, done_count FROM daily_rollups WHERE date = ?1",
                params![day],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        total_planned += p;
        total_done += dn;
        let r: f64 = conn
            .query_row(
                "SELECT completion_rate FROM daily_rollups WHERE date = ?1",
                params![day],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        rates.push(r);
    }

    let avg_rate = if rates.is_empty() {
        0.0
    } else {
        rates.iter().sum::<f64>() / rates.len() as f64
    };

    let aggregates = json!({
        "weekStart": week_start,
        "days": days,
        "dailyCompletionRates": rates,
        "avgCompletionRate": avg_rate,
        "totalPlannedTasks": total_planned,
        "totalDoneTasks": total_done,
    });

    conn.execute(
        "INSERT INTO weekly_rollups (week_start, aggregates) VALUES (?1, ?2)
         ON CONFLICT(week_start) DO UPDATE SET aggregates = excluded.aggregates",
        params![week_start, aggregates.to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn local_today_string() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

pub fn naive_date_from_str(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| e.to_string())
}

pub fn previous_week_monday(now: NaiveDate) -> NaiveDate {
    let this_mon = monday_of_week(now);
    this_mon - Duration::days(7)
}
