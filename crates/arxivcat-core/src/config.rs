use std::path::{Path, PathBuf};

use crate::error::Result;

pub fn get_cache_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("ArxivCat")
    } else {
        dirs::data_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("ArxivCat")
    }
}

pub fn get_downloads_dir() -> PathBuf {
    get_cache_dir().join("downloads")
}

pub fn get_config_path() -> PathBuf {
    get_cache_dir().join("config.json")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(rename = "deepseek_api_key")]
    pub deepseek_api_key: Option<String>,
    #[serde(rename = "chat_model")]
    pub chat_model: Option<String>,
    #[serde(rename = "workspace_path")]
    pub workspace_path: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = get_config_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = serde_json::from_str(&content)
            .map_err(|e| crate::error::ArxivError::Config(format!("invalid config.json: {e}")))?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = get_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        // Atomic write (temp + rename) so a crash never leaves a half-written
        // config. Config may hold the API key, so force 0600 on unix.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn resolve_api_key(&self) -> Option<String> {
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            if !key.is_empty() {
                return Some(key);
            }
        }
        self.deepseek_api_key.clone()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            deepseek_api_key: None,
            chat_model: Some("Flash".to_string()),
            workspace_path: None,
        }
    }
}

pub fn load_workspace_path() -> Option<String> {
    load_or_backup_corrupt().workspace_path
}

pub fn save_workspace_path(path: &Path) -> Result<()> {
    let mut config = load_or_backup_corrupt();
    config.workspace_path = Some(path.to_string_lossy().to_string());
    config.save()
}

pub fn save_token(token: &str) -> Result<()> {
    let mut config = load_or_backup_corrupt();
    config.deepseek_api_key = Some(token.to_string());
    config.save()
}

/// Load the config; if the file exists but fails to parse, back it up as
/// `config.json.corrupt-<ts>` and warn — a later save must never silently
/// overwrite a corrupted file the user could otherwise repair (P2-5).
fn load_or_backup_corrupt() -> Config {
    match Config::load() {
        Ok(c) => c,
        Err(e) => {
            let path = crate::config::get_config_path();
            if path.exists() {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let backup = path.with_extension(format!("json.corrupt-{ts}"));
                if std::fs::rename(&path, &backup).is_ok() {
                    eprintln!(
                        "warning: config.json was corrupted ({e}); backed up to {}",
                        backup.display()
                    );
                }
            }
            Config::default()
        }
    }
}

pub fn load_cached_token() -> Option<String> {
    load_or_backup_corrupt().resolve_api_key()
}

pub fn save_model_preference(model: &str) -> Result<()> {
    let mut config = load_or_backup_corrupt();
    config.chat_model = Some(model.to_string());
    config.save()
}

pub fn load_model_preference() -> String {
    load_or_backup_corrupt()
        .chat_model
        .unwrap_or_else(|| "Flash".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Config tests mutate the process-wide APPDATA env var — serialize them
    // or they clobber each other when run in parallel.
    static CONFIG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_config_deserialize_corrupted() {
        assert!(serde_json::from_str::<Config>("garbage").is_err());
        assert!(serde_json::from_str::<Config>("{{{[").is_err());
        assert!(serde_json::from_str::<Config>("").is_err());
    }

    #[test]
    fn test_config_deserialize_empty_object() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config.deepseek_api_key, None);
        assert_eq!(config.workspace_path, None);
        assert_eq!(config.chat_model, None);
    }

    #[test]
    fn test_config_deserialize_partial_fields() {
        let config: Config = serde_json::from_str(r#"{"workspace_path": "/my/ws"}"#).unwrap();
        assert_eq!(config.workspace_path, Some("/my/ws".into()));
        assert_eq!(config.deepseek_api_key, None);
        assert_eq!(config.chat_model, None);
    }

    #[test]
    fn test_config_deserialize_extra_fields_ignored() {
        let config: Config =
            serde_json::from_str(r#"{"workspace_path": "/ws", "unknown": 123}"#).unwrap();
        assert_eq!(config.workspace_path, Some("/ws".into()));
    }

    #[test]
    fn test_config_save_atomic_and_0600() {
        let _guard = CONFIG_TEST_LOCK.lock().unwrap();
        // Isolate via APPDATA so the real user config is never touched.
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("APPDATA", dir.path()) };

        let cfg = Config {
            deepseek_api_key: Some("sk-test".into()),
            chat_model: Some("Flash".into()),
            workspace_path: Some("/tmp/ws".into()),
        };
        cfg.save().unwrap();

        let path = dir.path().join("ArxivCat").join("config.json");
        assert!(path.exists(), "config.json written under isolated APPDATA");
        assert!(
            !path.with_extension("json.tmp").exists(),
            "no .tmp residue after atomic write"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "config (may hold API key) must be 0600");
        }

        // Roundtrip through the same isolated dir.
        let loaded = Config::load().unwrap();
        assert_eq!(loaded.deepseek_api_key.as_deref(), Some("sk-test"));
        assert_eq!(loaded.workspace_path.as_deref(), Some("/tmp/ws"));
    }
    #[test]
    fn corrupted_config_is_backed_up_not_overwritten() {
        let _guard = CONFIG_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("APPDATA", dir.path()) };
        let cfg_path = get_config_path();
        std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
        std::fs::write(&cfg_path, "{ corrupted json !!!").unwrap();

        // Saving must back up the corrupt file instead of silently clobbering it.
        save_token("sk-test").unwrap();
        assert!(
            std::fs::read_dir(cfg_path.parent().unwrap())
                .unwrap()
                .any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("config.json.corrupt-")),
            "corrupt config must be preserved as a .corrupt-* backup"
        );
        // And the config still works afterwards.
        assert_eq!(load_cached_token().as_deref(), Some("sk-test"));
    }

    #[test]
    fn corrupt_config_read_path_warns_and_backs_up() {
        let _guard = CONFIG_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("APPDATA", dir.path()) };
        let cfg_path = get_config_path();
        std::fs::create_dir_all(cfg_path.parent().unwrap()).unwrap();
        std::fs::write(&cfg_path, "{ not json").unwrap();

        // Read paths must not silently swallow corruption: they back it up
        // (and warn), and fall back to defaults instead of pretending OK.
        assert_eq!(load_workspace_path(), None);
        assert_eq!(load_model_preference(), "Flash");
        let backed_up = std::fs::read_dir(cfg_path.parent().unwrap())
            .unwrap()
            .any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("config.json.corrupt-")
            });
        assert!(backed_up, "read path must back up the corrupt config too");
    }
}
