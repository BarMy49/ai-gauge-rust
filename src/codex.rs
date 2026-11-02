//! Official account authorization. Opening a page is never treated as sign-in.
use crate::auth_client;
use anyhow::{anyhow, bail, ensure, Result};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub enum Progress {
    Message(String),
    Prompt { url: String, code: String },
    Connected(String),
}
struct Rpc {
    child: Child,
    input: ChildStdin,
    rx: Receiver<Value>,
    id: u64,
    inbox: VecDeque<Value>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Rpc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Rpc {
    fn new(
        root: &Path,
        download: bool,
        cancel: Arc<AtomicBool>,
        progress: &dyn Fn(Progress),
    ) -> Result<Self> {
        let program =
            auth_client::resolve(root, download, &cancel, &|m| progress(Progress::Message(m)))?;
        Self::spawn_mode(&program.path, root, program.standalone, cancel)
    }
    #[cfg(all(test, unix))]
    fn spawn(path: &Path, root: &Path) -> Result<Self> {
        Self::spawn_mode(path, root, false, Arc::new(AtomicBool::new(false)))
    }
    fn spawn_mode(
        path: &Path,
        root: &Path,
        standalone: bool,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        let home = root.join("codex");
        fs::create_dir_all(&home)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&home, fs::Permissions::from_mode(0o700))?;
        }
        let mut cmd = Command::new(path);
        if !standalone {
            cmd.arg("app-server");
        }
        cmd.args(["--listen", "stdio://"])
            .env("CODEX_HOME", home)
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_AUTH_PATH")
            .env_remove("CODEX_CONNECTORS_TOKEN")
            .env_remove("CODEX_APPS_AUTHORIZATION")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|_| anyhow!("Nie można uruchomić modułu logowania ChatGPT"))?;
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if line.len() > 4 * 1024 * 1024 {
                    break;
                }
                if let Ok(v) = serde_json::from_str(&line) {
                    if tx.send(v).is_err() {
                        break;
                    }
                }
            }
        });
        let mut rpc = Self {
            child,
            input,
            rx,
            id: 0,
            inbox: VecDeque::new(),
            cancel,
        };
        rpc.request(
            "initialize",
            json!({"clientInfo":{"name":"ai_gauge_rust","title":"AI Gauge","version":"0.3.0"}}),
        )?;
        writeln!(rpc.input, "{}", json!({"method":"initialized"}))?;
        rpc.input.flush()?;
        Ok(rpc)
    }
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        writeln!(
            self.input,
            "{}",
            json!({"id":self.id,"method":method,"params":params})
        )?;
        self.input.flush()?;
        let until = Instant::now() + Duration::from_secs(45);
        while Instant::now() < until {
            ensure!(!self.cancel.load(Ordering::Relaxed), "Anulowano logowanie");
            let v = match self.rx.recv_timeout(Duration::from_millis(250)) {
                Ok(v) => v,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => bail!("Moduł logowania zamknął połączenie"),
            };
            if v["id"].as_u64() == Some(self.id) {
                ensure!(v.get("error").is_none(),"Moduł logowania odrzucił żądanie. Sprawdź połączenie i wersję zainstalowanego Codex.");
                return Ok(v["result"].clone());
            }
            if v.get("method").is_some() {
                if self.inbox.len() >= 128 {
                    self.inbox.pop_front();
                }
                self.inbox.push_back(v);
            }
        }
        bail!("Przekroczono czas odpowiedzi ChatGPT")
    }
    fn account(&mut self, progress: &dyn Fn(Progress)) -> Result<String> {
        let v = self.request("account/read", json!({"refreshToken":true}))?;
        let label = account_label(&v)?;
        progress(Progress::Connected(label.clone()));
        Ok(label)
    }
}
fn account_label(v: &Value) -> Result<String> {
    ensure!(
        v["account"]["type"] == "chatgpt",
        "Brak potwierdzonej sesji ChatGPT. Połącz konto ponownie."
    );
    let email = v["account"]["email"].as_str().unwrap_or("Konto ChatGPT");
    let plan = v["account"]["planType"].as_str().unwrap_or("");
    Ok(if plan.is_empty() {
        email.into()
    } else {
        format!("{email} · {plan}")
    })
}
fn trusted_login(raw: &str) -> Result<()> {
    let u = url::Url::parse(raw)?;
    ensure!(
        u.scheme() == "https"
            && matches!(
                u.host_str(),
                Some("auth.openai.com" | "auth0.openai.com" | "chatgpt.com")
            )
            && u.username().is_empty()
            && u.password().is_none()
            && u.port_or_known_default() == Some(443),
        "Nieznany adres logowania ChatGPT"
    );
    Ok(())
}
pub fn limits(root: &Path, cancel: Arc<AtomicBool>, progress: &dyn Fn(Progress)) -> Result<Value> {
    let mut rpc = Rpc::new(root, false, cancel, progress)?;
    rpc.account(progress)?;
    rpc.request("account/rateLimits/read", json!({}))
}
pub fn logout(root: &Path) -> Result<()> {
    Rpc::new(root, false, Arc::new(AtomicBool::new(false)), &|_| {})?
        .request("account/logout", json!({}))?;
    Ok(())
}
pub fn login(
    root: &Path,
    device: bool,
    cancel: Arc<AtomicBool>,
    progress: &dyn Fn(Progress),
) -> Result<Value> {
    let mut rpc = Rpc::new(root, true, cancel.clone(), progress)?;
    progress(Progress::Message(
        "Oczekiwanie na autoryzację ChatGPT…".into(),
    ));
    let params = if device {
        json!({"type":"chatgptDeviceCode"})
    } else {
        json!({"type":"chatgpt"})
    };
    let result = rpc.request("account/login/start", params)?;
    let (url, code) = if device {
        (result["verificationUrl"].as_str().ok_or_else(||anyhow!("ChatGPT nie udostępnił logowania kodem. Włącz kody urządzeń w ustawieniach bezpieczeństwa konta lub użyj zwykłego logowania."))?,result["userCode"].as_str().ok_or_else(||anyhow!("Brak kodu logowania"))?)
    } else {
        (
            result["authUrl"]
                .as_str()
                .ok_or_else(|| anyhow!("ChatGPT nie zwrócił adresu autoryzacji"))?,
            "",
        )
    };
    trusted_login(url)?;
    progress(Progress::Prompt {
        url: url.into(),
        code: code.into(),
    });
    if open::that(url).is_err() {
        progress(Progress::Message(
            "Użyj przycisku Otwórz stronę logowania, aby kontynuować w przeglądarce.".into(),
        ));
    }
    let login_id = result["loginId"].clone();
    let until = Instant::now() + Duration::from_secs(600);
    while Instant::now() < until {
        if cancel.load(Ordering::Relaxed) {
            let _ = writeln!(
                rpc.input,
                "{}",
                json!({"id":9000,"method":"account/login/cancel","params":{"loginId":login_id}})
            );
            let _ = rpc.input.flush();
            bail!("Anulowano logowanie")
        }
        let next = if let Some(v) = rpc.inbox.pop_front() {
            Some(v)
        } else {
            match rpc.rx.recv_timeout(Duration::from_millis(250)) {
                Ok(v) => Some(v),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => bail!("Przerwano autoryzację ChatGPT"),
            }
        };
        if let Some(v) = next {
            if v["method"] == "account/login/completed"
                && (login_id.is_null() || v["params"]["loginId"] == login_id)
            {
                ensure!(v["params"]["success"].as_bool()==Some(true),"ChatGPT nie potwierdził logowania. Spróbuj ponownie lub wybierz logowanie kodem.");
                rpc.account(progress)?;
                return rpc.request("account/rateLimits/read", json!({}));
            }
        }
    }
    bail!("Upłynął czas logowania (10 minut). Spróbuj ponownie.")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_false_authenticated_state() {
        assert!(account_label(&json!({"account":null})).is_err());
        assert!(account_label(&json!({"account":{"type":"apiKey"}})).is_err());
        assert_eq!(
            account_label(
                &json!({"account":{"type":"chatgpt","email":"test@example.test","planType":"plus"}})
            )
            .unwrap(),
            "test@example.test · plus"
        );
    }
    #[test]
    fn exact_authentication_hosts() {
        assert!(trusted_login("https://auth.openai.com/codex/device").is_ok());
        assert!(trusted_login("https://auth.openai.com.evil.test/").is_err());
        assert!(trusted_login("http://auth.openai.com/").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rpc_keeps_notifications_before_reply_and_waits_for_slow_start() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("fixture");
        fs::write(&p,r##"#!/bin/sh
while IFS= read -r line; do
 case "$line" in
  *'"method":"initialize"'*) sleep 1.2; printf '%s\n' '{"method":"account/login/completed","params":{"success":true}}' '{"id":1,"result":{}}';;
  *'"method":"account/read"'*) printf '%s\n' '{"id":2,"result":{"account":{"type":"chatgpt","email":"test@example.test"}}}';;
 esac
done
"##).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        let mut rpc = Rpc::spawn(&p, d.path()).unwrap();
        assert_eq!(rpc.inbox.len(), 1);
        assert_eq!(rpc.account(&|_| {}).unwrap(), "test@example.test");
    }
}
