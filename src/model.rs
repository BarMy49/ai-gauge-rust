use anyhow::{bail, ensure, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Service {
    Chatgpt,
    Openai,
    Deepseek,
    Claude,
    Anthropic,
}

impl Service {
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }
    pub const ALL: [Self; 5] = [
        Self::Chatgpt,
        Self::Openai,
        Self::Deepseek,
        Self::Claude,
        Self::Anthropic,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Chatgpt => "chatgpt",
            Self::Openai => "openai",
            Self::Deepseek => "deepseek",
            Self::Claude => "claude",
            Self::Anthropic => "anthropic",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Chatgpt => "ChatGPT / Codex",
            Self::Openai => "OpenAI API",
            Self::Deepseek => "DeepSeek API",
            Self::Claude => "Claude Code / Claude",
            Self::Anthropic => "Anthropic API",
        }
    }
    pub fn api(self) -> bool {
        matches!(self, Self::Openai | Self::Deepseek | Self::Anthropic)
    }
    pub fn url(self) -> &'static str {
        match self {
            Self::Chatgpt => "https://chatgpt.com/codex/settings/usage",
            Self::Openai => "https://platform.openai.com/usage",
            Self::Deepseek => "https://platform.deepseek.com/",
            Self::Claude => "https://claude.ai/settings/usage",
            Self::Anthropic => "https://platform.claude.com/usage",
        }
    }
    pub fn host(self) -> &'static str {
        match self {
            Self::Chatgpt => "chatgpt.com",
            Self::Claude => "claude.ai",
            _ => "",
        }
    }
    pub fn help(self) -> &'static str {
        match self {
        Self::Chatgpt=>"Odczyt przez oficjalny Codex CLI (zainstaluj codex i zaloguj się: codex login, albo użyj przycisku logowania). Dotyczy limitów Codex na tym koncie, nie wszystkich rozmów ChatGPT.",
        Self::Openai=>"Wymagany klucz ADMIN organizacji. Pokazuje koszt calej organizacji od poczatku miesiaca UTC. Procent jest liczony wzgledem Twojego budzetu; to nie saldo przedplaty ani RPM/TPM.",
        Self::Deepseek=>"Zwykly klucz API. Pokazuje rzeczywiste saldo USD/CNY. Kwota odniesienia sluzy do obliczenia procentu zuzycia; po doladowaniu mozesz ja zmienic.",
        Self::Claude=>"Najpewniej działa przez Claude Code: jeśli jest zalogowany na tym komputerze (claude, potem /login), limity odczytamy z jego oficjalnego logowania. Bez Claude Code zaloguj się w osobnym zwykłym oknie Chrome lub Edge, zamknij je i naciśnij Odśwież. Odczyt sesji i tygodnia z panelu Usage. Integracja korzysta z wewnetrznego interfejsu strony i moze wymagac aktualizacji.",
        Self::Anthropic=>"Wymagany klucz ADMIN. Pokazuje koszt organizacji od poczatku miesiaca UTC, takze Claude Code rozliczane tym API. Budzet jest Twoim progiem; nie odczytujemy salda przedplaty.",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceConfig {
    pub browser: String,
    pub enabled: bool,
    pub budget: String,
    pub currency: String,
    pub browser_auth: bool,
    pub device_login: bool,
    pub organization: String,
}
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            browser: "auto".into(),
            enabled: false,
            budget: "0".into(),
            currency: "USD".into(),
            browser_auth: false,
            device_login: false,
            organization: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub service_order: Vec<Service>,
    pub hidden_services: Vec<Service>,
    pub appearance: String,
    pub text_scale: u16,
    pub interval: u64,
    pub hidden: bool,
    pub show_remaining: bool,
    /// Show the part of the reference amount that is left for metered APIs
    /// billed in currency (DeepSeek balance, OpenAI/Anthropic budget).
    pub show_api_remaining: bool,
    pub selected: String,
    pub services: BTreeMap<Service, ServiceConfig>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            service_order: Service::ALL.to_vec(),
            hidden_services: vec![],
            appearance: "system".into(),
            text_scale: 100,
            interval: 300,
            hidden: false,
            show_remaining: false,
            show_api_remaining: false,
            selected: String::new(),
            services: Service::ALL
                .into_iter()
                .map(|s| {
                    (
                        s,
                        ServiceConfig {
                            browser_auth: s == Service::Chatgpt,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        }
    }
}
/// Services that existed in older versions and were removed. Their entries
/// are dropped from settings.json before it is parsed.
const REMOVED_SERVICES: [&str; 1] = ["gemini"];
pub fn drop_removed_services(v: &mut serde_json::Value) {
    for list in ["service_order", "hidden_services"] {
        if let Some(a) = v[list].as_array_mut() {
            a.retain(|s| !s.as_str().is_some_and(|s| REMOVED_SERVICES.contains(&s)));
        }
    }
    if let Some(m) = v["services"].as_object_mut() {
        m.retain(|k, _| !REMOVED_SERVICES.contains(&k.as_str()));
    }
    if let Some(sel) = v["selected"].as_str() {
        if REMOVED_SERVICES
            .iter()
            .any(|r| sel.split(':').next() == Some(r))
        {
            v["selected"] = serde_json::Value::String(String::new());
        }
    }
}
impl Config {
    pub fn normalize_layout(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.service_order.retain(|s| seen.insert(*s));
        for service in Service::ALL {
            if seen.insert(service) {
                self.service_order.push(service);
            }
        }
        self.hidden_services.sort();
        self.hidden_services.dedup();
        if !["system", "light", "dark"].contains(&self.appearance.as_str()) {
            self.appearance = "system".into();
        }
        if ![100, 125, 150, 200].contains(&self.text_scale) {
            self.text_scale = 100;
        }
    }
    pub fn validate(&mut self) -> Result<()> {
        self.normalize_layout();
        self.interval = self.interval.clamp(60, 3600);
        for s in Service::ALL {
            let c = self.services.entry(s).or_default();
            if !["auto", "chrome", "edge"].contains(&c.browser.as_str()) {
                c.browser = "auto".into();
            }
            ensure!(
                decimal(&c.budget)? >= Decimal::ZERO,
                "Budzet nie moze byc ujemny"
            );
            ensure!(
                matches!(c.currency.as_str(), "USD" | "CNY"),
                "Nieprawidlowa waluta"
            );
        }
        Ok(())
    }
}

/// Presentation preference: show what is left instead of what is used.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Remaining {
    /// Subscription limits (ChatGPT/Codex, Claude).
    pub limits: bool,
    /// Currency-metered APIs measured against a reference amount.
    pub api: bool,
}
impl From<bool> for Remaining {
    fn from(limits: bool) -> Self {
        Self { limits, api: false }
    }
}
impl Config {
    pub fn remaining(&self) -> Remaining {
        Remaining {
            limits: self.show_remaining,
            api: self.show_api_remaining,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Metric {
    pub service: Service,
    pub key: String,
    pub label: String,
    pub value: String,
    pub percent: Option<f64>,
    pub reset: String,
    pub source: String,
    pub fetched: DateTime<Utc>,
    pub error: Option<String>,
}
impl Metric {
    pub fn new(
        service: Service,
        key: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
        percent: Option<f64>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            service,
            key: key.into(),
            label: label.into(),
            value: value.into(),
            percent,
            reset: String::new(),
            source: source.into(),
            fetched: Utc::now(),
            error: None,
        }
    }
    pub fn identity(&self) -> String {
        format!("{}:{}", self.service.id(), self.key)
    }
    pub fn current(&self, interval: u64) -> bool {
        self.error.is_none()
            && Utc::now().signed_duration_since(self.fetched).num_seconds()
                <= (interval * 2).max(if self.service == Service::Claude {
                    // Claude limits are asked for at most every 5 minutes.
                    660
                } else {
                    90
                }) as i64
    }
    pub fn shows_remaining(&self, show_remaining: impl Into<Remaining>) -> bool {
        let r = show_remaining.into();
        if self.service.api() {
            r.api
        } else {
            r.limits && matches!(self.service, Service::Chatgpt | Service::Claude)
        }
    }
    pub fn display_percent(&self, show_remaining: impl Into<Remaining>) -> Option<f64> {
        let show_remaining = show_remaining.into();
        self.percent.map(|p| {
            if self.shows_remaining(show_remaining) {
                (100. - p).clamp(0., 100.)
            } else {
                p
            }
        })
    }
    pub fn percentage_label(&self, show_remaining: impl Into<Remaining>) -> String {
        let show_remaining = show_remaining.into();
        self.display_percent(show_remaining)
            .map(|p| {
                format!(
                    "{p:.0}% {}",
                    if self.shows_remaining(show_remaining) {
                        "pozostało"
                    } else {
                        "wykorzystano"
                    }
                )
            })
            .unwrap_or_default()
    }
    pub fn display_value(&self, show_remaining: impl Into<Remaining>) -> String {
        let show_remaining = show_remaining.into();
        if !self.service.api() && self.percent.is_some() {
            self.percentage_label(show_remaining)
        } else {
            self.value.clone()
        }
    }
    pub fn line(&self, interval: u64, show_remaining: impl Into<Remaining>) -> String {
        let show_remaining = show_remaining.into();
        let value = self.display_value(show_remaining);
        let p = Some(self.percentage_label(show_remaining))
            .filter(|p| !p.is_empty() && !value.contains('%'))
            .map(|p| format!(" ({p})"))
            .unwrap_or_default();
        format!(
            "{} | {}{}{}",
            self.label,
            value,
            p,
            if self.current(interval) {
                ""
            } else {
                " [nieaktualne]"
            }
        )
    }
}
pub fn decimal(s: &str) -> Result<Decimal> {
    Decimal::from_str(s.trim())
        .map_err(|_| anyhow::anyhow!("Nieprawidlowa kwota: wpisz liczbe z kropka dziesietna"))
}
pub fn ratio(a: Decimal, b: Decimal) -> Option<f64> {
    if b <= Decimal::ZERO {
        return None;
    }
    (a / b * Decimal::from(100)).to_string().parse().ok()
}
pub fn percent(v: f64) -> Result<f64> {
    ensure!(
        v.is_finite() && (0.0..=100.0).contains(&v),
        "Nieprawidlowy procent uslugi"
    );
    Ok(v)
}
pub fn cost_metric(s: Service, cost: Decimal, budget: Decimal) -> Result<Metric> {
    ensure!(cost >= Decimal::ZERO, "Nieprawidlowy koszt");
    let mut m = Metric::new(
        s,
        "month",
        "Koszt miesiaca UTC",
        format!("{cost:.2} USD"),
        ratio(cost, budget),
        "Raport organizacji; mozliwe opoznienie. Procent wlasnego budzetu, nie saldo przedplaty.",
    );
    if budget > Decimal::ZERO {
        m.reset = format!(
            "Budzet {budget:.2} USD; pozostalo {:.2} USD",
            (budget - cost).max(Decimal::ZERO)
        );
    }
    Ok(m)
}
pub fn validate_key(s: &str) -> Result<()> {
    if s.is_empty() {
        bail!("Wpisz klucz API w ustawieniach")
    }
    ensure!(
        s.bytes().all(|b| (33..=126).contains(&b)),
        "Klucz zawiera spacje lub niedozwolone znaki"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_configs_and_partial_layouts_are_normalized() {
        let mut old: Config =
            serde_json::from_str(r#"{"interval":300,"selected":"claude:week"}"#).unwrap();
        old.validate().unwrap();
        assert_eq!(old.service_order, Service::ALL);
        assert!(old.hidden_services.is_empty());
        assert_eq!(old.appearance, "system");
        assert_eq!(old.text_scale, 100);
        old.service_order = vec![Service::Deepseek, Service::Deepseek, Service::Claude];
        old.hidden_services = vec![Service::Openai, Service::Openai];
        old.normalize_layout();
        assert_eq!(
            &old.service_order[..2],
            &[Service::Deepseek, Service::Claude]
        );
        assert_eq!(old.service_order.len(), 5);
        assert_eq!(old.hidden_services, vec![Service::Openai]);
        assert_eq!(old.selected, "claude:week");
    }
    #[test]
    fn budgets_and_percent() {
        let m = cost_metric(
            Service::Openai,
            decimal("25").unwrap(),
            decimal("20").unwrap(),
        )
        .unwrap();
        assert_eq!(m.percent, Some(125.0));
        assert!(percent(f64::NAN).is_err());
        assert!(percent(101.).is_err());
        assert_eq!(ratio(decimal("2").unwrap(), Decimal::ZERO), None);
    }
    #[test]
    fn config_defaults() {
        let mut c: Config = serde_json::from_str("{}").unwrap();
        c.validate().unwrap();
        assert_eq!(c.services.len(), 5);
        assert_eq!(c.interval, 300);
        assert!(!c.show_remaining);
        assert!(!c.show_api_remaining);
    }
    #[test]
    fn remaining_limits_are_a_presentation_preference() {
        for service in [Service::Chatgpt, Service::Claude] {
            for (used, remaining) in [(0., 100.), (37., 63.), (37.25, 62.75), (100., 0.)] {
                let m = Metric::new(service, "quota", "Limit", "used", Some(used), "fixture");
                assert_eq!(m.display_percent(true), Some(remaining));
                assert_eq!(m.display_percent(false), Some(used));
                assert_eq!(m.percent, Some(used));
                assert_eq!(m.display_value(true), format!("{remaining:.0}% pozostało"));
                assert_eq!(m.display_value(false), format!("{used:.0}% wykorzystano"));
                assert!(m.line(300, true).contains(&m.display_value(true)));
            }
            let unknown = Metric::new(service, "credits", "Kredyty", "12.50", None, "fixture");
            assert_eq!(unknown.display_percent(true), None);
            assert_eq!(unknown.display_value(true), "12.50");
            assert_eq!(unknown.percentage_label(true), "");
        }
    }
    #[test]
    fn remaining_preference_leaves_api_budgets_and_balances_unchanged() {
        for service in [Service::Openai, Service::Anthropic, Service::Deepseek] {
            let m = Metric::new(
                service,
                "budget",
                "Budżet",
                "25.00 USD",
                Some(125.),
                "fixture",
            );
            assert_eq!(m.display_percent(true), Some(125.));
            assert_eq!(m.display_value(true), "25.00 USD");
            assert_eq!(m.percentage_label(true), m.percentage_label(false));
            assert_eq!(m.line(300, true), m.line(300, false));
        }
    }
    #[test]
    fn api_remaining_shows_what_is_left_of_the_reference_amount() {
        let api = Remaining {
            limits: false,
            api: true,
        };
        for service in [Service::Openai, Service::Anthropic, Service::Deepseek] {
            for (used, left) in [(0., 100.), (30., 70.), (100., 0.), (125., 0.)] {
                let m = Metric::new(service, "b", "Saldo", "7.00 USD", Some(used), "fixture");
                assert_eq!(m.display_percent(api), Some(left));
                assert_eq!(m.percentage_label(api), format!("{left:.0}% pozostało"));
                assert_eq!(m.display_value(api), "7.00 USD");
                assert!(m
                    .line(300, api)
                    .contains(&format!("({left:.0}% pozostało)")));
            }
        }
        // The two preferences are independent.
        let sub = Metric::new(Service::Claude, "w", "Tydzien", "x", Some(30.), "fixture");
        assert_eq!(sub.display_percent(api), Some(30.));
        let key = Metric::new(Service::Deepseek, "b", "Saldo", "x", Some(30.), "fixture");
        assert_eq!(key.display_percent(true), Some(30.));
    }
    #[test]
    fn settings_from_versions_with_gemini_still_load() {
        let mut v = serde_json::json!({
            "service_order":["gemini","claude"],
            "hidden_services":["gemini"],
            "selected":"gemini:week",
            "services":{"gemini":{"enabled":true},"claude":{"enabled":true}}
        });
        drop_removed_services(&mut v);
        let mut c: Config = serde_json::from_value(v).unwrap();
        c.validate().unwrap();
        assert_eq!(c.service_order[0], Service::Claude);
        assert!(c.hidden_services.is_empty());
        assert_eq!(c.selected, "");
        assert!(c.services[&Service::Claude].enabled);
    }
    #[test]
    fn bad_key_never_echoed() {
        let secret = "private\nbad";
        let e = validate_key(secret).unwrap_err().to_string();
        assert!(!e.contains("private"));
    }
}
