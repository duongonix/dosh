use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct EnvContext {
    cwd: PathBuf,
}

impl EnvContext {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    pub fn from_current_dir() -> Result<Self> {
        Ok(Self {
            cwd: std::env::current_dir()?,
        })
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn set_cwd(&mut self, path: impl Into<PathBuf>) {
        self.cwd = path.into();
    }

    pub fn change_dir(&mut self, path: &str) -> Result<()> {
        let target = PathBuf::from(path);
        let resolved = if target.is_absolute() {
            target
        } else {
            self.cwd.join(target)
        };

        if !resolved.exists() {
            bail!("directory does not exist: {}", resolved.display());
        }
        if !resolved.is_dir() {
            bail!("not a directory: {}", resolved.display());
        }

        self.cwd = resolved.canonicalize().unwrap_or(self.cwd.clone());
        Ok(())
    }
}
