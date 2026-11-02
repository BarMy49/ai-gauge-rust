//! Browser is an authentication/data adapter, never the application UI.
//! Each provider gets a dedicated profile. CDP is bound to loopback only.
use crate::{model::*, providers};
use anyhow::{anyhow, bail, ensure, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fs,
    hash::{Hash, Hasher},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

type Process = Arc<Mutex<Option<Child>>>;
pub struct BrowserPool {
    root: PathBuf,
    sessions: Mutex<HashMap<Service, Arc<Mutex<Session>>>>,
    processes: Mutex<Vec<Process>>,
    stop: Arc<AtomicBool>,
}
impl BrowserPool {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            sessions: Mutex::new(HashMap::new()),
            processes: Mutex::new(vec![]),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
    fn session(&self, s: Service) -> Arc<Mutex<Session>> {
        self.sessions
            .lock()
            .unwrap()
            .entry(s)
            .or_insert_with(|| {
                let p = Arc::new(Mutex::new(None));
                self.processes.lock().unwrap().push(p.clone());
                Arc::new(Mutex::new(Session {
                    service: s,
                    root: self.root.join("browser").join(s.id()),
                    process: p,
                    port: 0,
                    visible: false,
                    manual: false,
                    browser: None,
                    stop: self.stop.clone(),
                }))
            })
            .clone()
    }
    pub fn login(&self, s: Service, cfg: &ServiceConfig) -> Result<()> {
        let session = self.session(s);
        let mut session = session.lock().unwrap();
        if s == Service::Gemini {
            session.begin_manual(cfg)?;
        } else {
            session.ensure(true)?;
        }
        Ok(())
    }
    pub fn manual_running(&self) -> bool {
        let session = self.session(Service::Gemini);
        let Ok(session) = session.try_lock() else {
            return true;
        };
        session.manual && session.running()
    }
    pub fn refresh(&self, s: Service, cfg: &ServiceConfig) -> Result<Vec<Metric>> {
        let session = self.session(s);
        let mut session = session.lock().unwrap();
        let result = session.read(cfg);
        if !session.visible {
            session.kill();
        }
        result
    }
    pub fn clear(&self, s: Service) -> Result<()> {
        let session = self.session(s);
        let mut session = session.lock().unwrap();
        session.kill();
        if session.root.exists() {
            fs::remove_dir_all(&session.root).map_err(|_| {
                anyhow!(
                    "Nie mozna usunac calego profilu; zamknij okna tej sesji i sprobuj ponownie"
                )
            })?;
        }
        Ok(())
    }
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        for p in self.processes.lock().unwrap().iter() {
            if let Some(mut c) = p.lock().unwrap().take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}
fn browser_exe(choice: &str) -> Result<PathBuf> {
    #[cfg(windows)]
    for var in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(var) {
            for sub in [
                "Microsoft/Edge/Application/msedge.exe",
                "Google/Chrome/Application/chrome.exe",
            ] {
                if choice == "chrome" && !sub.contains("Chrome")
                    || choice == "edge" && !sub.contains("Edge")
                {
                    continue;
                }
                let p = PathBuf::from(&root).join(sub);
                if p.is_file() {
                    return Ok(p);
                }
            }
        }
    }
    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
        "msedge",
        "chrome",
    ] {
        if choice == "chrome" && !name.contains("chrome")
            || choice == "edge" && !name.contains("edge")
        {
            continue;
        }
        if let Ok(p) = which::which(name) {
            return Ok(p);
        }
    }
    bail!("Zainstaluj desktopowy Chrome, Chromium lub Edge do logowania kont subskrypcyjnych")
}
struct Session {
    service: Service,
    root: PathBuf,
    process: Process,
    port: u16,
    visible: bool,
    manual: bool,
    browser: Option<PathBuf>,
    stop: Arc<AtomicBool>,
}
impl Session {
    fn begin_manual(&mut self, cfg: &ServiceConfig) -> Result<()> {
        ensure!(
            !(self.manual && self.running()),
            "Osobne okno Gemini jest już otwarte. Dokończ logowanie i zamknij je."
        );
        self.kill();
        let exe = browser_exe(&cfg.browser)?;
        self.browser = Some(exe.clone());
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        let mut command = manual_command(&exe, &self.root, self.service.url());
        *self.process.lock().unwrap() = Some(
            command
                .spawn()
                .map_err(|_| anyhow!("Nie można otworzyć przeglądarki do logowania Gemini"))?,
        );
        self.manual = true;
        self.visible = true;
        Ok(())
    }
    fn kill(&mut self) {
        if let Some(mut c) = self.process.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.port = 0;
    }
    fn running(&self) -> bool {
        self.process
            .lock()
            .unwrap()
            .as_mut()
            .map(|p| matches!(p.try_wait(), Ok(None)))
            .unwrap_or(false)
    }
    fn wait(&self, d: Duration) -> Result<()> {
        let until = Instant::now() + d;
        while Instant::now() < until {
            ensure!(
                !self.stop.load(Ordering::Relaxed),
                "Aplikacja jest zamykana"
            );
            std::thread::sleep(Duration::from_millis(50))
        }
        Ok(())
    }
    fn ensure(&mut self, visible: bool) -> Result<()> {
        ensure!(
            !self.stop.load(Ordering::Relaxed),
            "Aplikacja jest zamykana"
        );
        if self.manual {
            ensure!(
                !self.running(),
                "Profil Gemini jest zajęty. Zamknij jego okna i wybierz Kontynuuj."
            );
            self.manual = false;
            self.visible = false;
        }
        if self.running() {
            if visible && !self.visible {
                self.kill()
            } else {
                if visible {
                    self.new_tab(self.service.url())?;
                }
                return Ok(());
            }
        }
        let exe = match &self.browser {
            Some(path) => path.clone(),
            None => browser_exe("auto")?,
        };
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        let _ = fs::remove_file(self.root.join("DevToolsActivePort"));
        let mut cmd = Command::new(exe);
        cmd.arg("--remote-debugging-address=127.0.0.1")
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", self.root.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if !visible {
            cmd.arg("--headless=new");
        }
        cmd.arg(if visible {
            self.service.url()
        } else {
            "about:blank"
        });
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        *self.process.lock().unwrap() = Some(
            cmd.spawn()
                .map_err(|_| anyhow!("Nie mozna otworzyc prywatnego profilu przegladarki"))?,
        );
        self.visible = visible;
        for _ in 0..100 {
            if let Ok(text) = fs::read_to_string(self.root.join("DevToolsActivePort")) {
                if let Some(port) = text.lines().next().and_then(|s| s.parse::<u16>().ok()) {
                    if port > 0 {
                        self.port = port;
                        if self.tabs().is_ok() {
                            return Ok(());
                        }
                    }
                }
            }
            if !self.running() {
                break;
            }
            self.wait(Duration::from_millis(100))?;
        }
        self.kill();
        bail!("Nie mozna polaczyc sie z profilem Chrome/Edge. Zamknij jego poprzednie okna i sprobuj ponownie.")
    }
    fn local(&self, path: &str, put: bool) -> Result<Value> {
        let c = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(4))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let url = format!("http://127.0.0.1:{}{path}", self.port);
        let response = if put { c.put(url) } else { c.get(url) }
            .send()
            .map_err(|_| anyhow!("Brak lokalnej sesji przegladarki"))?;
        ensure!(
            response.status().is_success(),
            "Blad lokalnej sesji przegladarki"
        );
        response
            .json()
            .map_err(|_| anyhow!("Nieznany format sesji przegladarki"))
    }
    fn tabs(&self) -> Result<Vec<Value>> {
        Ok(self
            .local("/json/list", false)?
            .as_array()
            .ok_or_else(|| anyhow!("Brak kart sesji"))?
            .iter()
            .filter(|t| t["type"] == "page")
            .cloned()
            .collect())
    }
    fn new_tab(&self, url: &str) -> Result<Value> {
        let encoded: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
        self.local(&format!("/json/new?{encoded}"), true)
    }
    fn read(&mut self, cfg: &ServiceConfig) -> Result<Vec<Metric>> {
        if self.service == Service::Gemini && self.browser.is_none() {
            self.browser = Some(browser_exe(&cfg.browser)?);
        }
        self.ensure(false)?;
        let existing = self
            .tabs()?
            .into_iter()
            .find(|t| allowed(self.service, t["url"].as_str().unwrap_or("")));
        let own = existing.is_none();
        let tab = match existing {
            Some(t) => t,
            None => self.new_tab(self.service.url())?,
        };
        let ws = tab["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| anyhow!("Brak lokalnego kanalu strony"))?;
        let mut c = Cdp::new(ws, self.port)?;
        let result = (|| {
            for attempt in 0..15 {
                self.wait(Duration::from_millis(if attempt == 0 { 800 } else { 1000 }))?;
                let url = c
                    .eval("location.href")
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                if !allowed(self.service, &url) {
                    if self.service == Service::Gemini
                        && url.starts_with("https://accounts.google.com/")
                    {
                        if url::Url::parse(&url).is_ok_and(|u| u.path().contains("rejected")) {
                            bail!("Google odrzuciło tę przeglądarkę. Zamknij okno profilu, wybierz inną zainstalowaną przeglądarkę w ustawieniach Gemini i zaloguj się ponownie. Nie potwierdzono połączenia.");
                        }
                        bail!("Sesja Google wygasła lub logowanie zostało odrzucone. Wybierz Zaloguj się ponownie; logowanie odbywa się w zwykłym oknie przeglądarki.");
                    }
                    continue;
                }
                if self.service == Service::Claude {
                    let org = serde_json::to_string(&cfg.organization)?;
                    let js = format!(
                        r#"(async()=>{{const r=await fetch('/api/organizations',{{credentials:'same-origin'}});if(!r.ok)return {{error:'login'}};const orgs=await r.json();if(!Array.isArray(orgs))return {{error:'format'}};const wanted={org};const o=wanted?orgs.find(o=>o.uuid===wanted):(orgs.length===1?orgs[0]:null);if(!o)return {{error:'organization'}};const u=await fetch('/api/organizations/'+encodeURIComponent(o.uuid)+'/usage',{{credentials:'same-origin'}});return u.ok?await u.json():{{error:'usage'}};}})()"#
                    );
                    let v = c.eval(&js)?;
                    if v["error"] == "organization" {
                        bail!("Wybierz organizacje Claude: wpisz jej UUID w ustawieniach (konto ma kilka organizacji)")
                    }
                    if v.get("error").is_none() {
                        return providers::claude(&v);
                    }
                } else {
                    let v = c.eval(include_str!("../assets/dashboard.js"))?;
                    if let Some(items) = v["items"].as_array() {
                        let mut rows = vec![];
                        for item in items {
                            let label = item["label"].as_str().unwrap_or("Limit");
                            let p = percent(
                                item["percent"]
                                    .as_f64()
                                    .ok_or_else(|| anyhow!("Brak procentu na stronie"))?,
                            )?;
                            let mut hasher = DefaultHasher::new();
                            label.hash(&mut hasher);
                            let label = if self.service == Service::Chatgpt {
                                format!("Codex / {label}")
                            } else {
                                label.into()
                            };
                            rows.push(Metric::new(self.service,format!("web/{:x}",hasher.finish()),label,format!("{p:.0}% wykorzystano"),Some(p),"Odczyt panelu uzycia w prywatnym profilu przegladarki; strona moze opozniac aktualizacje"));
                        }
                        if !rows.is_empty() {
                            return Ok(rows);
                        }
                    }
                }
            }
            bail!("Brak licznikow. Zaloguj sie, otworz panel limitow i nacisnij Odswiez. Widok tego konta moze nie udostepniac procentow.")
        })();
        if own {
            if let Some(id) = tab["id"].as_str() {
                let _ = self.local(&format!("/json/close/{id}"), false);
            }
        }
        result
    }
}
fn manual_command(exe: &std::path::Path, root: &std::path::Path, url: &str) -> Command {
    let mut command = Command::new(exe);
    command
        .arg(format!("--user-data-dir={}", root.display()))
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-background-mode",
            "--new-window",
            url,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
fn allowed(s: Service, url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .map(|u| {
            u.scheme() == "https"
                && u.host_str() == Some(s.host())
                && u.username().is_empty()
                && u.password().is_none()
                && u.port_or_known_default() == Some(443)
        })
        .unwrap_or(false)
}
struct Cdp {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    id: u64,
}
impl Drop for Cdp {
    fn drop(&mut self) {
        let _ = self.socket.close(None);
    }
}
impl Cdp {
    fn new(raw: &str, port: u16) -> Result<Self> {
        let url = url::Url::parse(raw)?;
        ensure!(
            url.scheme() == "ws"
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
                && url.port() == Some(port),
            "Nieprawidlowy lokalny adres przegladarki"
        );
        let (socket, _) = tungstenite::connect(raw)
            .map_err(|_| anyhow!("Nie mozna otworzyc lokalnego kanalu przegladarki"))?;
        if let MaybeTlsStream::Plain(tcp) = socket.get_ref() {
            tcp.set_read_timeout(Some(Duration::from_secs(8)))?;
            tcp.set_write_timeout(Some(Duration::from_secs(4)))?;
        }
        Ok(Self { socket, id: 0 })
    }
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        self.socket
            .send(Message::Text(
                json!({"id":self.id,"method":method,"params":params})
                    .to_string()
                    .into(),
            ))
            .map_err(|_| anyhow!("Zamknieto lokalna sesje przegladarki"))?;
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            let msg = self
                .socket
                .read()
                .map_err(|_| anyhow!("Przekroczono czas odczytu przegladarki"))?;
            if let Message::Text(text) = msg {
                let v: Value = serde_json::from_str(&text)?;
                if v["id"].as_u64() == Some(self.id) {
                    ensure!(v.get("error").is_none(), "Strona odmowila odczytu licznika");
                    return Ok(v["result"].clone());
                }
            }
        }
        bail!("Przekroczono czas odczytu strony")
    }
    fn eval(&mut self, expression: &str) -> Result<Value> {
        let frame = self.call("Page.getFrameTree", json!({}))?["frameTree"]["frame"]["id"].clone();
        let context = self.call(
            "Page.createIsolatedWorld",
            json!({"frameId":frame,"worldName":"ai-gauge-rust"}),
        )?["executionContextId"]
            .clone();
        let v=self.call("Runtime.evaluate",json!({"expression":expression,"contextId":context,"returnByValue":true,"awaitPromise":true}))?;
        ensure!(
            v.get("exceptionDetails").is_none(),
            "Widok uzycia zmienil format; wymagane ponowne logowanie lub aktualizacja integracji"
        );
        Ok(v["result"]["value"].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_google_login_has_no_automation_arguments() {
        let cmd = manual_command(
            std::path::Path::new("chrome"),
            std::path::Path::new("separate-profile"),
            Service::Gemini.url(),
        );
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|a| a == "--user-data-dir=separate-profile"));
        assert_eq!(args.last().unwrap(), Service::Gemini.url());
        assert!(!args.iter().any(|a| a.contains("debugging")
            || a.contains("headless")
            || a.contains("automation")));
    }
    #[test]
    fn official_origin_only() {
        assert!(allowed(Service::Gemini, "https://gemini.google.com/app"));
        for u in [
            "https://gemini.google.com.evil.test/",
            "http://gemini.google.com/",
            "https://x@gemini.google.com/",
            "https://gemini.google.com:444/",
        ] {
            assert!(!allowed(Service::Gemini, u));
        }
    }
}
