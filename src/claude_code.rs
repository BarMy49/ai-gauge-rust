//! Claude subscription limits read through Claude Code's own sign-in.
//! claude.ai answers automated background reads with a Cloudflare bot check,
//! so when Claude Code is signed in we ask the same usage endpoint that its
//! `/usage` command uses. The credentials file is only read, never written,
//! and the token is never logged or shown.
use crate::providers;
use anyhow::{anyhow, ensure, Result};
use serde_json::Value;
use std::{fs, path::PathBuf};
use zeroize::Zeroizing;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

fn credentials_path() -> Option<PathBuf> {
    let dir = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => directories::BaseDirs::new()?.home_dir().join(".claude"),
    };
    Some(dir.join(".credentials.json"))
}

/// True when Claude Code has stored a sign-in on this computer.
pub fn available() -> bool {
    credentials_path().is_some_and(|p| p.is_file())
}

fn token_from(v: &Value, now_ms: i64) -> Result<Zeroizing<String>> {
    let o = &v["claudeAiOauth"];
    let token = o["accessToken"]
        .as_str()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| {
            anyhow!("Claude Code nie jest zalogowany kontem Claude. Uruchom claude i wpisz /login.")
        })?;
    if let Some(exp) = o["expiresAt"].as_i64() {
        ensure!(
            exp > now_ms + 30_000,
            "Logowanie Claude Code wygasło. Uruchom claude na chwilę, aby je odświeżyło, i naciśnij Odśwież."
        );
    }
    Ok(Zeroizing::new(token.to_owned()))
}

pub fn usage() -> Result<Value> {
    let path = credentials_path().ok_or_else(|| anyhow!("Brak katalogu domowego"))?;
    let raw = Zeroizing::new(
        fs::read(&path).map_err(|_| anyhow!("Nie można odczytać logowania Claude Code"))?,
    );
    let v: Value = serde_json::from_slice(&raw)
        .map_err(|_| anyhow!("Nieznany format logowania Claude Code"))?;
    let token = token_from(&v, chrono::Utc::now().timestamp_millis())?;
    providers::response(
        providers::client()?
            .get(USAGE_URL)
            .bearer_auth(token.as_str())
            .header("anthropic-beta", "oauth-2025-04-20"),
    )
    .map_err(|e| {
        if e.to_string().starts_with("HTTP 401") {
            anyhow!("Logowanie Claude Code wygasło. Uruchom claude na chwilę, aby je odświeżyło, i naciśnij Odśwież.")
        } else {
            e
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn token_requires_valid_unexpired_sign_in() {
        let now = 1_000_000;
        let ok = json!({"claudeAiOauth":{"accessToken":"t","expiresAt":now+3_600_000}});
        assert_eq!(token_from(&ok, now).unwrap().as_str(), "t");
        let expired = json!({"claudeAiOauth":{"accessToken":"t","expiresAt":now-1}});
        let e = token_from(&expired, now).unwrap_err().to_string();
        assert!(e.contains("wygasło"));
        assert!(token_from(&json!({}), now).is_err());
        assert!(token_from(&json!({"claudeAiOauth":{"accessToken":""}}), now).is_err());
    }
    #[test]
    fn usage_response_parses_like_the_web_panel() {
        let v = json!({
            "five_hour":{"utilization":12.0,"resets_at":"2026-11-27T18:00:00+00:00"},
            "seven_day":{"utilization":40.5,"resets_at":"2026-12-01T10:00:00+00:00"},
            "seven_day_opus":null,
            "extra_usage":{"is_enabled":false,"utilization":null}
        });
        let rows = providers::claude(&v).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|m| m.percent == Some(40.5)));
    }
}
