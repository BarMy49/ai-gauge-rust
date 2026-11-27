use crate::{browser::BrowserPool, claude_code, codex, model::*, providers, store::Store};
use anyhow::{ensure, Result};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Snapshot {
    pub busy: HashSet<Service>,
    pub gemini_state: GeminiState,
    pub auth_message: String,
    pub auth_url: String,
    pub auth_code: String,
    pub account_label: String,
    pub auth_busy: bool,
    pub config: Config,
    pub metrics: Vec<Metric>,
    pub status: BTreeMap<Service, String>,
    pub message: String,
    pub revision: u64,
}
struct Data {
    gemini_state: GeminiState,
    auth_message: String,
    auth_url: String,
    auth_code: String,
    account_label: String,
    auth_busy: bool,
    config: Config,
    metrics: BTreeMap<String, Metric>,
    status: BTreeMap<Service, String>,
    busy: HashSet<Service>,
    last: BTreeMap<Service, Instant>,
    generation: BTreeMap<Service, u64>,
    message: String,
    revision: u64,
}
struct Inner {
    data: Mutex<Data>,
    store: Arc<Store>,
    browser: BrowserPool,
    demo: bool,
    login_cancel: Mutex<Option<Arc<AtomicBool>>>,
    show: AtomicBool,
    quit: AtomicBool,
}
#[derive(Clone)]
pub struct Engine(Arc<Inner>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeminiState {
    Disconnected,
    AwaitingSignIn,
    Ready,
    Reading,
    Connected,
    Error,
}
impl GeminiState {
    pub fn paused(self) -> bool {
        matches!(self, Self::AwaitingSignIn | Self::Ready | Self::Error)
    }
}
impl Engine {
    #[cfg(windows)]
    pub fn data_dir(&self) -> std::path::PathBuf {
        self.0.store.root.clone()
    }
    pub fn new(store: Arc<Store>, mut config: Config, demo: bool) -> Self {
        config
            .services
            .get_mut(&Service::Chatgpt)
            .unwrap()
            .browser_auth = false;
        if demo {
            for c in config.services.values_mut() {
                c.enabled = true;
            }
            config.selected = "chatgpt:demo-5h".into();
        }
        let mut data = Data {
            gemini_state: GeminiState::Disconnected,
            auth_message: String::new(),
            auth_url: String::new(),
            auth_code: String::new(),
            account_label: String::new(),
            auth_busy: false,
            config,
            metrics: BTreeMap::new(),
            status: Service::ALL
                .into_iter()
                .map(|s| (s, "Nie skonfigurowano".into()))
                .collect(),
            busy: HashSet::new(),
            last: BTreeMap::new(),
            generation: BTreeMap::new(),
            message: if demo {
                "DEMO: dane przykladowe, bez polaczen i zapisu".into()
            } else {
                "Wybierz usluge, zaloguj sie lub wpisz klucz w ustawieniach".into()
            },
            revision: 1,
        };
        if demo {
            for (s, id, label, value, p, reset) in [
                (
                    Service::Chatgpt,
                    "demo-5h",
                    "Codex / sesja 5 h",
                    "37% wykorzystano",
                    37.,
                    "Reset dzis 18:20",
                ),
                (
                    Service::Chatgpt,
                    "demo-week",
                    "Codex / tydzien",
                    "62% wykorzystano",
                    62.,
                    "Reset sroda 09:00",
                ),
                (
                    Service::Openai,
                    "month",
                    "Budzet miesieczny",
                    "8.40 USD",
                    42.,
                    "Budzet wlasny 20 USD",
                ),
                (
                    Service::Deepseek,
                    "USD",
                    "Saldo USD",
                    "13.72 USD",
                    31.4,
                    "Kwota odniesienia 20 USD",
                ),
                (
                    Service::Claude,
                    "five_hour",
                    "Sesja 5 h",
                    "24% wykorzystano",
                    24.,
                    "Reset dzis 19:00",
                ),
                (
                    Service::Claude,
                    "seven_day",
                    "Tydzien",
                    "81% wykorzystano",
                    81.,
                    "Reset za 3 dni",
                ),
                (
                    Service::Anthropic,
                    "month",
                    "Budzet miesieczny",
                    "3.20 USD",
                    16.,
                    "Budzet wlasny 20 USD",
                ),
                (
                    Service::Gemini,
                    "week",
                    "Tydzien",
                    "56% wykorzystano",
                    56.,
                    "Dane demonstracyjne",
                ),
            ] {
                let mut m = Metric::new(s, id, label, value, Some(p), "DEMO");
                m.reset = reset.into();
                data.metrics.insert(m.identity(), m);
                data.status.insert(s, "DEMO".into());
            }
        }
        Self(Arc::new(Inner {
            browser: BrowserPool::new(store.root.clone()),
            store,
            data: Mutex::new(data),
            demo,
            login_cancel: Mutex::new(None),
            show: AtomicBool::new(false),
            quit: AtomicBool::new(false),
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        let d = self.0.data.lock().unwrap();
        Snapshot {
            busy: d.busy.clone(),
            gemini_state: d.gemini_state,
            auth_message: d.auth_message.clone(),
            auth_url: d.auth_url.clone(),
            auth_code: d.auth_code.clone(),
            account_label: d.account_label.clone(),
            auth_busy: d.auth_busy,
            config: d.config.clone(),
            metrics: d.metrics.values().cloned().collect(),
            status: d.status.clone(),
            message: d.message.clone(),
            revision: d.revision,
        }
    }
    pub fn visual_fixture(&self, scenario: &str) {
        if !self.0.demo {
            return;
        }
        let mut d = self.0.data.lock().unwrap();
        match scenario {
            "empty" => d.config.hidden_services = Service::ALL.to_vec(),
            "stale" => {
                for m in d.metrics.values_mut() {
                    m.fetched -= chrono::Duration::hours(1);
                }
            }
            "error" => {
                for m in d.metrics.values_mut() {
                    m.error = Some("Brak połączenia. Spróbuj ponownie.".into());
                }
                for status in d.status.values_mut() {
                    *status = "Błąd połączenia — odśwież ponownie".into();
                }
            }
            "loading" => {
                d.metrics.clear();
                for status in d.status.values_mut() {
                    *status = "Pobieranie danych…".into();
                }
                d.busy = Service::ALL.into_iter().collect();
            }
            "gemini" => {
                d.gemini_state = GeminiState::AwaitingSignIn;
                d.metrics.retain(|_, m| m.service != Service::Gemini);
                d.status
                    .insert(Service::Gemini, "Oczekiwanie na ręczne logowanie".into());
            }
            _ => {}
        }
        d.revision += 1;
    }
    pub fn message(&self, text: impl Into<String>) {
        let mut d = self.0.data.lock().unwrap();
        d.message = text.into();
        d.revision += 1;
    }
    fn preferences(&self, change: impl FnOnce(&mut Config), message: String) {
        let mut d = self.0.data.lock().unwrap();
        let mut config = d.config.clone();
        change(&mut config);
        config.normalize_layout();
        if self.0.demo || self.0.store.save(&config).is_ok() {
            d.config = config;
            d.message = message;
        } else {
            d.message = "Nie można zapisać ustawień".into();
        }
        d.revision += 1;
    }
    pub fn visibility(&self, service: Service, visible: bool) {
        self.preferences(
            |c| {
                c.hidden_services.retain(|s| *s != service);
                if !visible {
                    c.hidden_services.push(service);
                }
            },
            format!(
                "{}: {}",
                service.name(),
                if visible {
                    "widoczna"
                } else {
                    "ukryta; monitorowanie bez zmian"
                }
            ),
        );
    }
    pub fn reorder(&self, service: Service, position: usize) {
        self.preferences(
            |c| {
                c.service_order.retain(|s| *s != service);
                c.service_order
                    .insert(position.min(c.service_order.len()), service);
            },
            format!("{} — pozycja {} z 6", service.name(), position.min(5) + 1),
        );
    }
    pub fn reset_layout(&self) {
        self.preferences(
            |c| {
                c.service_order = Service::ALL.to_vec();
                c.hidden_services.clear();
            },
            "Przywrócono domyślny układ usług".into(),
        );
    }
    pub fn appearance(&self, appearance: String, text_scale: u16) {
        self.preferences(
            |c| {
                c.appearance = appearance;
                c.text_scale = text_scale;
            },
            "Zapisano wygląd aplikacji".into(),
        );
    }
    pub fn continue_gemini(&self) {
        {
            let mut d = self.0.data.lock().unwrap();
            if d.busy.contains(&Service::Gemini) {
                return;
            }
            if self.0.browser.manual_running() {
                d.message = "Zamknij wszystkie okna osobnego profilu Gemini, a następnie wybierz Kontynuuj.".into();
                d.revision += 1;
                return;
            }
            d.gemini_state = GeminiState::Disconnected;
        }
        self.refresh(Service::Gemini);
    }
    pub fn cancel_gemini(&self) {
        let mut d = self.0.data.lock().unwrap();
        if d.busy.contains(&Service::Gemini) {
            return;
        }
        d.gemini_state = GeminiState::Disconnected;
        d.config.services.get_mut(&Service::Gemini).unwrap().enabled = false;
        d.message = "Anulowano połączenie. Możesz zamknąć osobne okno Gemini. Sesja w profilu została zachowana.".into();
        if !self.0.demo {
            let _ = self.0.store.save(&d.config);
        }
        d.revision += 1;
    }
    pub fn pin(&self, id: &str) {
        let mut d = self.0.data.lock().unwrap();
        if let Some(label) = d
            .metrics
            .get(id)
            .filter(|m| m.percent.is_some())
            .map(|m| m.label.clone())
        {
            d.config.selected = id.into();
            d.message = format!("Ikona: {}", label);
            d.revision += 1;
            if !self.0.demo && self.0.store.save(&d.config).is_err() {
                d.message = "Nie mozna zapisac wybranego licznika".into();
            }
        }
    }
    pub fn save(&self, s: Service, cfg: ServiceConfig, key: String, remember: bool) {
        if self.0.demo {
            self.message("DEMO: ustawienia nie sa zapisywane");
            return;
        }
        let this = self.clone();
        std::thread::spawn(move || {
            let result = (|| {
                ensure!(
                    decimal(&cfg.budget)? >= rust_decimal::Decimal::ZERO,
                    "Budzet nie moze byc ujemny"
                );
                if !key.trim().is_empty() {
                    this.0.store.set_key(s, key.trim().to_owned(), remember)?;
                }
                let mut d = this.0.data.lock().unwrap();
                d.config.services.insert(s, cfg);
                *d.generation.entry(s).or_default() += 1;
                d.metrics.retain(|_, m| m.service != s);
                this.0.store.save(&d.config)?;
                d.message = "Zapisano ustawienia uslugi".into();
                d.revision += 1;
                Ok(())
            })();
            if let Err(e) = result {
                this.message(format!("{e}"))
            } else {
                this.refresh(s);
            }
        });
    }
    pub fn general(
        &self,
        interval: u64,
        hidden: bool,
        show_remaining: bool,
        show_api_remaining: bool,
    ) {
        let mut d = self.0.data.lock().unwrap();
        d.config.interval = interval.clamp(60, 3600);
        d.config.hidden = hidden;
        d.config.show_remaining = show_remaining;
        d.config.show_api_remaining = show_api_remaining;
        d.revision += 1;
        d.message = if self.0.demo || self.0.store.save(&d.config).is_ok() {
            "Zapisano opcje aplikacji".into()
        } else {
            "Nie mozna zapisac ustawien".into()
        };
    }
    pub fn delete_key(&self, s: Service) {
        if self.0.demo {
            return;
        }
        let this = self.clone();
        std::thread::spawn(move || match this.0.store.delete_key(s) {
            Ok(()) => {
                let mut d = this.0.data.lock().unwrap();
                *d.generation.entry(s).or_default() += 1;
                d.metrics.retain(|_, m| m.service != s);
                d.status.insert(s, "Usunieto klucz".into());
                d.revision += 1;
                d.message = "Klucz usuniety".into();
            }
            Err(e) => this.message(e.to_string()),
        });
    }
    pub fn tick(&self) {
        if self.0.demo || self.quitting() {
            return;
        }
        let due = {
            let mut d = self.0.data.lock().unwrap();
            if d.gemini_state == GeminiState::AwaitingSignIn
                && !d.busy.contains(&Service::Gemini)
                && !self.0.browser.manual_running()
            {
                d.gemini_state = GeminiState::Ready;
                d.revision += 1;
            }
            Service::ALL
                .into_iter()
                .filter(|s| {
                    d.config.services[s].enabled
                        && !(*s == Service::Gemini && d.gemini_state.paused())
                        && !d.busy.contains(s)
                        && d.last
                            .get(s)
                            .map(|t| t.elapsed() >= Duration::from_secs(d.config.interval))
                            .unwrap_or(true)
                })
                .collect::<Vec<_>>()
        };
        for s in due {
            self.refresh(s)
        }
    }
    pub fn refresh_all(&self) {
        for s in Service::ALL {
            self.refresh(s)
        }
    }
    pub fn refresh(&self, s: Service) {
        self.start(s, false, false)
    }
    pub fn login(&self, s: Service) {
        if s.api() {
            let _ = open::that(s.url());
            return;
        }
        self.start(s, true, false)
    }
    pub fn login_config(&self, s: Service, cfg: ServiceConfig) {
        if self.0.demo {
            self.message("DEMO: brak logowania");
            return;
        }
        if s.api() {
            self.login(s);
            return;
        }
        {
            let mut d = self.0.data.lock().unwrap();
            d.config.services.insert(s, cfg);
            *d.generation.entry(s).or_default() += 1;
            d.revision += 1;
            if self.0.store.save(&d.config).is_err() {
                d.message = "Nie mozna zapisac ustawien logowania".into();
                return;
            }
        }
        self.login(s);
    }
    pub fn logout(&self, s: Service) {
        self.start(s, false, true)
    }
    fn start(&self, s: Service, login: bool, logout: bool) {
        if self.0.demo {
            self.message("DEMO: brak logowania i polaczen z uslugami");
            return;
        }
        let (cfg, generation) = {
            let mut d = self.0.data.lock().unwrap();
            if !login && !logout && !d.config.services[&s].enabled {
                return;
            }
            if d.busy.contains(&s) {
                d.message = "Usluga jest zajeta; dokoncz logowanie lub poczekaj na odczyt".into();
                d.revision += 1;
                return;
            }
            if s == Service::Gemini {
                if !login && !logout && d.gemini_state.paused() {
                    return;
                }
                d.gemini_state = if login {
                    GeminiState::AwaitingSignIn
                } else if logout {
                    GeminiState::Disconnected
                } else {
                    GeminiState::Reading
                };
            }
            if s == Service::Chatgpt && login {
                d.auth_busy = true;
                d.account_label.clear();
                d.auth_message = "Przygotowywanie logowania…".into();
                d.auth_url.clear();
                d.auth_code.clear();
            }
            d.busy.insert(s);
            d.last.insert(s, Instant::now());
            d.status.insert(
                s,
                if login {
                    "Dokoncz logowanie w przegladarce"
                } else if logout {
                    "Wylogowywanie..."
                } else {
                    "Odswiezanie..."
                }
                .into(),
            );
            d.revision += 1;
            (
                d.config.services[&s].clone(),
                *d.generation.get(&s).unwrap_or(&0),
            )
        };
        let cancel = Arc::new(AtomicBool::new(false));
        if s == Service::Chatgpt {
            *self.0.login_cancel.lock().unwrap() = Some(cancel.clone());
        }
        // Claude Code's sign-in avoids the Cloudflare check claude.ai shows to
        // background browser reads, so it is preferred whenever it exists.
        let via_claude_code = s == Service::Claude && claude_code::available();
        let this = self.clone();
        std::thread::spawn(move || {
            let progress = |p| this.auth_progress(p);
            let result: Result<Vec<Metric>> = (|| {
                if logout {
                    if s == Service::Chatgpt {
                        codex::logout(&this.0.store.root)?;
                    } else {
                        this.0.browser.clear(s)?;
                    }
                    return Ok(vec![]);
                }
                if s.api() {
                    let key = this.0.store.key(s)?;
                    providers::api(s, &key, &cfg)
                } else if s == Service::Chatgpt {
                    let v = if login {
                        codex::login(&this.0.store.root, cfg.device_login, cancel, &progress)?
                    } else {
                        codex::limits(&this.0.store.root, cancel, &progress)?
                    };
                    providers::codex(&v)
                } else if via_claude_code {
                    let mut rows = providers::claude(&claude_code::usage()?)?;
                    for m in &mut rows {
                        m.source = "Claude Code (oficjalne logowanie konta Claude)".into();
                    }
                    Ok(rows)
                } else if login {
                    this.0.browser.login(s, &cfg)?;
                    Ok(vec![])
                } else {
                    this.0.browser.refresh(s, &cfg)
                }
            })();
            let mut d = this.0.data.lock().unwrap();
            d.busy.remove(&s);
            if s == Service::Chatgpt && login {
                d.auth_busy = false;
                d.auth_message.clear();
                d.auth_url.clear();
                d.auth_code.clear();
            }
            if logout {
                if s == Service::Chatgpt {
                    d.account_label.clear();
                }
                d.config.services.get_mut(&s).unwrap().enabled = false;
                if !this.0.demo {
                    let _ = this.0.store.save(&d.config);
                }
            }
            if d.generation.get(&s).copied().unwrap_or(0) != generation {
                d.last.remove(&s);
                d.revision += 1;
                return;
            }
            match result {
                Ok(metrics) => {
                    if s == Service::Gemini {
                        d.gemini_state = if logout {
                            GeminiState::Disconnected
                        } else if login {
                            GeminiState::AwaitingSignIn
                        } else {
                            GeminiState::Connected
                        };
                    }
                    d.metrics.retain(|_, m| m.service != s);
                    for m in metrics {
                        d.metrics.insert(m.identity(), m);
                    }
                    let text = if logout {
                        "Wyczyszczono sesje lokalna"
                    } else if login && s == Service::Gemini {
                        "Zaloguj się ręcznie, zamknij osobne okno Gemini i wybierz Kontynuuj."
                    } else if login && !(s == Service::Chatgpt) && !via_claude_code {
                        "Zaloguj się w otwartym oknie przeglądarki, zamknij je i naciśnij Odśwież"
                    } else {
                        "Aktualne"
                    };
                    d.status.insert(s, text.into());
                    d.message = text.into();
                    if login {
                        d.config.services.get_mut(&s).unwrap().enabled = true;
                        let _ = this.0.store.save(&d.config);
                    }
                }
                Err(e) => {
                    if s == Service::Gemini {
                        d.gemini_state = GeminiState::Error;
                    }
                    let text = e.to_string();
                    d.status.insert(s, text.clone());
                    for m in d.metrics.values_mut().filter(|m| m.service == s) {
                        m.error = Some(text.clone());
                    }
                    d.message = format!("{}: {text}", s.name());
                }
            }
            d.revision += 1;
        });
    }
    fn auth_progress(&self, p: codex::Progress) {
        let mut d = self.0.data.lock().unwrap();
        match p {
            codex::Progress::Message(text) => {
                d.auth_message = text.clone();
                d.message = text;
            }
            codex::Progress::Prompt { url, code } => {
                d.auth_url = url;
                d.auth_code = code;
                d.auth_message = if d.auth_code.is_empty() {
                    "Dokończ logowanie w przeglądarce. Aplikacja czeka na potwierdzenie.".into()
                } else {
                    "Wpisz ten kod na otwartej stronie ChatGPT.".into()
                };
            }
            codex::Progress::Connected(label) => {
                d.account_label = label;
                d.auth_message = "Konto potwierdzone. Pobieranie limitów…".into();
            }
        }
        d.revision += 1;
    }
    pub fn cancel_login(&self) {
        if let Some(cancel) = self.0.login_cancel.lock().unwrap().as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.message("Anulowanie logowania…");
    }
    pub fn request_show(&self) {
        self.0.show.store(true, Ordering::Relaxed)
    }
    pub fn take_show(&self) -> bool {
        self.0.show.swap(false, Ordering::Relaxed)
    }
    pub fn quit(&self) {
        self.0.quit.store(true, Ordering::Relaxed)
    }
    pub fn quitting(&self) -> bool {
        self.0.quit.load(Ordering::Relaxed)
    }
    pub fn shutdown(&self) {
        if let Some(c) = self.0.login_cancel.lock().unwrap().as_ref() {
            c.store(true, Ordering::Relaxed);
        }
        self.quit();
        self.0.browser.shutdown();
        let until = Instant::now() + Duration::from_secs(2);
        while self.0.data.lock().unwrap().busy.contains(&Service::Chatgpt) && Instant::now() < until
        {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

pub fn selected(s: &Snapshot) -> Option<&Metric> {
    s.metrics.iter().find(|m| m.identity() == s.config.selected)
}
pub fn tray_percent(s: &Snapshot) -> Option<f64> {
    selected(s)
        .filter(|m| s.config.services[&m.service].enabled && m.current(s.config.interval))
        .and_then(|m| m.display_percent(s.config.remaining()))
}
pub fn tooltip(s: &Snapshot) -> String {
    match selected(s) {
        Some(m) => format!(
            "{}: {} / {}{}",
            m.display_value(s.config.remaining()),
            m.service.name(),
            m.label,
            if m.current(s.config.interval) {
                ""
            } else {
                " [nieaktualne]"
            }
        ),
        None => "AI Gauge - wybierz licznik w menu".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_preferences_persist_without_changing_monitoring_or_pin() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::new(Some(dir.path().to_path_buf())).unwrap());
        let mut cfg = Config {
            selected: "gemini:week".into(),
            ..Default::default()
        };
        cfg.services.get_mut(&Service::Gemini).unwrap().enabled = true;
        let engine = Engine::new(store.clone(), cfg, false);
        engine.reorder(Service::Gemini, 0);
        engine.visibility(Service::Gemini, false);
        engine.appearance("dark".into(), 200);
        let saved = store.load().unwrap();
        assert_eq!(saved.service_order[0], Service::Gemini);
        assert!(saved.hidden_services.contains(&Service::Gemini));
        assert!(saved.services[&Service::Gemini].enabled);
        assert_eq!(saved.selected, "gemini:week");
        assert_eq!((saved.appearance.as_str(), saved.text_scale), ("dark", 200));
        engine.reset_layout();
        let saved = store.load().unwrap();
        assert_eq!(saved.service_order, Service::ALL);
        assert!(saved.hidden_services.is_empty());
        assert_eq!(saved.selected, "gemini:week");
    }
    #[test]
    fn pending_gemini_auth_and_errors_block_background_reads() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::new(Some(dir.path().to_path_buf())).unwrap());
        let engine = Engine::new(store, Config::default(), false);
        for state in [
            GeminiState::AwaitingSignIn,
            GeminiState::Ready,
            GeminiState::Error,
        ] {
            {
                let mut d = engine.0.data.lock().unwrap();
                d.gemini_state = state;
                d.config.services.get_mut(&Service::Gemini).unwrap().enabled = true;
            }
            engine.refresh(Service::Gemini);
            engine.tick();
            assert!(!engine.snapshot().busy.contains(&Service::Gemini));
        }
        engine.cancel_gemini();
        assert_eq!(engine.snapshot().gemini_state, GeminiState::Disconnected);
        assert!(!engine.snapshot().config.services[&Service::Gemini].enabled);
    }
    #[test]
    fn demo_pin_does_not_write_or_contact_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::new(Some(dir.path().to_path_buf())).unwrap());
        let engine = Engine::new(store, Config::default(), true);
        assert_eq!(engine.snapshot().metrics.len(), 8);
        engine.pin("claude:seven_day");
        engine.refresh_all();
        engine.general(300, false, true, false);
        assert_eq!(engine.snapshot().config.selected, "claude:seven_day");
        assert_eq!(tray_percent(&engine.snapshot()), Some(19.));
        assert!(!dir.path().join("settings.json").exists());
    }
    #[test]
    fn remaining_preference_is_saved_and_refreshes_display() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::new(Some(dir.path().to_path_buf())).unwrap());
        let engine = Engine::new(store.clone(), Config::default(), false);
        let revision = engine.snapshot().revision;
        engine.general(300, false, true, false);
        assert!(engine.snapshot().revision > revision);
        assert!(store.load().unwrap().show_remaining);
        let restarted = Engine::new(store.clone(), store.load().unwrap(), false);
        assert!(restarted.snapshot().config.show_remaining);
        restarted.general(300, false, false, true);
        assert!(!store.load().unwrap().show_remaining);
        assert!(store.load().unwrap().show_api_remaining);
    }
    #[test]
    fn tray_and_tooltip_agree_without_treating_unknown_as_remaining() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::new(Some(dir.path().to_path_buf())).unwrap());
        let engine = Engine::new(store, Config::default(), true);
        let mut s = engine.snapshot();
        assert_eq!(tray_percent(&s), Some(37.));
        assert!(tooltip(&s).contains("37% wykorzystano"));
        s.config.show_remaining = true;
        assert_eq!(tray_percent(&s), Some(63.));
        assert!(tooltip(&s).contains("63% pozostało"));
        let id = s.config.selected.clone();
        let m = s.metrics.iter_mut().find(|m| m.identity() == id).unwrap();
        m.label = "Long provider limit name ".repeat(10);
        assert!(tooltip(&s)
            .chars()
            .take(120)
            .collect::<String>()
            .contains("63% pozostało"));
        let m = s.metrics.iter_mut().find(|m| m.identity() == id).unwrap();
        m.fetched -= chrono::Duration::seconds(1000);
        assert_eq!(tray_percent(&s), None);
        assert!(tooltip(&s).contains("[nieaktualne]"));
        let m = s.metrics.iter_mut().find(|m| m.identity() == id).unwrap();
        m.fetched = chrono::Utc::now();
        m.error = Some("offline".into());
        assert_eq!(tray_percent(&s), None);
        let m = s.metrics.iter_mut().find(|m| m.identity() == id).unwrap();
        m.error = None;
        m.percent = None;
        m.value = "Brak danych".into();
        assert_eq!(tray_percent(&s), None);
        assert!(!tooltip(&s).contains("100%"));
    }
    #[test]
    fn stale_data_never_counts_as_current() {
        let mut m = Metric::new(
            Service::Claude,
            "week",
            "Tydzien",
            "20%",
            Some(20.),
            "fixture",
        );
        m.fetched -= chrono::Duration::seconds(1000);
        assert!(!m.current(300));
        m.fetched = chrono::Utc::now();
        m.error = Some("offline".into());
        assert!(!m.current(300));
    }
}
