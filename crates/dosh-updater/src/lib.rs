mod archive;
mod download;
mod github;
mod platform;
mod ui;
mod verify;

use anyhow::{Context, Result, anyhow, bail};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const DEFAULT_REPO: &str = "duongonix/dosh";

#[derive(Debug, Clone)]
pub struct UpdateConfig {
    pub repo: String,
    pub current_version: String,
    pub bin_name: String,
    pub interactive: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub current_version: String,
    pub latest_version: String,
    pub updated: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub last_checked_unix_secs: u64,
    pub last_latest_version: Option<String>,
}

pub fn should_check(
    now_unix_secs: u64,
    checkpoint: Option<&Checkpoint>,
    interval_secs: u64,
) -> bool {
    let Some(cp) = checkpoint else {
        return true;
    };
    now_unix_secs.saturating_sub(cp.last_checked_unix_secs) >= interval_secs
}

pub fn load_checkpoint(path: &Path) -> Result<Option<Checkpoint>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let cp: Checkpoint = serde_json::from_str(&text)?;
    Ok(Some(cp))
}

pub fn save_checkpoint(path: &Path, cp: &Checkpoint) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(cp)?)?;
    Ok(())
}

pub fn check_latest(config: &UpdateConfig) -> Result<Option<UpdateResult>> {
    let release = github::fetch_latest_release(&config.repo)?;
    let latest = normalize_version_tag(&release.tag_name)?;
    let current = Version::parse(&config.current_version)
        .with_context(|| format!("invalid current version: {}", config.current_version))?;
    if latest <= current {
        return Ok(None);
    }
    Ok(Some(UpdateResult {
        current_version: config.current_version.clone(),
        latest_version: latest.to_string(),
        updated: false,
        message: "update available".to_string(),
    }))
}

pub fn update(config: UpdateConfig) -> Result<UpdateResult> {
    let release = github::fetch_latest_release(&config.repo)?;
    let latest = normalize_version_tag(&release.tag_name)?;
    let current = Version::parse(&config.current_version)
        .with_context(|| format!("invalid current version: {}", config.current_version))?;
    if latest <= current {
        return Ok(UpdateResult {
            current_version: config.current_version,
            latest_version: latest.to_string(),
            updated: false,
            message: "Dosh is up to date.".to_string(),
        });
    }

    ui::render_update_box(
        &format!("v{}", current),
        &format!("v{}", latest),
        &release.body,
    );
    if !ui::is_tty() {
        return Ok(UpdateResult {
            current_version: current.to_string(),
            latest_version: latest.to_string(),
            updated: false,
            message: "Update available (non-interactive mode)".to_string(),
        });
    }
    if config.interactive && !ui::ask_yes_no("Update now? [Y/n]: ", true)? {
        return Ok(UpdateResult {
            current_version: current.to_string(),
            latest_version: latest.to_string(),
            updated: false,
            message: "Update cancelled.".to_string(),
        });
    }

    let platform = platform::current_target()?;
    let wanted_name = platform.asset_name(&latest.to_string());
    let asset = github::find_asset(&release.assets, &wanted_name)
        .ok_or_else(|| anyhow!("no matching asset for platform: {wanted_name}"))?;
    let checksums = github::find_checksums_asset(&release.assets);

    let tmp = tempfile::tempdir()?;
    let archive_path = tmp.path().join(&asset.name);
    download::download_to_path(&asset.browser_download_url, &archive_path, &asset.name)?;

    if let Some(sum_asset) = checksums {
        let sums_path = tmp.path().join(&sum_asset.name);
        download::download_to_path(&sum_asset.browser_download_url, &sums_path, &sum_asset.name)?;
        verify::verify_against_checksum_asset(&archive_path, &sums_path, &asset.name)?;
    } else {
        eprintln!("warning: no checksum asset found for this release");
        if !ui::ask_yes_no("Continue without checksum verification? [y/N]: ", false)? {
            bail!("checksum missing. update aborted");
        }
    }

    let extract_dir = tmp.path().join("extract");
    fs::create_dir_all(&extract_dir)?;
    archive::extract_archive(&archive_path, &extract_dir)?;

    let bin_file_name = if cfg!(windows) {
        format!("{}.exe", config.bin_name)
    } else {
        config.bin_name.clone()
    };
    let new_bin = archive::find_binary(&extract_dir, &bin_file_name)
        .ok_or_else(|| anyhow!("binary not found in package: {bin_file_name}"))?;

    self_replace::self_replace(&new_bin).map_err(|e| anyhow!("self replace failed: {e}"))?;

    Ok(UpdateResult {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        updated: true,
        message: format!(
            "Update complete.\nUpdated v{} -> v{}\nRestart Dosh to use the new version.",
            current, latest
        ),
    })
}

fn normalize_version_tag(tag: &str) -> Result<Version> {
    let normalized = tag.trim().trim_start_matches('v');
    Version::parse(normalized).with_context(|| format!("invalid release tag: {tag}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn version_compare_works() {
        let c = Version::parse("1.0.3").expect("parse current");
        let l = super::normalize_version_tag("v1.0.4").expect("parse latest");
        assert!(l > c);
    }

    #[test]
    fn check_interval_works() {
        let cp = Checkpoint {
            last_checked_unix_secs: 100,
            last_latest_version: None,
        };
        assert!(!should_check(100 + 3600, Some(&cp), 24 * 3600));
        assert!(should_check(100 + 24 * 3600, Some(&cp), 24 * 3600));
        assert!(should_check(100, None, 24 * 3600));
    }

    #[test]
    fn find_binary_supports_nested_and_flat() {
        let dir = tempfile::tempdir().expect("tmp");
        let flat = dir.path().join("dosh.exe");
        fs::write(&flat, b"x").expect("write flat");
        assert!(archive::find_binary(dir.path(), "dosh.exe").is_some());
        fs::remove_file(&flat).expect("remove flat");
        let nested_dir = dir.path().join("nested");
        fs::create_dir_all(&nested_dir).expect("mkdir nested");
        fs::write(nested_dir.join("dosh.exe"), b"x").expect("write nested");
        assert!(archive::find_binary(dir.path(), "dosh.exe").is_some());
    }

    #[test]
    fn checksum_verify_works() {
        let dir = tempfile::tempdir().expect("tmp");
        let artifact = dir.path().join("a.zip");
        fs::write(&artifact, b"abc").expect("write artifact");
        let hash = verify::sha256_file(&artifact).expect("sha256");
        let sums = dir.path().join("SHA256SUMS");
        let mut f = fs::File::create(&sums).expect("create sums");
        writeln!(f, "{}  a.zip", hash).expect("write sums");
        verify::verify_against_checksum_asset(&artifact, &sums, "a.zip").expect("verify checksum");
    }
}
