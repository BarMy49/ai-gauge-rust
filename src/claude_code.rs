//! Claude subscription limits read through Claude Code's own sign-in.
//! claude.ai answers automated background reads with a Cloudflare bot check,
//! so when Claude Code is signed in we ask the same usage endpoint that its
//! `/usage` command uses. The credentials file is only read, never written,
//! and the token is never logged or shown.
use crate::{model::*, providers};
use anyhow::{anyhow, bail, ensure, Result};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
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

/// The usage endpoint rate-limits quickly (HTTP 429), so it is asked at most
/// this often. Between requests the last answer, kept on disk, is shown.
const MIN_REQUEST_GAP_SECS: i64 = 5 * 60;
/// Wait after a 429 when the server does not say how long.
const DEFAULT_BACKOFF_SECS: i64 = 15 * 60;
const CACHE_FILE: &str = "claude-usage.json";
const EXPIRED: &str = "Logowanie Claude Code wygasło. Uruchom claude na chwilę, aby je odświeżyło, i naciśnij Odśwież.";

#[derive(Debug, PartialEq)]
enum Plan {
    Request,
    Cached,
}
fn plan(last_request: Option<i64>, blocked_until: Option<i64>, now: i64) -> Plan {
    if blocked_until.is_some_and(|b| now < b) {
        return Plan::Cached;
    }
    match last_request {
        Some(t) if now - t < MIN_REQUEST_GAP_SECS => Plan::Cached,
        _ => Plan::Request,
    }
}

fn load_cache(root: &Path) -> Value {
    fs::read(root.join(CACHE_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
}
fn save_cache(root: &Path, v: &Value) {
    if let Ok(bytes) = serde_json::to_vec(v) {
        let _ = fs::write(root.join(CACHE_FILE), bytes);
    }
}
fn local_time(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default()
}
fn limited_note(until: i64) -> String {
    format!(
        "; Claude ogranicza zapytania, kolejna próba po {}",
        local_time(until)
    )
}

/// Rows from the last successful answer, marked with when it was read.
fn cached_rows(cache: &Value, note: &str) -> Result<Vec<Metric>> {
    let fetched = cache["fetched"]
        .as_i64()
        .ok_or_else(|| anyhow!("Brak zapisanego odczytu"))?;
    let mut rows = providers::claude(&cache["usage"])?;
    let when = Utc
        .timestamp_opt(fetched, 0)
        .single()
        .unwrap_or_else(Utc::now);
    for m in &mut rows {
        m.fetched = when;
        m.source = format!("Claude Code · odczyt z {}{note}", local_time(fetched));
    }
    Ok(rows)
}

enum Answer {
    Usage(Value),
    Limited { wait_secs: i64 },
}

fn request() -> Result<Answer> {
    let path = credentials_path().ok_or_else(|| anyhow!("Brak katalogu domowego"))?;
    let raw = Zeroizing::new(
        fs::read(&path).map_err(|_| anyhow!("Nie można odczytać logowania Claude Code"))?,
    );
    let v: Value = serde_json::from_slice(&raw)
        .map_err(|_| anyhow!("Nieznany format logowania Claude Code"))?;
    let token = token_from(&v, Utc::now().timestamp_millis())?;
    let r = providers::client()?
        .get(USAGE_URL)
        .bearer_auth(token.as_str())
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .map_err(|_| anyhow!("Brak połączenia z Claude"))?;
    let status = r.status().as_u16();
    if status == 429 {
        let wait_secs = r
            .headers()
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.trim().parse::<i64>().ok())
            .filter(|s| (1..=24 * 3600).contains(s))
            .unwrap_or(DEFAULT_BACKOFF_SECS);
        return Ok(Answer::Limited { wait_secs });
    }
    if status == 401 {
        bail!(EXPIRED);
    }
    ensure!(
        (200..300).contains(&status),
        "HTTP {status}: błąd usługi Claude"
    );
    Ok(Answer::Usage(
        r.json()
            .map_err(|_| anyhow!("Nieznany format limitów Claude"))?,
    ))
}

/// Claude limits, asking the server at most every few minutes and backing
/// off after a 429. The last answer survives restarts in the data folder.
pub fn read(root: &Path) -> Result<Vec<Metric>> {
    let now = Utc::now().timestamp();
    let mut cache = load_cache(root);
    if !cache.is_object() {
        cache = json!({});
    }
    let blocked = cache["blocked_until"].as_i64().filter(|b| now < *b);
    if plan(cache["requested"].as_i64(), blocked, now) == Plan::Cached {
        let note = blocked.map(limited_note).unwrap_or_default();
        if let Ok(rows) = cached_rows(&cache, &note) {
            return Ok(rows);
        }
        if let Some(b) = blocked {
            bail!(
                "Claude chwilowo ogranicza zapytania. Kolejna próba po {}.",
                local_time(b)
            );
        }
    }
    cache["requested"] = json!(now);
    let answer = request();
    match answer {
        Ok(Answer::Usage(usage)) => {
            let mut rows = providers::claude(&usage)?;
            cache["usage"] = usage;
            cache["fetched"] = json!(now);
            cache["blocked_until"] = Value::Null;
            save_cache(root, &cache);
            for m in &mut rows {
                m.source = "Claude Code (oficjalne logowanie konta Claude)".into();
            }
            Ok(rows)
        }
        Ok(Answer::Limited { wait_secs }) => {
            let until = now + wait_secs;
            cache["blocked_until"] = json!(until);
            save_cache(root, &cache);
            cached_rows(&cache, &limited_note(until)).map_err(|_| {
                anyhow!(
                    "Claude chwilowo ogranicza zapytania. Kolejna próba po {}.",
                    local_time(until)
                )
            })
        }
        Err(e) => {
            save_cache(root, &cache);
            Err(e)
        }
    }
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
    fn requests_are_spaced_out_and_paused_after_429() {
        let now = 10_000;
        assert_eq!(plan(None, None, now), Plan::Request);
        assert_eq!(plan(Some(now - 60), None, now), Plan::Cached);
        assert_eq!(
            plan(Some(now - MIN_REQUEST_GAP_SECS), None, now),
            Plan::Request
        );
        assert_eq!(plan(Some(now - 3600), Some(now + 10), now), Plan::Cached);
        assert_eq!(plan(Some(now - 3600), Some(now - 1), now), Plan::Request);
    }
    #[test]
    fn last_answer_is_shown_with_its_own_time() {
        let cache = json!({"fetched": 1_000, "usage": {"five_hour":{"utilization":20.0}}});
        let rows = cached_rows(&cache, "; x").unwrap();
        assert_eq!(rows[0].percent, Some(20.));
        assert_eq!(rows[0].fetched.timestamp(), 1_000);
        assert!(rows[0].source.ends_with("; x"));
        assert!(cached_rows(&json!({}), "").is_err());
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
