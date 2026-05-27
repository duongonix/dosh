use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginInstallSource {
    LocalPath(String),
    Registry(String),
    Url(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStateEntry {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub trusted: bool,
    pub checksum: String,
    pub install_dir: String,
    pub installed_at: String,
    pub source: PluginInstallSource,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginsState {
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginStateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginLockEntry {
    pub name: String,
    pub version: String,
    pub checksum: String,
    pub source: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginsLockfile {
    #[serde(default)]
    pub plugins: Vec<PluginLockEntry>,
}

impl PluginsState {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn upsert(&mut self, entry: PluginStateEntry) {
        self.plugins.insert(entry.name.clone(), entry);
    }

    pub fn remove(&mut self, name: &str) -> Option<PluginStateEntry> {
        self.plugins.remove(name)
    }
}

impl PluginsLockfile {
    pub fn from_state(state: &PluginsState) -> Self {
        let mut plugins = state
            .plugins
            .values()
            .map(|v| PluginLockEntry {
                name: v.name.clone(),
                version: v.version.clone(),
                checksum: v.checksum.clone(),
                source: format!("{:?}", v.source),
                enabled: v.enabled,
            })
            .collect::<Vec<_>>();
        plugins.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
        Self { plugins }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

pub fn now_rfc3339() -> String {
    let now = OffsetDateTime::now_utc();
    format!("{}Z", now.unix_timestamp())
}

pub fn default_state_path(plugins_dir: &Path) -> PathBuf {
    plugins_dir.join("plugins.toml")
}

pub fn default_lock_path(plugins_dir: &Path) -> PathBuf {
    plugins_dir.join("plugins.lock")
}
