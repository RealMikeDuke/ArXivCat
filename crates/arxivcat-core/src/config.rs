use std::path::{Path, PathBuf};

use crate::error::Result;

pub const DEEPSEEK_PROFILE: u8 = 1;
pub const CUSTOM_PROFILE: u8 = 2;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomApiConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub struct ApiProfile {
    pub id: u8,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub supports_thinking: bool,
}

impl ApiProfile {
    fn deepseek(api_key: Option<String>) -> Self {
        Self {
            id: DEEPSEEK_PROFILE,
            name: "DeepSeek".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-v4-flash".to_string(),
            api_key,
            supports_thinking: true,
        }
    }

    fn custom(config: CustomApiConfig) -> Self {
        Self {
            id: CUSTOM_PROFILE,
            name: "Custom OpenAI-compatible API".to_string(),
            base_url: config.base_url,
            model: config.model,
            api_key: Some(config.api_key),
            supports_thinking: false,
        }
    }

    /// Summary generation uses the active provider credentials but always targets
    /// the configured DeepSeek Flash model, independent of interactive chat.
    fn summary(config: CustomApiConfig) -> Self {
        Self {
            id: CUSTOM_PROFILE,
            name: "Custom OpenAI-compatible API".to_string(),
            base_url: config.base_url,
            model: "deepseek/deepseek-v4.1-flash".to_string(),
            api_key: Some(config.api_key),
            supports_thinking: false,
        }
    }
}

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
    #[serde(rename = "api_profile")]
    pub api_profile: Option<u8>,
    #[serde(rename = "custom_api")]
    pub custom_api: Option<CustomApiConfig>,
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

    pub fn active_api_profile(&self) -> Result<ApiProfile> {
        match self.api_profile.unwrap_or(DEEPSEEK_PROFILE) {
            DEEPSEEK_PROFILE => {
                let api_key = std::env::var("DEEPSEEK_API_KEY")
                    .ok()
                    .filter(|key| !key.is_empty())
                    .or_else(|| self.deepseek_api_key.clone());
                Ok(ApiProfile::deepseek(api_key))
            }
            CUSTOM_PROFILE => self
                .custom_api
                .clone()
                .map(ApiProfile::custom)
                .ok_or_else(|| {
                    crate::error::ArxivError::Config(
                        "custom API is not configured; run `arxivcat token set --profile 2`".into(),
                    )
                }),
            profile => Err(crate::error::ArxivError::Config(format!(
                "unknown API profile: {profile}"
            ))),
        }
    }

    pub fn active_summary_api_profile(&self) -> Result<ApiProfile> {
        match self.api_profile.unwrap_or(DEEPSEEK_PROFILE) {
            DEEPSEEK_PROFILE => self.active_api_profile(),
            CUSTOM_PROFILE => self
                .custom_api
                .clone()
                .map(ApiProfile::summary)
                .ok_or_else(|| {
                    crate::error::ArxivError::Config(
                        "custom API is not configured; run `arxivcat token set --profile 2`".into(),
                    )
                }),
            profile => Err(crate::error::ArxivError::Config(format!(
                "unknown API profile: {profile}"
            ))),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            deepseek_api_key: None,
            chat_model: Some("Flash".to_string()),
            workspace_path: None,
            api_profile: Some(DEEPSEEK_PROFILE),
            custom_api: None,
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

pub fn save_custom_api(base_url: &str, model: &str, api_key: &str) -> Result<()> {
    let custom_api = validate_custom_api(base_url, model, api_key)?;
    let mut config = load_or_backup_corrupt();
    config.custom_api = Some(custom_api);
    config.save()
}

pub fn validate_custom_api(base_url: &str, model: &str, api_key: &str) -> Result<CustomApiConfig> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    let model = model.trim().to_string();
    let api_key = api_key.trim().to_string();
    let url = reqwest::Url::parse(&base_url).map_err(|_| {
        crate::error::ArxivError::Config("custom API base URL must be a valid HTTP(S) URL".into())
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(crate::error::ArxivError::Config(
            "custom API base URL must be an HTTP(S) URL without credentials, query, or fragment"
                .into(),
        ));
    }
    if model.is_empty() || model.chars().any(char::is_control) {
        return Err(crate::error::ArxivError::Config(
            "custom API model cannot be empty or contain control characters".into(),
        ));
    }
    if api_key.is_empty() || api_key.chars().any(char::is_control) {
        return Err(crate::error::ArxivError::Config(
            "custom API token cannot be empty or contain control characters".into(),
        ));
    }
    Ok(CustomApiConfig {
        base_url,
        model,
        api_key,
    })
}

pub fn use_api_profile(profile: u8) -> Result<()> {
    let mut config = load_or_backup_corrupt();
    match profile {
        DEEPSEEK_PROFILE => {}
        CUSTOM_PROFILE if config.custom_api.is_none() => {
            return Err(crate::error::ArxivError::Config(
                "custom API is not configured; run `arxivcat token set --profile 2`".into(),
            ));
        }
        CUSTOM_PROFILE => {}
        _ => {
            return Err(crate::error::ArxivError::Config(format!(
                "unknown API profile: {profile}"
            )))
        }
    }
    config.api_profile = Some(profile);
    config.save()
}

pub fn load_active_api_profile() -> Result<ApiProfile> {
    load_or_backup_corrupt().active_api_profile()
}

pub fn load_active_summary_api_profile() -> Result<ApiProfile> {
    load_or_backup_corrupt().active_summary_api_profile()
}

pub fn load_config() -> Config {
    load_or_backup_corrupt()
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
    load_or_backup_corrupt()
        .active_api_profile()
        .ok()
        .and_then(|profile| profile.api_key)
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
        assert_eq!(config.active_api_profile().unwrap().id, DEEPSEEK_PROFILE);
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
            api_profile: Some(DEEPSEEK_PROFILE),
            custom_api: None,
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
    fn custom_profile_uses_saved_endpoint_model_and_token() {
        let config: Config =
            serde_json::from_str(r#"{"api_profile":2,"custom_api":{"base_url":"https://api.example.test/v1/","model":"test-model","api_key":"test-token"}}"#).unwrap();
        let profile = config.active_api_profile().unwrap();
        assert_eq!(profile.name, "Custom OpenAI-compatible API");
        assert_eq!(profile.base_url, "https://api.example.test/v1/");
        assert_eq!(profile.model, "test-model");
        assert_eq!(profile.api_key.as_deref(), Some("test-token"));
    }

    #[test]
    fn summary_profile_pins_deepseek_flash_without_changing_chat_profile() {
        let config: Config = serde_json::from_str(r#"{"api_profile":2,"custom_api":{"base_url":"https://api.example.test/v1","model":"gpt-5.6-terra","api_key":"test-token"}}"#).unwrap();
        assert_eq!(config.active_api_profile().unwrap().model, "gpt-5.6-terra");
        assert_eq!(
            config.active_summary_api_profile().unwrap().model,
            "deepseek/deepseek-v4.1-flash"
        );
    }

    #[test]
    fn custom_api_validation_normalizes_and_rejects_invalid_values() {
        let custom = validate_custom_api("https://api.example.test/v1/", "model", "token").unwrap();
        assert_eq!(custom.base_url, "https://api.example.test/v1");
        assert!(validate_custom_api("ftp://api.example.test", "model", "token").is_err());
        assert!(validate_custom_api("https://api.example.test?x=1", "model", "token").is_err());
        assert!(validate_custom_api("https://api.example.test", "", "token").is_err());
        assert!(validate_custom_api("https://api.example.test", "model", "").is_err());
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
