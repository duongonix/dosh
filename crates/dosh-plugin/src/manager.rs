use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::discovery::discover_plugins;
use crate::distribution::install_plugin;
use crate::error::PluginError;
use crate::manifest::{PluginManifest, PluginSource};
use crate::marketplace::RegistryClient;
use crate::storage::{
    PluginInstallSource, PluginStateEntry, PluginsLockfile, PluginsState, default_lock_path,
    default_state_path, now_rfc3339,
};
use crate::trust::TrustStore;

#[derive(Debug, Clone)]
pub struct PluginManager {
    pub plugins_dir: PathBuf,
    pub state_path: PathBuf,
    pub lock_path: PathBuf,
    pub trust_store_path: PathBuf,
}

impl PluginManager {
    pub fn new(plugins_dir: PathBuf, _configs_dir: PathBuf) -> Self {
        Self {
            state_path: default_state_path(&plugins_dir),
            lock_path: default_lock_path(&plugins_dir),
            trust_store_path: plugins_dir.join("trust.toml"),
            plugins_dir,
        }
    }

    pub fn list(&self) -> Result<Vec<PluginStateEntry>> {
        self.sync_state_from_filesystem()?;
        let state = PluginsState::load(&self.state_path)?;
        let mut out = state.plugins.into_values().collect::<Vec<_>>();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn install_from_path(&self, from: &Path) -> Result<PluginStateEntry> {
        let manifest = read_manifest(from)?;
        ensure_plugin_wasm_ready(from, &manifest)?;
        let installed_path = install_plugin(from, &self.plugins_dir)?;
        if let Err(err) = validate_plugin_entry(&manifest, &installed_path) {
            let _ = fs::remove_dir_all(&installed_path);
            return Err(err);
        }
        let checksum = compute_manifest_checksum(&manifest, &installed_path)?;
        let mut state = PluginsState::load(&self.state_path)?;
        let entry = PluginStateEntry {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            enabled: true,
            trusted: false,
            checksum,
            install_dir: installed_path.display().to_string(),
            installed_at: now_rfc3339(),
            source: PluginInstallSource::LocalPath(from.display().to_string()),
        };
        state.upsert(entry.clone());
        state.save(&self.state_path)?;
        self.write_lockfile(&state)?;
        Ok(entry)
    }

    pub fn install_from_registry(
        &self,
        registry: &dyn RegistryClient,
        name: &str,
        version: Option<&str>,
    ) -> Result<PluginStateEntry> {
        let plan = registry
            .install_plan(name, version)?
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;
        let entry = self.install_from_path(Path::new(&plan.install_from))?;
        Ok(PluginStateEntry {
            source: PluginInstallSource::Registry(plan.package.name),
            ..entry
        })
    }

    pub fn uninstall(&self, name: &str) -> Result<()> {
        let mut state = PluginsState::load(&self.state_path)?;
        let Some(entry) = state.remove(name) else {
            return Err(PluginError::NotFound(name.to_string()).into());
        };
        let path = PathBuf::from(entry.install_dir);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        state.save(&self.state_path)?;
        self.write_lockfile(&state)?;
        Ok(())
    }

    pub fn set_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        self.sync_state_from_filesystem()?;
        let mut state = PluginsState::load(&self.state_path)?;
        let item = state
            .plugins
            .get_mut(name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;
        item.enabled = enabled;
        state.save(&self.state_path)?;
        self.write_lockfile(&state)?;
        Ok(())
    }

    pub fn trust(&self, id: &str) -> Result<()> {
        let mut store = TrustStore::load(&self.trust_store_path)?;
        store.trust_plugin(id);
        store.save(&self.trust_store_path)?;
        Ok(())
    }

    pub fn untrust(&self, id: &str) -> Result<()> {
        let mut store = TrustStore::load(&self.trust_store_path)?;
        store.untrust_plugin(id);
        store.save(&self.trust_store_path)?;
        Ok(())
    }

    pub fn trusted(&self) -> Result<Vec<String>> {
        let store = TrustStore::load(&self.trust_store_path)?;
        Ok(store.trusted_plugins.into_iter().collect())
    }

    pub fn doctor(&self) -> Result<Vec<String>> {
        let mut notes = Vec::new();
        if !self.plugins_dir.exists() {
            notes.push("plugins directory does not exist".into());
            return Ok(notes);
        }
        for package in discover_plugins(&self.plugins_dir)? {
            if !package.entry_path.exists() {
                notes.push(format!("{}: missing entry file", package.manifest.name));
                continue;
            }
            if let Some(expected) = &package.manifest.checksum {
                let actual = sha256_file(&package.entry_path)?;
                if !actual.eq_ignore_ascii_case(expected.trim_start_matches("sha256:")) {
                    notes.push(format!("{}: checksum mismatch", package.manifest.name));
                }
            }
        }
        if notes.is_empty() {
            notes.push("ok".into());
        }
        Ok(notes)
    }

    pub fn sync_state_from_filesystem(&self) -> Result<usize> {
        let discovered = discover_plugins(&self.plugins_dir)?;
        if discovered.is_empty() {
            return Ok(0);
        }

        let mut state = PluginsState::load(&self.state_path)?;
        let mut added = 0usize;
        for package in discovered {
            if state.plugins.contains_key(&package.manifest.name) {
                continue;
            }
            let entry = PluginStateEntry {
                name: package.manifest.name.clone(),
                version: package.manifest.version.clone(),
                enabled: true,
                trusted: false,
                checksum: package.manifest.checksum.clone().unwrap_or_default(),
                install_dir: package.root_dir.display().to_string(),
                installed_at: now_rfc3339(),
                source: PluginInstallSource::LocalPath(package.root_dir.display().to_string()),
            };
            state.upsert(entry);
            added += 1;
        }

        if added > 0 {
            state.save(&self.state_path)?;
            self.write_lockfile(&state)?;
        }

        Ok(added)
    }

    fn write_lockfile(&self, state: &PluginsState) -> Result<()> {
        let lock = PluginsLockfile::from_state(state);
        lock.save(&self.lock_path)
    }
}

fn read_manifest(plugin_dir: &Path) -> Result<PluginManifest> {
    let manifest_path = find_manifest_file(plugin_dir)?;
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    PluginManifest::from_toml_str(&raw)
}

fn find_manifest_file(plugin_dir: &Path) -> Result<PathBuf> {
    let primary = plugin_dir.join("dosh.plugin.toml");
    if primary.exists() {
        return Ok(primary);
    }
    let legacy = plugin_dir.join("plugin.toml");
    if legacy.exists() {
        return Ok(legacy);
    }
    Err(PluginError::InvalidManifest("missing dosh.plugin.toml".into()).into())
}

fn compute_manifest_checksum(manifest: &PluginManifest, installed_path: &Path) -> Result<String> {
    let entry = manifest.entry.clone().context("missing entry")?;
    let path = installed_path.join(entry);
    let digest = sha256_file(&path)?;
    Ok(format!("sha256:{digest}"))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn validate_plugin_entry(manifest: &PluginManifest, plugin_root: &Path) -> Result<()> {
    if manifest.source != PluginSource::Wasm {
        return Ok(());
    }
    let entry_rel = manifest
        .entry
        .clone()
        .context("missing wasm entry in manifest")?;
    let entry_path = plugin_root.join(entry_rel);
    let bytes = fs::read(&entry_path)
        .with_context(|| format!("failed to read wasm entry {}", entry_path.display()))?;
    if bytes.is_empty() {
        anyhow::bail!(
            "invalid plugin `{}`: wasm entry is empty. Build your plugin first, then install again.",
            manifest.name
        );
    }
    const WASM_MAGIC: &[u8; 4] = b"\0asm";
    if bytes.len() < 8 || &bytes[..4] != WASM_MAGIC {
        anyhow::bail!(
            "invalid plugin `{}`: wasm entry is not a valid wasm module (bad header).",
            manifest.name
        );
    }
    Ok(())
}

fn ensure_plugin_wasm_ready(plugin_dir: &Path, manifest: &PluginManifest) -> Result<()> {
    if manifest.source != PluginSource::Wasm {
        return Ok(());
    }
    let entry_rel = manifest
        .entry
        .clone()
        .context("missing wasm entry in manifest")?;
    let entry_path = plugin_dir.join(&entry_rel);
    let cargo_toml = plugin_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        // No Rust source to build from: validate existing wasm as-is.
        let bytes = fs::read(&entry_path).with_context(|| {
            format!(
                "plugin wasm entry is missing/invalid at `{}` and no Cargo.toml found for auto-build",
                entry_path.display()
            )
        })?;
        if bytes.len() < 8 || &bytes[..4] != b"\0asm" {
            anyhow::bail!(
                "invalid plugin wasm entry at `{}` (bad wasm header)",
                entry_path.display()
            );
        }
        return Ok(());
    }

    // If plugin source exists, always rebuild to avoid stale/stub wasm from old templates.
    let original_manifest = maybe_detach_nested_workspace(&cargo_toml)?;
    let mut built_target = None::<&str>;
    let mut last_status_ok = false;
    for target in ["wasm32-unknown-unknown", "wasm32-wasip1"] {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(target)
            .arg("--manifest-path")
            .arg(&cargo_toml)
            .status()
            .with_context(|| {
                format!(
                    "failed to run cargo build for plugin at {}",
                    plugin_dir.display()
                )
            })?;
        if status.success() {
            built_target = Some(target);
            last_status_ok = true;
            break;
        }
    }
    restore_manifest_if_needed(&cargo_toml, original_manifest)?;
    if !last_status_ok {
        anyhow::bail!(
            "failed to build wasm plugin automatically. Install target(s) with `rustup target add wasm32-wasip1 wasm32-unknown-unknown` and build plugin first."
        );
    }

    let crate_name = cargo_package_name(&cargo_toml)?;
    let target = built_target.expect("built target must be set on success");
    let artifact = plugin_dir
        .join("target")
        .join(target)
        .join("release")
        .join(format!("{}.wasm", crate_name.replace('-', "_")));
    if !artifact.exists() {
        anyhow::bail!(
            "plugin build finished but artifact not found: {}",
            artifact.display()
        );
    }
    fs::copy(&artifact, &entry_path).with_context(|| {
        format!(
            "failed to copy built wasm from {} to {}",
            artifact.display(),
            entry_path.display()
        )
    })?;
    Ok(())
}

fn cargo_package_name(cargo_toml: &Path) -> Result<String> {
    let raw = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
    let v: toml::Value = toml::from_str(&raw)?;
    let name = v
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing package.name in {}", cargo_toml.display()))?;
    Ok(name.to_string())
}

fn maybe_detach_nested_workspace(cargo_toml: &Path) -> Result<Option<String>> {
    let raw = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
    let has_workspace_table =
        raw.contains("\n[workspace]") || raw.trim_start().starts_with("[workspace]");
    if has_workspace_table {
        return Ok(None);
    }

    let Some(mut dir) = cargo_toml.parent() else {
        return Ok(None);
    };
    while let Some(parent) = dir.parent() {
        let candidate = parent.join("Cargo.toml");
        if candidate.exists() {
            let parent_raw = fs::read_to_string(&candidate).unwrap_or_default();
            if parent_raw.contains("[workspace]") {
                let mut patched = raw.clone();
                if !patched.ends_with('\n') {
                    patched.push('\n');
                }
                patched.push_str("\n[workspace]\n");
                fs::write(cargo_toml, patched)
                    .with_context(|| format!("failed to patch {}", cargo_toml.display()))?;
                return Ok(Some(raw));
            }
        }
        dir = parent;
    }
    Ok(None)
}

fn restore_manifest_if_needed(cargo_toml: &Path, original: Option<String>) -> Result<()> {
    if let Some(raw) = original {
        fs::write(cargo_toml, raw)
            .with_context(|| format!("failed to restore {}", cargo_toml.display()))?;
    }
    Ok(())
}
