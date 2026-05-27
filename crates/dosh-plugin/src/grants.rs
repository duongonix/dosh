use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::Permission;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionGrants {
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginGrantEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginGrantEntry {
    #[serde(default)]
    pub permissions: BTreeSet<Permission>,
}

impl PermissionGrants {
    pub fn from_toml_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read permission grants {}", path.display()))?;
        let grants: Self = toml::from_str(&raw)
            .with_context(|| format!("invalid permission grants TOML {}", path.display()))?;
        Ok(grants)
    }

    pub fn save_to_toml_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn missing_permissions(
        &self,
        plugin_name: &str,
        requested: &[Permission],
    ) -> Vec<Permission> {
        let granted = self
            .plugins
            .get(plugin_name)
            .map(|v| &v.permissions)
            .cloned()
            .unwrap_or_default();
        requested
            .iter()
            .filter(|p| !granted.contains(*p))
            .cloned()
            .collect()
    }

    pub fn grant_permissions(&mut self, plugin_name: &str, permissions: &[Permission]) {
        let entry = self.plugins.entry(plugin_name.to_string()).or_default();
        for p in permissions {
            entry.permissions.insert(p.clone());
        }
    }
}
