//! OpenAI 兼容 API 的 SSE 流式输出，经事件 `llm/stream` 推送到前端。

use crate::llm::{protocol_for, resolve_base, trim_slash, ChatMessage, LlmSettings};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub async fn stream_chat_events(
    app: AppHandle,
    settings: LlmSettings,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    if protocol_for(&settings.provider_id) != "openai_compatible" {
        let text = crate::llm::llm_complete(&settings, messages).await?;
        let _ = app.emit(
            "llm/stream",
            json!({ "delta": text, "done": true, "mode": "single" }),
        );
        return Ok(());
    }

    let base = resolve_base(&settings)?;
    let url = format!("{}/chat/completions", trim_slash(&base));
    let body = json!({
        "model": settings.model,
        "messages": messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
        "temperature": 0.6,
        "stream": true,
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let key = settings.api_key.trim();
    let skip_auth = settings.provider_id == "ollama" && key.is_empty();
    let mut req = client.post(&url).header("Content-Type", "application/json");
    if !skip_auth {
        req = req.bearer_auth(key);
    }
    if settings.provider_id == "openrouter" {
        req = req
            .header("HTTP-Referer", "https://github.com/songlvhan/ai-schedule-engine")
            .header("X-Title", "AI Schedule Engine");
    }
    let response = req.json(&body).send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        let t = response.text().await.map_err(|e| e.to_string())?;
        return Err(format!(
            "HTTP {} — {}",
            status,
            t.chars().take(500).collect::<String>()
        ));
    }

    let mut stream = response.bytes_stream();
    let mut carry = String::new();
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        carry.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(nl) = carry.find('\n') {
            let line = carry[..nl].trim().to_string();
            carry.drain(..=nl);
            if line.is_empty() {
                continue;
            }
            if line == "data: [DONE]" {
                let _ = app.emit("llm/stream", json!({ "delta": "", "done": true, "mode": "stream" }));
                return Ok(());
            }
            if let Some(rest) = line.strip_prefix("data: ") {
                if rest == "[DONE]" {
                    let _ = app.emit("llm/stream", json!({ "delta": "", "done": true, "mode": "stream" }));
                    return Ok(());
                }
                let v: Value = match serde_json::from_str(rest) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(delta) = v.pointer("/choices/0/delta/content").and_then(|x| x.as_str()) {
                    if !delta.is_empty() {
                        let _ = app.emit(
                            "llm/stream",
                            json!({ "delta": delta, "done": false, "mode": "stream" }),
                        );
                    }
                }
            }
        }
    }
    let _ = app.emit("llm/stream", json!({ "delta": "", "done": true, "mode": "stream" }));
    Ok(())
}
