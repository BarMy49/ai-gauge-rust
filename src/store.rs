use crate::model::{validate_key, Config, Service};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use fs2::FileExt;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};
use zeroize::Zeroizing;

pub struct Store {
    pub root: PathBuf,
    ram: Mutex<HashMap<Service, Zeroizing<String>>>,
}

impl Store {
    pub fn new(root: Option<PathBuf>) -> Result<Self> {
        let root = root.unwrap_or_else(|| {
            ProjectDirs::from("", "AI Gauge", "AI Gauge Rust")
                .expect("Brak katalogu danych")
                .data_local_dir()
                .to_path_buf()
        });
        fs::create_dir_all(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self {
            root,
            ram: Mutex::new(HashMap::new()),
        })
    }
    pub fn lock(&self) -> Result<File> {
        let f = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("instance.lock"))?;
        f.try_lock_exclusive()
            .context("AI Gauge juz dziala. Otworz okno z ikony tray.")?;
        Ok(f)
    }
    pub fn load(&self) -> Result<Config> {
        let p = self.root.join("settings.json");
        let mut c = if p.exists() {
            serde_json::from_slice(&fs::read(p)?).context("Nie mozna odczytac settings.json")?
        } else {
            Config::default()
        };
        c.validate()?;
        Ok(c)
    }
    pub fn save(&self, c: &Config) -> Result<()> {
        let mut f = tempfile::NamedTempFile::new_in(&self.root)?;
        f.write_all(&serde_json::to_vec_pretty(c)?)?;
        f.as_file().sync_all()?;
        f.persist(self.root.join("settings.json"))
            .map_err(|_| anyhow::anyhow!("Nie mozna zapisac ustawien"))?;
        Ok(())
    }
    fn entry(s: Service) -> Result<keyring::Entry> {
        keyring::Entry::new("AI Gauge Rust", s.id())
            .map_err(|_| anyhow::anyhow!("Nie mozna otworzyc systemowego magazynu kluczy"))
    }
    pub fn key(&self, s: Service) -> Result<Zeroizing<String>> {
        if let Some(v) = self.ram.lock().unwrap().get(&s) {
            return Ok(v.clone());
        }
        match Self::entry(s)?.get_password() {
            Ok(v) => Ok(Zeroizing::new(v)),
            Err(keyring::Error::NoEntry) => Ok(Zeroizing::new(String::new())),
            Err(_) => anyhow::bail!(
                "Odblokuj systemowy magazyn kluczy albo wpisz klucz tylko na te sesje"
            ),
        }
    }
    pub fn set_key(&self, s: Service, value: String, remember: bool) -> Result<()> {
        let value = Zeroizing::new(value);
        validate_key(&value)?;
        if remember {
            Self::entry(s)?.set_password(&value).map_err(|_| {
                anyhow::anyhow!(
                    "Nie mozna zapisac klucza. Odblokuj magazyn lub wybierz zapis tylko w pamieci."
                )
            })?;
            self.ram.lock().unwrap().remove(&s);
        } else {
            // A RAM-only override must work even without a keyring. The previous
            // saved credential is left intact; deletion is an explicit UI action.
            self.ram.lock().unwrap().insert(s, value);
        }
        Ok(())
    }
    pub fn delete_key(&self, s: Service) -> Result<()> {
        let entry = Self::entry(s)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => anyhow::bail!("Nie mozna usunac klucza z magazynu systemowego"),
        };
        self.ram.lock().unwrap().remove(&s);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_key_stays_out_of_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Some(dir.path().to_path_buf())).unwrap();
        store
            .set_key(Service::Deepseek, "fixture-private-key".into(), false)
            .unwrap();
        assert_eq!(
            &*store.key(Service::Deepseek).unwrap(),
            "fixture-private-key"
        );
        let mut cfg = Config::default();
        cfg.services.get_mut(&Service::Deepseek).unwrap().enabled = true;
        store.save(&cfg).unwrap();
        let bytes = fs::read_to_string(dir.path().join("settings.json")).unwrap();
        assert!(!bytes.contains("fixture-private-key"));
        assert!(store.load().unwrap().services[&Service::Deepseek].enabled);
    }
    #[test]
    fn second_instance_cannot_lock() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Some(dir.path().to_path_buf())).unwrap();
        let _first = store.lock().unwrap();
        assert!(store.lock().is_err());
    }
}
