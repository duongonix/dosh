pub mod model;
pub mod paths;

use anyhow::Result;

pub use model::DoshConfig;
pub use paths::{DoshPaths, SharedPathKind};

pub fn load_default_config() -> Result<DoshConfig> {
    let paths = DoshPaths::detect()?;
    let path = paths.config_file();
    if !path.exists() {
        return Ok(DoshConfig::default());
    }
    let text = std::fs::read_to_string(path)?;
    let cfg: DoshConfig = toml::from_str(&text)?;
    Ok(cfg)
}
