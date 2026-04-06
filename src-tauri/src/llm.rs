//! 多厂商 LLM：服务端 HTTP（无 CORS）。协议：OpenAI 兼容、Anthropic、Gemini。

use crate::secure_settings;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettings {
    /// 兼容前端 camelCase 与少数环境下传入的 snake_case。
    #[serde(alias = "provider_id")]
    pub provider_id: String,
    #[serde(alias = "api_key")]
    pub api_key: String,
    #[serde(default, alias = "base_url_override")]
    pub base_url_override: Option<String>,
    pub model: String,
}

#[derive(Debug, Serialize)]
pub struct LlmTestResult {
    pub ok: bool,
    pub message: String,
    pub protocol: String,
    pub resolved_base_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub(crate) fn protocol_for(provider_id: &str) -> &'static str {
    match provider_id {
        "anthropic" | "anthropic_custom" => "anthropic",
        "google_gemini" | "gemini_custom" => "gemini",
        _ => "openai_compatible",
    }
}

fn default_base_url(provider_id: &str) -> Option<&'static str> {
    Some(match provider_id {
        "openai" => "https://api.openai.com/v1",
        "deepseek" => "https://api.deepseek.com/v1",
        "groq" => "https://api.groq.com/openai/v1",
        "moonshot" => "https://api.moonshot.cn/v1",
        "zhipu" => "https://open.bigmodel.cn/api/paas/v4",
        "qwen" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "mistral" => "https://api.mistral.ai/v1",
        "openrouter" => "https://openrouter.ai/api/v1",
        "together" => "https://api.together.xyz/v1",
        "ollama" => "http://127.0.0.1:11434/v1",
        "anthropic" => "https://api.anthropic.com",
        "google_gemini" => "https://generativelanguage.googleapis.com",
        "custom_openai" => return None,
        "anthropic_custom" => return None,
        "gemini_custom" => return None,
        _ => return None,
    })
}

pub(crate) fn resolve_base(settings: &LlmSettings) -> Result<String, String> {
    if let Some(o) = &settings.base_url_override {
        let t = o.trim();
        if t.is_empty() {
            return Err("自定义 Base URL 不能为空".into());
        }
        return Ok(trim_slash(t));
    }
    default_base_url(&settings.provider_id)
        .map(|s| s.to_string())
        .ok_or_else(|| "请填写 Base URL（自定义厂商）".into())
}

pub(crate) fn trim_slash(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}

fn extract_openai_content(v: &Value) -> Result<String, String> {
    v.pointer("/choices/0/message/content")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            let err = v
                .pointer("/error/message")
                .or_else(|| v.pointer("/error"))
                .and_then(|x| x.as_str())
                .unwrap_or("未知响应格式");
            format!("OpenAI 兼容 API: {}", err)
        })
}

fn extract_anthropic_content(v: &Value) -> Result<String, String> {
    if let Some(t) = v.pointer("/content/0/text").and_then(|x| x.as_str()) {
        return Ok(t.to_string());
    }
    let err = v
        .pointer("/error/message")
        .and_then(|x| x.as_str())
        .unwrap_or("未知 Anthropic 响应");
    Err(format!("Anthropic API: {}", err))
}

fn extract_gemini_content(v: &Value) -> Result<String, String> {
    if let Some(s) = v
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|x| x.as_str())
    {
        return Ok(s.to_string());
    }
    let err = v
        .pointer("/error/message")
        .and_then(|x| x.as_str())
        .unwrap_or("未知 Gemini 响应");
    Err(format!("Gemini API: {}", err))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn urlencoding_encode_key(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

async fn chat_full(
    settings: &LlmSettings,
    messages: Vec<ChatMessage>,
) -> Result<(String, &'static str, String), String> {
    let protocol = protocol_for(&settings.provider_id);
    let base = resolve_base(settings)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    match protocol {
        "openai_compatible" => {
            let url = format!("{}/chat/completions", base);
            let body = json!({
                "model": settings.model,
                "messages": messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
                "temperature": 0.6,
            });
            let mut req = client.post(&url).header("Content-Type", "application/json");
            let key = settings.api_key.trim();
            let skip_auth = settings.provider_id == "ollama" && key.is_empty();
            if !skip_auth {
                req = req.bearer_auth(key);
            }
            if settings.provider_id == "openrouter" {
                req = req
                    .header("HTTP-Referer", "https://github.com/songlvhan/ai-schedule-engine")
                    .header("X-Title", "AI Schedule Engine");
            }
            let v = req.json(&body).send().await.map_err(|e| e.to_string())?;
            let status = v.status();
            let text = v.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("HTTP {} — {}", status, truncate(&text, 800)));
            }
            let val: Value = serde_json::from_str(&text)
                .map_err(|e| format!("JSON: {} — {}", e, truncate(&text, 400)))?;
            let out = extract_openai_content(&val)?;
            Ok((out, "openai_compatible", base))
        }
        "anthropic" => {
            let url = format!("{}/v1/messages", trim_slash(&base));
            let body = json!({
                "model": settings.model,
                "max_tokens": 1024,
                "messages": messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
            });
            let v = client
                .post(&url)
                .header("x-api-key", &settings.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = v.status();
            let text = v.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("HTTP {} — {}", status, truncate(&text, 800)));
            }
            let val: Value = serde_json::from_str(&text).map_err(|e| format!("JSON: {}", e))?;
            let out = extract_anthropic_content(&val)?;
            Ok((out, "anthropic", base))
        }
        "gemini" => {
            let model = settings.model.trim().trim_start_matches("models/");
            let url = format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                trim_slash(&base),
                model,
                urlencoding_encode_key(&settings.api_key)
            );
            let mut contents: Vec<Value> = Vec::new();
            for m in &messages {
                let role = match m.role.as_str() {
                    "assistant" => "model",
                    "model" => "model",
                    _ => "user",
                };
                contents.push(json!({
                    "role": role,
                    "parts": [{ "text": m.content }]
                }));
            }
            let body = json!({ "contents": contents });
            let v = client
                .post(&url)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = v.status();
            let text = v.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("HTTP {} — {}", status, truncate(&text, 800)));
            }
            let val: Value = serde_json::from_str(&text).map_err(|e| format!("JSON: {}", e))?;
            let out = extract_gemini_content(&val)?;
            Ok((out, "gemini", base))
        }
        _ => Err("未知协议".into()),
    }
}

