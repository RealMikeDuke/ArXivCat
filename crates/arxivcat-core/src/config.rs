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
    Config::load().ok()?.workspace_path
}

pub fn save_workspace_path(path: &Path) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();
    config.workspace_path = Some(path.to_string_lossy().to_string());
    config.save()
}

pub fn save_token(token: &str) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();
    config.deepseek_api_key = Some(token.to_string());
    config.save()
}

pub fn load_cached_token() -> Option<String> {
    let config = Config::load().ok()?;
    config.resolve_api_key()
}

pub fn save_model_preference(model: &str) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();
    config.chat_model = Some(model.to_string());
    config.save()
}

pub fn load_model_preference() -> String {
    Config::load()
        .ok()
        .and_then(|c| c.chat_model)
        .unwrap_or_else(|| "Flash".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
