use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedPathKind {
    Themes,
    Fonts,
    Plugins,
    Configs,
    Cache,
    Logs,
}

#[derive(Debug, Clone)]
pub struct DoshPaths {
    shared_root: PathBuf,
    user_config_root: PathBuf,
}

impl DoshPaths {
    pub fn detect() -> Result<Self> {
        let project = ProjectDirs::from("dev", "duongonix", "dosh")
            .context("failed to resolve platform-specific project directories")?;

        // Shared root by OS convention:
        // - Windows: %APPDATA%\\duongonix\\dosh\\shared
        // - macOS: ~/Library/Application Support/dev.duongonix.dosh/shared
        // - Linux: ~/.local/share/dosh/shared
        let shared_root = project.data_dir().join("shared");
        let user_config_root = BaseDirs::new()
            .map(|base| base.home_dir().join(".config").join("dosh"))
            .unwrap_or_else(|| shared_root.clone());

        Ok(Self {
            shared_root,
            user_config_root,
        })
    }

    pub fn shared_root(&self) -> &Path {
        &self.shared_root
    }

    pub fn path_for(&self, kind: SharedPathKind) -> PathBuf {
        let leaf = match kind {
            SharedPathKind::Themes => "themes",
            SharedPathKind::Fonts => "fonts",
            SharedPathKind::Plugins => "plugins",
            SharedPathKind::Configs => "configs",
            SharedPathKind::Cache => "cache",
            SharedPathKind::Logs => "logs",
        };
        self.shared_root.join(leaf)
    }

    pub fn themes_dir(&self) -> PathBuf {
        self.path_for(SharedPathKind::Themes)
    }

    pub fn fonts_dir(&self) -> PathBuf {
        self.path_for(SharedPathKind::Fonts)
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.user_config_root.join("plugins")
    }

    pub fn configs_dir(&self) -> PathBuf {
        self.path_for(SharedPathKind::Configs)
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.path_for(SharedPathKind::Cache)
    }

    pub fn user_cache_dir(&self) -> PathBuf {
        self.user_config_root.join("cache")
    }

    pub fn writable_cache_dir(&self) -> PathBuf {
        let primary = self.cache_dir();
        if is_dir_writable(&primary) {
            return primary;
        }
        let fallback = self.user_cache_dir();
        let _ = std::fs::create_dir_all(&fallback);
        fallback
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.path_for(SharedPathKind::Logs)
    }

    pub fn config_file(&self) -> PathBuf {
        self.configs_dir().join("config.toml")
    }

    pub fn theme_file(&self) -> PathBuf {
        self.configs_dir().join("theme.toml")
    }

    pub fn startup_file(&self) -> PathBuf {
        self.configs_dir().join("startup.dosh")
    }

    pub fn aliases_file(&self) -> PathBuf {
        self.configs_dir().join("aliases.dosh")
    }

    pub fn plugins_file(&self) -> PathBuf {
        self.configs_dir().join("plugins.toml")
    }

    pub fn history_db_file(&self) -> PathBuf {
        self.writable_cache_dir().join("history.sqlite3")
    }

    pub fn history_text_file(&self) -> PathBuf {
        self.writable_cache_dir().join("reedline.history")
    }
}

fn is_dir_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".dosh_write_probe");
    match std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_shared_root() {
        let paths = DoshPaths::detect().expect("path detection should work on supported OSes");
        assert!(paths.shared_root().to_string_lossy().len() > 1);
    }

    #[test]
    fn creates_expected_child_paths() {
        let paths = DoshPaths::detect().unwrap();
        let plugins = paths.plugins_dir();
        let plugins_s = plugins.to_string_lossy();
        assert!(plugins_s.contains(".config"));
        assert!(plugins_s.contains("dosh"));
        assert!(plugins_s.contains("plugins"));
    }
}
