//! 自然语言排期：LLM 产出 JSON → 写入 busy + 计划块布局。

use crate::db::Db;
use crate::llm::{llm_complete, ChatMessage, LlmSettings};
use crate::metrics::recompute_daily_rollup;
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Weekday};
use rusqlite::params;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub busy_inserted: usize,
    pub plans_inserted: usize,
    pub note: String,
    pub trace_id: String,
}

fn extract_json_object(text: &str) -> Result<Value, String> {
    let t = text.trim();
    if let Some(start) = t.find("```") {
        let after = &t[start + 3..];
        let after = after.trim_start_matches(['j', 's', 'o', 'n', '\n', '\r']);
        if let Some(end) = after.rfind("```") {
            let inner = after[..end].trim();
            return serde_json::from_str(inner).map_err(|e| format!("JSON: {}", e));
        }
    }
    if let Some(i) = t.find('{') {
        if let Some(j) = t.rfind('}') {
            let inner = &t[i..=j];
            return serde_json::from_str(inner).map_err(|e| format!("JSON: {}", e));
        }
    }
    Err("响应中未找到 JSON".into())
}

fn day_start_end_rfc(day: &str) -> Result<(chrono::DateTime<Local>, chrono::DateTime<Local>), String> {
    let d = NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let start = d
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| "日期无效".to_string())?;
    let end = d
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| "日期无效".to_string())?;
    let start = Local
        .from_local_datetime(&start)
        .single()
        .ok_or_else(|| "时区歧义".to_string())?;
    let end = Local
        .from_local_datetime(&end)
        .single()
        .ok_or_else(|| "时区歧义".to_string())?;
    Ok((start, end))
}

fn weekday_zh(d: NaiveDate) -> &'static str {
    match d.weekday() {
        Weekday::Mon => "周一",
        Weekday::Tue => "周二",
        Weekday::Wed => "周三",
        Weekday::Thu => "周四",
        Weekday::Fri => "周五",
        Weekday::Sat => "周六",
        Weekday::Sun => "周日",
    }
}

/// 用户明确要在用餐时间工作/开会时，不再插入默认午餐/晚餐占位。
/// 模型常把「A，然后 B」合并成一条 title；在后端按连接词拆成多条 task，避免只生成一个计划块。
fn expand_tasks_if_single_merged_title(tasks: Vec<Value>) -> Vec<Value> {
    if tasks.len() != 1 {
        return tasks;
    }
    let t0 = match tasks.get(0) {
        Some(v) => v.clone(),
        None => return tasks,
    };
    let title = t0
        .get("title")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    if title.len() < 6 {
        return tasks;
    }
    let dur = t0.get("durationMinutes").cloned();

    let seps = [
        "，然后", "。然后", "；然后", ",然后", "，接着", "。接着", "；接着",
        "，另外", "。另外", "；另外",
        "，还有", "。还有",
    ];
    for sep in seps {
        if let Some((a, b)) = title.split_once(sep) {
            let a = a.trim();
            let b = b.trim();
            if a.len() >= 2 && b.len() >= 2 {
                return vec![
                    json!({ "title": a, "durationMinutes": dur }),
                    json!({ "title": b, "durationMinutes": dur }),
                ];
            }
        }
    }
    // 中文分号分隔的两件事（长度足够才拆，避免误伤）
    if let Some((a, b)) = title.split_once('；') {
        let a = a.trim();
        let b = b.trim();
        if a.len() >= 4 && b.len() >= 4 {
            return vec![
                json!({ "title": a, "durationMinutes": dur }),
                json!({ "title": b, "durationMinutes": dur }),
            ];
        }
    }
    tasks
}

fn implicit_meal_blocks_disabled(user_text: &str) -> bool {
    let t = user_text;
    t.contains("用餐时工作")
        || t.contains("吃饭时工作")
        || t.contains("午饭时工作")
        || t.contains("午饭时间工作")
        || t.contains("晚饭时工作")
        || t.contains("晚餐时工作")
        || t.contains("不避开用餐")
        || t.contains("取消默认用餐")
        || t.contains("午休也要")
}

