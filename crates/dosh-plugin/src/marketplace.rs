use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::manifest::PluginManifest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryPackageMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    pub package: RegistryPackageMetadata,
    pub install_from: String,
}

pub trait RegistryClient: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<RegistryPackageMetadata>>;
    fn info(&self, name: &str) -> Result<Option<RegistryPackageMetadata>>;
    fn install_plan(&self, name: &str, version: Option<&str>) -> Result<Option<InstallPlan>>;
}

#[derive(Debug, Clone)]
pub struct LocalRegistry {
    root: PathBuf,
}

impl LocalRegistry {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl RegistryClient for LocalRegistry {
    fn search(&self, query: &str) -> Result<Vec<RegistryPackageMetadata>> {
        let q = query.to_ascii_lowercase();
        let mut out = Vec::new();
        for pkg in scan_registry(&self.root)? {
            if pkg.name.to_ascii_lowercase().contains(&q)
                || pkg
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains(&q)
            {
                out.push(pkg);
            }
        }
        Ok(out)
    }

    fn info(&self, name: &str) -> Result<Option<RegistryPackageMetadata>> {
        let mut all = scan_registry(&self.root)?
            .into_iter()
            .filter(|m| m.name == name)
            .collect::<Vec<_>>();
        all.sort_by(|a, b| a.version.cmp(&b.version));
        Ok(all.pop())
    }

    fn install_plan(&self, name: &str, version: Option<&str>) -> Result<Option<InstallPlan>> {
        let all = scan_registry(&self.root)?;
        let mut candidates = all
            .into_iter()
            .filter(|m| m.name == name)
            .collect::<Vec<_>>();
        if let Some(v) = version {
            candidates.retain(|m| m.version == v);
        }
        candidates.sort_by(|a, b| a.version.cmp(&b.version));
        let Some(package) = candidates.pop() else {
            return Ok(None);
        };
        Ok(Some(InstallPlan {
            install_from: package.source_path.clone(),
            package,
        }))
    }
}

fn scan_registry(root: &Path) -> Result<Vec<RegistryPackageMetadata>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in WalkDir::new(root).min_depth(1).max_depth(5) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name != "dosh.plugin.toml" && name != "plugin.toml" {
            continue;
        }
        let raw = fs::read_to_string(entry.path())?;
        let manifest = PluginManifest::from_toml_str(&raw)?;
        out.push(RegistryPackageMetadata {
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            source_path: entry.path().parent().unwrap_or(root).display().to_string(),
        });
    }
    Ok(out)
}
