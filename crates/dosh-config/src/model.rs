use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoshConfig {
    #[serde(default)]
    pub prompt: PromptConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub completion: CompletionConfig,
    #[serde(default)]
    pub plugin: PluginConfig,
    #[serde(default)]
    pub table: TableConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub watch: WatchConfig,
}

impl Default for DoshConfig {
    fn default() -> Self {
        Self {
            prompt: PromptConfig::default(),
            history: HistoryConfig::default(),
            completion: CompletionConfig::default(),
            plugin: PluginConfig::default(),
            table: TableConfig::default(),
            security: SecurityConfig::default(),
            watch: WatchConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    pub theme: String,
    pub multiline: bool,
    pub right_prompt: bool,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            theme: "classic".to_string(),
            multiline: true,
            right_prompt: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    pub max_entries: usize,
    pub dedupe: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 2000,
            dedupe: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionConfig {
    pub timeout_ms: u64,
    pub case_insensitive: bool,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 250,
            case_insensitive: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub safe_mode_dynamic_completion: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            safe_mode_dynamic_completion: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableConfig {
    pub unicode: bool,
    pub max_width: usize,
}

impl Default for TableConfig {
    fn default() -> Self {
        Self {
            unicode: true,
            max_width: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub confirm_destructive: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            confirm_destructive: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    pub debounce_ms: u64,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self { debounce_ms: 500 }
    }
}