fn parse_hhmm(day: &str, hhmm: &str) -> Result<chrono::DateTime<Local>, String> {
    let d = NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let parts: Vec<&str> = hhmm.trim().split(':').collect();
    if parts.len() < 2 {
        return Err("时间格式需 HH:MM".into());
    }
    let h: u32 = parts[0].parse().map_err(|_| "小时无效")?;
    let m: u32 = parts[1].parse().map_err(|_| "分钟无效")?;
    let nt = NaiveTime::from_hms_opt(h, m, 0).ok_or_else(|| "时间无效".to_string())?;
    let ndt = d.and_time(nt);
    Local
        .from_local_datetime(&ndt)
        .single()
        .ok_or_else(|| "时区歧义".into())
}

/// 在扣除 busy 后的空隙中顺序放入任务，默认每任务 60 分钟或模型给定 durationMinutes。
pub async fn apply_natural_language(
    db: &Db,
    settings: &LlmSettings,
    day: &str,
    user_text: &str,
    trace_id: &str,
) -> Result<ApplyResult, String> {
    let now = Local::now();
    let target_naive = NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let today_naive = now.date_naive();
    if target_naive < today_naive {
        return Err("不能为过去的日期排期".into());
    }

    let is_today = target_naive == today_naive;
    let now_line = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let tz = iana_time_zone::get_timezone()
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "local".into());
    let now_rfc = now.to_rfc3339();

    let sys = format!(
        "你是日程解析器。以下为引擎提供的**真实本地时间**（由系统时钟给出，请严格服从）：\n\
         - 当前时间（RFC3339）：{now_rfc}\n\
         - 当前时区：{tz}\n\
         - 目标日期：{day}（{wk}）\n\
         - 目标日是否为今天：{is_today}\n\
         规则（必须遵守）：\n\
         1) 若目标日为**今天**，则 busyBlocks 与 tasks 所涉时段均不得早于当前时间；不要把任务放到已经过去的上午/中午。\n\
         2) 若目标日为**今天**，安排顺序应从「当前时刻之后」的可行空档开始。\n\
         3) 默认 12:00–13:00、18:00–19:00 视为午餐/晚餐时间，勿把**工作任务**排在这些时段，除非用户明确要求在该时段工作。\n\
         4) 若目标日晚于今天，可按整日安排，但仍建议避开上述用餐时段除非用户要求。\n\
         5) **tasks 数组**：用户每提到一件**独立工作**（例如「翻译一篇小说」与「开发一个软件」是两件），必须各占 `tasks` 中的一条，**一条 title 只写一件事**；禁止用顿号/逗号/「然后」「还有」把多件事糊成一条 title。\n\
         6) 若用户用「然后」「另外」「还有」连接多步工作，仍须拆成多个 task 对象。\n\
         只输出一个 JSON 对象，不要 markdown。Schema: \
         {{ \"busyBlocks\": [ {{ \"start\":\"HH:MM\", \"end\":\"HH:MM\", \"label\": string }} ], \
         \"tasks\": [ {{ \"title\": string, \"durationMinutes\"?: number }} ] }}。\
         busyBlocks 表示用户**不可用于安排**的时间段（将显示为红色）。\
         tasks：每项为**单独计划块**；未给 durationMinutes 时默认 60。",
        now_rfc = now_rfc.as_str(),
        tz = tz.as_str(),
        day = day,
        wk = weekday_zh(target_naive),
        is_today = is_today,
    );
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: sys,
        },
        ChatMessage {
            role: "user".into(),
            content: format!(
                "{}\n\n（引擎要求：若包含多件独立工作，JSON 里 tasks 必须多条，每条 title 一件事。）",
                user_text
            ),
        },
    ];
    let raw = llm_complete(settings, messages).await?;
    let v = extract_json_object(&raw)?;
    let busy_arr = v
        .get("busyBlocks")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let tasks_arr = v
        .get("tasks")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let tasks_arr = expand_tasks_if_single_merged_title(tasks_arr);

    let (day_start, day_end) = day_start_end_rfc(day)?;

    let planning_start = if is_today {
        now
    } else {
        day_start
    };

    let mut busy_inserted = 0usize;
    let mut merged_busy: Vec<(chrono::DateTime<Local>, chrono::DateTime<Local>, String)> =
        Vec::new();

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    for b in &busy_arr {
        let start_s = b.get("start").and_then(|x| x.as_str()).unwrap_or("09:00");
        let end_s = b.get("end").and_then(|x| x.as_str()).unwrap_or("10:00");
        let label = b
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or("忙碌")
            .to_string();
        let mut ts = parse_hhmm(day, start_s)?;
        let te = parse_hhmm(day, end_s)?;
        if is_today && te > planning_start && ts < planning_start {
            ts = planning_start;
        }
        if te <= ts {
            continue;
        }
        if is_today && te <= planning_start {
            continue;
        }
        let id = format!("b{}", Uuid::new_v4());
        conn.execute(
            "INSERT INTO busy_blocks (id, start_at, end_at, label, source_message_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                ts.to_rfc3339(),
                te.to_rfc3339(),
                label,
                trace_id,
                now_rfc.clone()
            ],
        )
        .map_err(|e| e.to_string())?;
        busy_inserted += 1;
        merged_busy.push((ts, te, label));
    }
    merged_busy.sort_by_key(|x| x.0);

    // 默认可打断用餐窗口（仅影响排任务，不写入 DB；除非用户声明要在用餐时工作）
    if !implicit_meal_blocks_disabled(user_text) {
        let lunch_s = parse_hhmm(day, "12:00")?;
        let lunch_e = parse_hhmm(day, "13:00")?;
        let dinner_s = parse_hhmm(day, "18:00")?;
        let dinner_e = parse_hhmm(day, "19:00")?;
        merged_busy.push((lunch_s, lunch_e, "午餐（默认占位）".into()));
        merged_busy.push((dinner_s, dinner_e, "晚餐（默认占位）".into()));
        merged_busy.sort_by_key(|x| x.0);
    }

    // 生成空闲区间 [planning_start, day_end] 减去 busy
    let mut free: Vec<(chrono::DateTime<Local>, chrono::DateTime<Local>)> = Vec::new();
    let mut cursor = planning_start;
    for (bs, be, _) in &merged_busy {
        let bs = *bs;
        let be = *be;
        // 已完全过去的忙碌段不影响从「当前」开始的空档计算
        if be <= planning_start {
            continue;
        }
        let eff_bs = bs.max(planning_start);
        if eff_bs > cursor {
            free.push((cursor, eff_bs));
        }
        cursor = be.max(cursor).max(planning_start);
    }
    if cursor < day_end {
        free.push((cursor, day_end));
    }
    if free.is_empty() && merged_busy.is_empty() {
        free.push((planning_start, day_end));
    }

    let mut plans_inserted = 0usize;
    let mut gap_idx = 0;
    let mut cursor_in_gap: Option<chrono::DateTime<Local>> = None;

    for t in &tasks_arr {
        let title = t
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("任务")
            .to_string();
        let dur_m = t
            .get("durationMinutes")
            .and_then(|x| x.as_u64())
            .unwrap_or(60) as i64;
        let need = Duration::minutes(dur_m);

        let mut placed = false;
        while gap_idx < free.len() && !placed {
            let (gs, ge) = free[gap_idx];
            let start = cursor_in_gap.unwrap_or(gs);
            if start >= ge {
                gap_idx += 1;
                cursor_in_gap = None;
                continue;
            }
            let end = start + need;
            if end <= ge {
                let id = format!("p{}", Uuid::new_v4());
                let src = format!("trace:{}", trace_id);
                conn.execute(
                    "INSERT INTO plan_items (id, day, title, start_at, end_at, status, source_conversation_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
                    params![
                        id,
                        day,
                        title,
                        start.to_rfc3339(),
                        end.to_rfc3339(),
                        src
                    ],
                )
                .map_err(|e| e.to_string())?;
                plans_inserted += 1;
                cursor_in_gap = Some(end);
                placed = true;
            } else {
                gap_idx += 1;
                cursor_in_gap = None;
            }
        }
    }

    drop(conn);
    {
        let g = db.0.lock().map_err(|e| e.to_string())?;
        recompute_daily_rollup(&*g, day)?;
    }

    Ok(ApplyResult {
        busy_inserted,
        plans_inserted,
        trace_id: trace_id.to_string(),
        note: format!(
            "已写入 {} 个不可用块、{} 个计划项（规划锚点：{}，时区 {}）。溯源 ID：{}",
            busy_inserted, plans_inserted, now_line, tz, trace_id
        ),
    })
}
