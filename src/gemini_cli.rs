//! Gemini quotas read through Gemini CLI's own sign-in.
//! Gemini CLI keeps a Google sign-in in ~/.gemini/oauth_creds.json. The file is
//! only read, never written. An expired access token is renewed in memory with
//! Gemini CLI's installed-app client (Google refresh tokens do not rotate, so
//! Gemini CLI keeps working). The same quota endpoint backs its /stats view.
use crate::{model::*, providers};
use anyhow::{anyhow, ensure, Result};
use chrono::DateTime;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
use zeroize::Zeroizing;

const ENDPOINT: &str = "https://cloudcode-pa.googleapis.com/v1internal";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
// Public installed-app client of Gemini CLI (not a secret, see its source).
const CLIENT_ID: &str = "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "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";
const RELOGIN: &str = "Zaloguj Gemini CLI: uruchom gemini i wybierz logowanie kontem Google.";

fn dir() -> Option<PathBuf> {
    match std::env::var_os("GEMINI_CLI_HOME") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d).join(".gemini")),
        _ => Some(directories::BaseDirs::new()?.home_dir().join(".gemini")),
    }
}
fn credentials_path() -> Option<PathBuf> {
    Some(dir()?.join("oauth_creds.json"))
}

/// True when Gemini CLI has stored a Google sign-in on this computer.
pub fn available() -> bool {
    credentials_path().is_some_and(|p| p.is_file())
}

/// The stored access token while it is still valid for at least a minute.
fn fresh_token(v: &Value, now_ms: i64) -> Option<Zeroizing<String>> {
    let token = v["access_token"].as_str().filter(|t| !t.is_empty())?;
    let exp = v["expiry_date"].as_i64()?;
    (exp > now_ms + 60_000).then(|| Zeroizing::new(token.to_owned()))
}

fn token(v: &Value) -> Result<Zeroizing<String>> {
    if let Some(t) = fresh_token(v, chrono::Utc::now().timestamp_millis()) {
        return Ok(t);
    }
    let refresh = v["refresh_token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| anyhow!(RELOGIN))?;
    let r = providers::client()?
        .post(TOKEN_URL)
        .form(&[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
        ])
        .send()
        .map_err(|_| anyhow!("Brak połączenia z kontem Google"))?;
    ensure!(r.status().is_success(), RELOGIN);
    let body: Value = r
        .json()
        .map_err(|_| anyhow!("Nieznana odpowiedź logowania Google"))?;
    let t = body["access_token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .ok_or_else(|| anyhow!(RELOGIN))?;
    Ok(Zeroizing::new(t.to_owned()))
}

fn post(token: &str, method: &str, body: Value) -> Result<Value> {
    providers::response(
        providers::client()?
            .post(format!("{ENDPOINT}:{method}"))
            .bearer_auth(token)
            .json(&body),
    )
}

fn project_from(load: &Value) -> Option<String> {
    let p = &load["cloudaicompanionProject"];
    p.as_str()
        .or_else(|| p["id"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub fn read() -> Result<Vec<Metric>> {
    let path = credentials_path().ok_or_else(|| anyhow!("Brak katalogu domowego"))?;
    let raw = Zeroizing::new(fs::read(&path).map_err(|_| anyhow!(RELOGIN))?);
    let v: Value = serde_json::from_slice(&raw)
        .map_err(|_| anyhow!("Nieznany format logowania Gemini CLI"))?;
    let token = token(&v)?;
    let project = match std::env::var("GOOGLE_CLOUD_PROJECT")
        .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT_ID"))
    {
        Ok(p) if !p.is_empty() => p,
        _ => {
            let load = post(
                &token,
                "loadCodeAssist",
                json!({"metadata":{"ideType":"IDE_UNSPECIFIED","platform":"PLATFORM_UNSPECIFIED","pluginType":"GEMINI"}}),
            )?;
            project_from(&load).ok_or_else(|| {
                anyhow!("Konto nie ma jeszcze projektu Gemini Code Assist. Uruchom raz gemini i zadaj pytanie, potem naciśnij Odśwież.")
            })?
        }
    };
    let quota = post(&token, "retrieveUserQuota", json!({ "project": project }))?;
    quota_metrics(&quota)
}

pub fn quota_metrics(v: &Value) -> Result<Vec<Metric>> {
    let mut out: Vec<Metric> = vec![];
    for b in v["buckets"].as_array().into_iter().flatten() {
        let (Some(model), Some(left)) = (b["modelId"].as_str(), b["remainingFraction"].as_f64())
        else {
            continue;
        };
        let used = percent(((1.0 - left) * 100.0).clamp(0.0, 100.0))?;
        let kind = b["tokenType"].as_str().unwrap_or("");
        let key = format!("cli/{model}/{kind}");
        if out.iter().any(|m| m.key == key) {
            continue;
        }
        let label = if kind.is_empty() || kind.eq_ignore_ascii_case("REQUESTS") {
            model.to_owned()
        } else {
            format!("{model} / {}", kind.to_lowercase())
        };
        let mut m = Metric::new(
            Service::Gemini,
            key,
            label,
            format!("{used:.0}% wykorzystano"),
            Some(used),
            "Gemini CLI (oficjalne logowanie konta Google)",
        );
        if let Some(r) = b["resetTime"].as_str() {
            m.reset = DateTime::parse_from_rfc3339(r)
                .map(|d| {
                    format!(
                        "Reset {}",
                        d.with_timezone(&chrono::Local).format("%d.%m %H:%M")
                    )
                })
                .unwrap_or_default();
        }
        out.push(m);
    }
    ensure!(
        !out.is_empty(),
        "Gemini CLI nie zwrócił limitów dla tego konta"
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quota_buckets_become_used_percentages() {
        let v = json!({"buckets":[
            {"modelId":"gemini-2.5-pro","remainingFraction":0.75,"resetTime":"2026-11-28T00:00:00Z","tokenType":"REQUESTS"},
            {"modelId":"gemini-2.5-flash","remainingFraction":1.0},
            {"modelId":"gemini-2.5-flash","remainingFraction":1.0},
            {"remainingFraction":0.5}
        ]});
        let rows = quota_metrics(&v).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].percent, Some(25.));
        assert_eq!(rows[0].label, "gemini-2.5-pro");
        assert!(rows[0].reset.starts_with("Reset "));
        assert_eq!(rows[1].percent, Some(0.));
        assert!(quota_metrics(&json!({"buckets":[]})).is_err());
    }
    #[test]
    fn stored_token_is_used_only_while_valid() {
        let now = 1_000_000;
        let v = json!({"access_token":"a","expiry_date":now+3_600_000});
        assert_eq!(fresh_token(&v, now).unwrap().as_str(), "a");
        let v = json!({"access_token":"a","expiry_date":now+1_000});
        assert!(fresh_token(&v, now).is_none());
    }
    #[test]
    fn project_id_in_either_shape() {
        assert_eq!(
            project_from(&json!({"cloudaicompanionProject":"p-1"})).as_deref(),
            Some("p-1")
        );
        assert_eq!(
            project_from(&json!({"cloudaicompanionProject":{"id":"p-2"}})).as_deref(),
            Some("p-2")
        );
        assert!(project_from(&json!({})).is_none());
    }
}
