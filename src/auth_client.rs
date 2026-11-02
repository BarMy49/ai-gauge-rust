//! Obtain the official, checksummed OpenAI account helper when no CLI is installed.
//! This runs only after an explicit sign-in action, never on background refresh.
use anyhow::{anyhow, ensure, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub struct Program {
    pub path: PathBuf,
    pub standalone: bool,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    release: String,
    sha256: String,
}
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const MAX_BINARY: u64 = 512 * 1024 * 1024;
pub fn resolve(
    root: &Path,
    download: bool,
    cancel: &AtomicBool,
    progress: &dyn Fn(String),
) -> Result<Program> {
    let dir = root.join("auth-client");
    let file = dir.join(if cfg!(windows) {
        "codex-app-server.exe"
    } else {
        "codex-app-server"
    });
    if file.is_file() {
        let receipt: Receipt = serde_json::from_slice(
            &fs::read(dir.join("receipt.json"))
                .context("Nie można zweryfikować modułu logowania")?,
        )?;
        ensure!(file_hash(&file)?==receipt.sha256,"Moduł logowania jest uszkodzony. Usuń katalog auth-client z danych aplikacji i połącz konto ponownie.");
        return Ok(Program {
            path: file,
            standalone: true,
        });
    }
    if let Ok(path) = which::which("codex") {
        return Ok(Program {
            path,
            standalone: false,
        });
    }
    ensure!(download,"Połącz konto ChatGPT. Moduł logowania zostanie przygotowany automatycznie przy pierwszym połączeniu.");
    progress("Przygotowywanie oficjalnego modułu logowania OpenAI…".into());
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .user_agent("AI-Gauge/0.3")
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;
    let release:Value=client.get("https://api.github.com/repos/openai/codex/releases/latest").send().map_err(|_|anyhow!("Nie można pobrać modułu logowania z GitHub. Sprawdź połączenie lub zainstaluj oficjalny Codex CLI."))?.error_for_status().map_err(|_|anyhow!("GitHub nie udostępnił modułu logowania. Spróbuj ponownie później."))?.json()?;
    let asset_name = if cfg!(windows) {
        "codex-app-server-x86_64-pc-windows-msvc.exe.zip"
    } else {
        "codex-app-server-x86_64-unknown-linux-musl.tar.gz"
    };
    let asset=release["assets"].as_array().and_then(|a|a.iter().find(|a|a["name"]==asset_name)).ok_or_else(||anyhow!("Brak oficjalnego modułu dla tego systemu. Zainstaluj Codex CLI i spróbuj ponownie."))?;
    let raw = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow!("Brak adresu pobierania"))?;
    validate_download_url(raw)?;
    let digest = asset["digest"]
        .as_str()
        .and_then(|s| s.strip_prefix("sha256:"))
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| {
            anyhow!("Wydanie nie zawiera sumy SHA-256. Pobieranie zostało zatrzymane.")
        })?;
    let mut response = client
        .get(raw)
        .send()
        .map_err(|_| anyhow!("Pobieranie modułu logowania nie powiodło się"))?
        .error_for_status()
        .map_err(|_| anyhow!("Serwer odmówił pobrania modułu logowania"))?;
    let total = response
        .content_length()
        .or_else(|| asset["size"].as_u64())
        .unwrap_or(0);
    ensure!(
        total <= MAX_ARCHIVE,
        "Moduł logowania przekracza dopuszczalny rozmiar"
    );
    let mut archive = tempfile::NamedTempFile::new_in(&dir)?;
    let mut hash = Sha256::new();
    let mut count = 0;
    let mut last_mb = u64::MAX;
    let mut buffer = vec![0; 256 * 1024];
    loop {
        ensure!(!cancel.load(Ordering::Relaxed), "Anulowano logowanie");
        let n = response
            .read(&mut buffer)
            .map_err(|_| anyhow!("Przerwano pobieranie modułu logowania. Spróbuj ponownie."))?;
        if n == 0 {
            break;
        }
        count += n as u64;
        ensure!(count <= MAX_ARCHIVE, "Moduł przekroczył limit rozmiaru");
        hash.update(&buffer[..n]);
        archive.write_all(&buffer[..n])?;
        let mb = count / (1024 * 1024);
        if mb != last_mb {
            last_mb = mb;
            progress(format!(
                "Pobieranie modułu logowania: {mb} / {} MB",
                total / (1024 * 1024)
            ));
        }
    }
    ensure!(
        format!("{:x}", hash.finalize()).eq_ignore_ascii_case(digest),
        "Suma kontrolna modułu logowania nie zgadza się. Plik nie został uruchomiony."
    );
    progress("Sprawdzanie i przygotowywanie logowania…".into());
    archive.flush()?;
    let mut binary = tempfile::NamedTempFile::new_in(&dir)?;
    if cfg!(windows) {
        let mut zip = zip::ZipArchive::new(fs::File::open(archive.path())?)?;
        let mut found = false;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry
                .enclosed_name()
                .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_default();
            if name.starts_with("codex-app-server") && name.ends_with(".exe") {
                ensure!(entry.size() <= MAX_BINARY, "Plik modułu jest za duży");
                copy_bounded(&mut entry, binary.as_file_mut())?;
                found = true;
                break;
            }
        }
        ensure!(found, "Archiwum nie zawiera oficjalnego programu logowania");
    } else {
        let reader = flate2::read::GzDecoder::new(fs::File::open(archive.path())?);
        let mut tar = tar::Archive::new(reader);
        let mut found = false;
        for entry in tar.entries()? {
            let mut entry = entry?;
            let name = entry
                .path()?
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if entry.header().entry_type().is_file() && name.starts_with("codex-app-server") {
                ensure!(entry.size() <= MAX_BINARY, "Plik modułu jest za duży");
                copy_bounded(&mut entry, binary.as_file_mut())?;
                found = true;
                break;
            }
        }
        ensure!(found, "Archiwum nie zawiera oficjalnego programu logowania");
    }
    ensure!(!cancel.load(Ordering::Relaxed), "Anulowano logowanie");
    binary.flush()?;
    let binary_hash = file_hash(binary.path())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        binary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o700))?;
    }
    binary
        .persist(&file)
        .map_err(|_| anyhow!("Nie można zapisać modułu logowania"))?;
    fs::write(
        dir.join("receipt.json"),
        serde_json::to_vec(&Receipt {
            release: release["tag_name"].as_str().unwrap_or("unknown").into(),
            sha256: binary_hash,
        })?,
    )?;
    Ok(Program {
        path: file,
        standalone: true,
    })
}
fn copy_bounded(reader: &mut impl Read, writer: &mut impl Write) -> Result<()> {
    let copied = std::io::copy(&mut reader.take(MAX_BINARY + 1), writer)?;
    ensure!(
        copied > 0 && copied <= MAX_BINARY,
        "Nieprawidłowy rozmiar programu logowania"
    );
    Ok(())
}
fn file_hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn validate_download_url(raw: &str) -> Result<()> {
    let u = url::Url::parse(raw)?;
    ensure!(
        u.scheme() == "https"
            && u.host_str() == Some("github.com")
            && u.path().starts_with("/openai/codex/releases/download/")
            && u.username().is_empty()
            && u.password().is_none(),
        "Nieprawidłowe źródło modułu logowania"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_official_downloads() {
        assert!(validate_download_url(
            "https://github.com/openai/codex/releases/download/v1/file.zip"
        )
        .is_ok());
        for u in [
            "https://github.com.evil.test/openai/codex/releases/download/file",
            "https://github.com/other/codex/releases/download/file",
            "http://github.com/openai/codex/releases/download/file",
        ] {
            assert!(validate_download_url(u).is_err());
        }
    }
    #[test]
    fn cache_integrity_detects_changes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("fixture");
        fs::write(&p, b"original").unwrap();
        let hash = file_hash(&p).unwrap();
        fs::write(&p, b"changed").unwrap();
        assert_ne!(hash, file_hash(&p).unwrap());
    }
}
