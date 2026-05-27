use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::manifest::PluginManifest;

#[derive(Debug, Clone)]
pub struct PluginPackage {
    pub root_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub entry_path: PathBuf,
    pub manifest: PluginManifest,
}

pub fn discover_plugins(root: &Path) -> Result<Vec<PluginPackage>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut found = Vec::new();
    for entry in WalkDir::new(root)
        .min_depth(1)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "plugin.toml" && entry.file_name() != "dosh.plugin.toml" {
            continue;
        }

        let manifest_path = entry.path().to_path_buf();
        let root_dir = manifest_path.parent().unwrap_or(root).to_path_buf();
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let manifest = PluginManifest::from_toml_str(&raw)
            .with_context(|| format!("invalid plugin manifest {}", manifest_path.display()))?;
        let entry_rel = manifest.entry.clone().unwrap_or_default();
        let entry_path = root_dir.join(entry_rel);

        found.push(PluginPackage {
            root_dir,
            manifest_path,
            entry_path,
            manifest,
        });
    }

    Ok(found)
}
