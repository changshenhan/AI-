//! API 配置写入系统钥匙串（macOS Keychain / Windows Credential / 等）。

use keyring::Entry;

const SERVICE: &str = "com.songlvhan.ai-schedule-engine";
const USER: &str = "llm_settings_v1";

pub fn save_json(json: &str) -> Result<(), String> {
    let e = Entry::new(SERVICE, USER).map_err(|e| e.to_string())?;
    e.set_password(json).map_err(|e| e.to_string())
}

pub fn load_json() -> Result<Option<String>, String> {
    let e = Entry::new(SERVICE, USER).map_err(|e| e.to_string())?;
    match e.get_password() {
        Ok(s) if !s.is_empty() => Ok(Some(s)),
        Ok(_) => Ok(None),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn delete_entry() -> Result<(), String> {
    let e = Entry::new(SERVICE, USER).map_err(|e| e.to_string())?;
    match e.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
