use crate::model::*;
use anyhow::{anyhow, bail, ensure, Result};
use chrono::{Datelike, TimeZone, Utc};
use reqwest::blocking::{Client, RequestBuilder};
use rust_decimal::Decimal;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::io::Read;
use std::{collections::HashSet, time::Duration};

pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("AI-Gauge/0.2.0")
        .build()?)
}
fn response(r: RequestBuilder) -> Result<Value> {
    let r = r
        .send()
        .map_err(|_| anyhow!("Brak polaczenia, blad TLS lub przekroczony czas odpowiedzi"))?;
    let status = r.status();
    if !status.is_success() {
        bail!(
            "HTTP {}: {}",
            status.as_u16(),
            match status.as_u16() {
                401 => "sprawdz klucz lub zaloguj ponownie",
                403 => "brak uprawnien (raporty kosztow wymagaja klucza ADMIN)",
                429 => "limit zapytan; odczekaj",
                _ => "blad uslugi",
            }
        )
    }
    let mut bytes = Vec::new();
    r.take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| anyhow!("Nie mozna odczytac odpowiedzi"))?;
    ensure!(bytes.len() < 8 * 1024 * 1024, "Odpowiedz za duza");
    serde_json::from_slice(&bytes).map_err(|_| anyhow!("Nieprawidlowy JSON uslugi"))
}
pub fn api(s: Service, key: &str, cfg: &ServiceConfig) -> Result<Vec<Metric>> {
    validate_key(key)?;
    let client = client()?;
    if s == Service::Deepseek {
        return deepseek(
            &response(
                client
                    .get("https://api.deepseek.com/user/balance")
                    .bearer_auth(key),
            )?,
            cfg,
        );
    }
    let now = Utc::now();
    let start = Utc
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .unwrap();
    let budget = decimal(&cfg.budget)?;
    let mut total = Decimal::ZERO;
    let mut seen = HashSet::new();
    let mut cursor = None;
    for _ in 0..100 {
        let req = match s {
            Service::Openai => client
                .get("https://api.openai.com/v1/organization/costs")
                .bearer_auth(key)
                .query(&[
                    ("start_time", start.timestamp().to_string()),
                    ("end_time", now.timestamp().to_string()),
                    ("bucket_width", "1d".into()),
                    ("limit", "31".into()),
                ]),
            Service::Anthropic => client
                .get("https://api.anthropic.com/v1/organizations/cost_report")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .query(&[
                    ("starting_at", start.to_rfc3339()),
                    ("ending_at", now.to_rfc3339()),
                    ("limit", "31".into()),
                ]),
            _ => bail!("Nie jest to usluga API"),
        };
        let req = if let Some(ref p) = cursor {
            req.query(&[("page", p)])
        } else {
            req
        };
        let v = response(req)?;
        total += costs_page(s, &v)?;
        if !v.get("has_more").and_then(Value::as_bool).unwrap_or(false) {
            return Ok(vec![cost_metric(s, total, budget)?]);
        }
        let next = v
            .get("next_page")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| anyhow!("Niepelny raport: brak kursora"))?;
        ensure!(
            seen.insert(next.to_owned()),
            "Niepelny raport: zapetlona paginacja"
        );
        cursor = Some(next.to_owned());
    }
    bail!("Raport przekroczyl limit stron; nie pokazujemy czesciowego kosztu")
}
pub fn costs_page(s: Service, v: &Value) -> Result<Decimal> {
    let mut total = Decimal::ZERO;
    for bucket in v["data"]
        .as_array()
        .ok_or_else(|| anyhow!("Brak danych kosztow"))?
    {
        for row in bucket["results"]
            .as_array()
            .ok_or_else(|| anyhow!("Nieprawidlowy koszyk kosztow"))?
        {
            if s == Service::Openai {
                ensure!(
                    row["amount"]["currency"]
                        .as_str()
                        .unwrap_or("usd")
                        .eq_ignore_ascii_case("usd"),
                    "Waluta raportu inna niz USD"
                );
                total += numeric(&row["amount"]["value"])?;
            } else {
                ensure!(
                    row["currency"]
                        .as_str()
                        .unwrap_or("USD")
                        .eq_ignore_ascii_case("USD"),
                    "Waluta raportu inna niz USD"
                );
                total += numeric(&row["amount"])? / Decimal::from(100);
            }
        }
    }
    Ok(total)
}
pub fn numeric(v: &Value) -> Result<Decimal> {
    match v {
        Value::String(s) => decimal(s),
        Value::Number(n) => decimal(&n.to_string()),
        _ => bail!("Brak kwoty w odpowiedzi"),
    }
}
pub fn deepseek(v: &Value, c: &ServiceConfig) -> Result<Vec<Metric>> {
    let mut rows = vec![];
    let base = decimal(&c.budget)?;
    for r in v["balance_infos"]
        .as_array()
        .ok_or_else(|| anyhow!("Brak informacji o saldzie"))?
    {
        let currency = r["currency"]
            .as_str()
            .ok_or_else(|| anyhow!("Brak waluty"))?;
        ensure!(["USD", "CNY"].contains(&currency), "Nieznana waluta");
        let balance = numeric(&r["total_balance"])?;
        ensure!(balance >= Decimal::ZERO, "Nieprawidlowe saldo");
        let p = if c.currency == currency {
            ratio((base - balance).max(Decimal::ZERO), base)
        } else {
            None
        };
        let mut m = Metric::new(
            Service::Deepseek,
            currency,
            format!("Saldo {currency}"),
            format!("{balance:.2} {currency}"),
            p,
            "Oficjalne API salda; procent wzgledem wlasnej kwoty odniesienia",
        );
        m.reset = format!(
            "Promocyjne: {}; doladowane: {}",
            r["granted_balance"].as_str().unwrap_or("?"),
            r["topped_up_balance"].as_str().unwrap_or("?")
        );
        rows.push(m);
    }
    ensure!(!rows.is_empty(), "Usluga nie zwrocila salda");
    Ok(rows)
}
pub fn codex(v: &Value) -> Result<Vec<Metric>> {
    let map = v["rateLimitsByLimitId"]
        .as_object()
        .cloned()
        .unwrap_or_else(|| serde_json::Map::from_iter([("codex".into(), v["rateLimits"].clone())]));
    let mut rows = vec![];
    for (id, b) in map {
        for k in ["primary", "secondary"] {
            if b[k].is_null() {
                continue;
            }
            let p = percent(
                b[k]["usedPercent"]
                    .as_f64()
                    .ok_or_else(|| anyhow!("Brak procentu Codex"))?,
            )?;
            let mins = b[k]["windowDurationMins"].as_i64().unwrap_or(0);
            let name = b["limitName"].as_str().unwrap_or(&id);
            let mut m = Metric::new(
                Service::Chatgpt,
                format!("codex/{id}/{k}"),
                format!("Codex: {name} / {mins} min"),
                format!("{p:.0}% wykorzystano"),
                Some(p),
                "Oficjalny Codex app-server. To limit Codex, nie wszystkich rozmow ChatGPT.",
            );
            if let Some(t) = b[k]["resetsAt"].as_i64() {
                if let Some(t) = Utc.timestamp_opt(t, 0).single() {
                    m.reset = format!(
                        "Reset {}",
                        t.with_timezone(&chrono::Local).format("%d.%m %H:%M")
                    );
                }
            }
            rows.push(m)
        }
        if let Some(balance) = b["credits"]["balance"].as_str() {
            rows.push(Metric::new(
                Service::Chatgpt,
                format!("codex/{id}/credits"),
                "Kredyty Codex",
                balance,
                None,
                "Codex app-server",
            ))
        }
    }
    ensure!(!rows.is_empty(), "Brak licznikow Codex; sprawdz logowanie");
    Ok(rows)
}
pub fn claude(v: &Value) -> Result<Vec<Metric>> {
    let mut out = vec![];
    for (k, value) in v
        .as_object()
        .ok_or_else(|| anyhow!("Nieznany format limitow Claude"))?
    {
        if !value.is_object()
            || value.get("utilization").is_none()
            || value["utilization"].is_null()
        {
            continue;
        }
        let label = match k.as_str() {
            "five_hour" => "Sesja 5 h".into(),
            "seven_day" => "Tydzien".into(),
            "seven_day_sonnet" => "Sonnet / tydzien".into(),
            "seven_day_opus" => "Opus / tydzien".into(),
            "seven_day_oauth_apps" => "Aplikacje OAuth / tydzien".into(),
            "extra_usage" => "Dodatkowe uzycie".into(),
            _ => k.replace('_', " "),
        };
        let p = percent(
            v[k]["utilization"]
                .as_f64()
                .ok_or_else(|| anyhow!("Nieznany format limitow Claude"))?,
        )?;
        let mut m = Metric::new(
            Service::Claude,
            k,
            label,
            format!("{p:.0}% wykorzystano"),
            Some(p),
            "Panel Claude Usage (wewnetrzny interfejs strony)",
        );
        if let Some(r) = v[k]["resets_at"].as_str() {
            m.reset = DateTime::parse_from_rfc3339(r)
                .map(|d| {
                    format!(
                        "Reset {}",
                        d.with_timezone(&chrono::Local).format("%d.%m %H:%M")
                    )
                })
                .unwrap_or_else(|_| "Reset: brak poprawnej daty".into());
        }
        out.push(m)
    }
    ensure!(
        !out.is_empty(),
        "Konto nie zwrocilo rozpoznanych licznikow Claude"
    );
    Ok(out)
}
use chrono::DateTime;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn currencies_and_cents() {
        let a = json!({"data":[{"results":[{"amount":"1234.5","currency":"USD"}]}]});
        assert_eq!(
            costs_page(Service::Anthropic, &a).unwrap(),
            decimal("12.345").unwrap()
        );
        let bad = json!({"data":[{"results":[{"amount":{"currency":"eur","value":1}}]}]});
        assert!(costs_page(Service::Openai, &bad).is_err());
    }
    #[test]
    fn two_currencies() {
        let v = json!({"balance_infos":[{"currency":"USD","total_balance":"15"},{"currency":"CNY","total_balance":"80"}]});
        let c = ServiceConfig {
            budget: "20".into(),
            ..Default::default()
        };
        let m = deepseek(&v, &c).unwrap();
        assert_eq!(m[0].percent, Some(25.));
        assert_eq!(m[1].percent, None);
    }
    #[test]
    fn multiple_codex_windows() {
        let m=codex(&json!({"rateLimitsByLimitId":{"x":{"primary":{"usedPercent":0,"windowDurationMins":300},"secondary":{"usedPercent":82,"windowDurationMins":10080}}}})).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].percent, Some(0.));
    }
    #[test]
    fn empty_is_not_zero() {
        assert!(claude(&json!({})).is_err());
        assert!(codex(&json!({})).is_err());
        assert!(deepseek(&json!({}), &Default::default()).is_err());
    }
}