async fn run_llm_test(settings: LlmSettings) -> Result<LlmTestResult, String> {
    let msgs = vec![ChatMessage {
        role: "user".into(),
        content: "Reply with exactly: OK".into(),
    }];
    let (text, protocol, base) = chat_full(&settings, msgs).await?;
    let ok = !text.trim().is_empty();
    Ok(LlmTestResult {
        ok,
        message: text,
        protocol: protocol.to_string(),
        resolved_base_url: Some(base),
    })
}

/// 供日程/总结等模块复用（避免重复 HTTP 逻辑）。
pub async fn llm_complete(settings: &LlmSettings, messages: Vec<ChatMessage>) -> Result<String, String> {
    let (text, _, _) = chat_full(settings, messages).await?;
    Ok(text)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("llm_settings.json"))
}

#[tauri::command]
pub fn llm_save_settings(app: AppHandle, settings: LlmSettings) -> Result<(), String> {
    let j = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    secure_settings::save_json(&j)?;
    let p = settings_path(&app)?;
    if p.exists() {
        let _ = fs::remove_file(&p);
    }
    Ok(())
}

#[tauri::command]
pub fn llm_load_settings(app: AppHandle) -> Result<Option<LlmSettings>, String> {
    load_llm_settings_inner(&app)
}

pub fn load_llm_settings_or_err(app: &AppHandle) -> Result<LlmSettings, String> {
    load_llm_settings_inner(app).and_then(|o| o.ok_or_else(|| "未配置 LLM".into()))
}

/// 界面传入的 `settings`（与 `llm_chat` 一致）优先；若缺字段/全空（例如旧版 IPC 异常）则回退钥匙串。
pub fn resolve_llm_settings_for_command(
    app: &AppHandle,
    from_ui: LlmSettings,
) -> Result<LlmSettings, String> {
    if settings_from_ui_usable(&from_ui) {
        return Ok(from_ui);
    }
    load_llm_settings_inner(app)?.ok_or_else(|| {
        "未找到 LLM 配置：请返回连接页点击「保存并进入」；若已保存仍失败，请重新编译/安装最新应用（勿使用旧版桌面包）。".into()
    })
}

fn settings_from_ui_usable(s: &LlmSettings) -> bool {
    let pid = s.provider_id.trim();
    let model = s.model.trim();
    if pid.is_empty() || model.is_empty() {
        return false;
    }
    if pid == "ollama" {
        return true;
    }
    !s.api_key.trim().is_empty()
}

fn load_llm_settings_inner(app: &AppHandle) -> Result<Option<LlmSettings>, String> {
    if let Some(j) = secure_settings::load_json()? {
        let s: LlmSettings = serde_json::from_str(&j).map_err(|e| e.to_string())?;
        return Ok(Some(s));
    }
    let p = settings_path(app)?;
    if p.exists() {
        let raw = fs::read_to_string(&p).map_err(|e| e.to_string())?;
        let s: LlmSettings = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let j = serde_json::to_string(&s).map_err(|e| e.to_string())?;
        let _ = secure_settings::save_json(&j);
        let _ = fs::remove_file(&p);
        return Ok(Some(s));
    }
    Ok(None)
}

#[tauri::command]
pub fn llm_clear_settings(app: AppHandle) -> Result<(), String> {
    let _ = secure_settings::delete_entry();
    let p = settings_path(&app)?;
    if p.exists() {
        fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn llm_test(settings: LlmSettings) -> Result<LlmTestResult, String> {
    run_llm_test(settings).await
}

#[tauri::command]
pub async fn llm_chat(settings: LlmSettings, messages: Vec<ChatMessage>) -> Result<String, String> {
    llm_complete(&settings, messages).await
}

#[tauri::command]
pub async fn llm_chat_stream(
    app: AppHandle,
    settings: LlmSettings,
    messages: Vec<ChatMessage>,
) -> Result<(), String> {
    crate::llm_stream::stream_chat_events(app, settings, messages).await
}
