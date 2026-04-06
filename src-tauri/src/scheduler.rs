//! 主进程定时：跨日检测日总结、周一检测周总结。

use crate::db::{get_scheduler_val, set_scheduler_val, Db};
use crate::metrics::previous_week_monday;
use crate::summary_service::{try_daily_summary, try_weekly_summary};
use chrono::{Datelike, Local, NaiveDate, Timelike};
use tauri::AppHandle;

pub fn scheduler_tick(app: &AppHandle, db: &Db) {
    let today = Local::now().date_naive();
    let today_s = today.format("%Y-%m-%d").to_string();

    let Ok(conn) = db.0.lock() else {
        return;
    };

    let last = match get_scheduler_val(&conn, "last_seen_calendar_date") {
        Ok(v) => v,
        Err(_) => None,
    };

    if let Some(ls) = last {
        if let Ok(last_d) = NaiveDate::parse_from_str(&ls, "%Y-%m-%d") {
            if today > last_d {
                let summarize_day = last_d.format("%Y-%m-%d").to_string();
                drop(conn);
                let app = app.clone();
                let db = db.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = try_daily_summary(&app, &db, &summarize_day, "midnight").await;
                    if let Ok(c) = db.0.lock() {
                        let _ = set_scheduler_val(&c, "last_seen_calendar_date", &today_s);
                    }
                });
                return;
            }
        }
    } else {
        let _ = set_scheduler_val(&conn, "last_seen_calendar_date", &today_s);
    }
    drop(conn);

    // 每周一 00:00 后：总结上一完整周（与 catchup 幂等重叠无妨）
    if Local::now().weekday() == chrono::Weekday::Mon {
        let hour = Local::now().hour();
        if hour == 0 || hour == 1 {
            let ws = previous_week_monday(today).format("%Y-%m-%d").to_string();
            let last_w = db
                .0
                .lock()
                .ok()
                .and_then(|c| get_scheduler_val(&c, "last_weekly_summarized").ok())
                .flatten();
            if last_w.as_deref() != Some(&ws) {
                let app = app.clone();
                let db = db.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = try_weekly_summary(&app, &db, &ws, "weekly_monday").await;
                    if let Ok(c) = db.0.lock() {
                        let _ = set_scheduler_val(&c, "last_weekly_summarized", &ws);
                    }
                });
            }
        }
    }
}
