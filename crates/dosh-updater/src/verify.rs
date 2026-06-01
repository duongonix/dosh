use anyhow::{Result, anyhow, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn verify_against_checksum_asset(
    artifact: &Path,
    sums_file: &Path,
    asset_name: &str,
) -> Result<()> {
    let actual = sha256_file(artifact)?;
    let expected = read_expected_checksum(sums_file, asset_name)?
        .ok_or_else(|| anyhow!("checksum missing for asset: {asset_name}"))?;
    if actual.eq_ignore_ascii_case(&expected) {
        return Ok(());
    }
    bail!("Checksum verification failed. Update aborted.");
}

fn read_expected_checksum(sums_file: &Path, asset_name: &str) -> Result<Option<String>> {
    let text = fs::read_to_string(sums_file)?;
    if sums_file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .ends_with(".sha256")
    {
        let first = text.lines().next().unwrap_or_default();
        let hash = first.split_whitespace().next().unwrap_or_default().trim();
        if hash.is_empty() {
            return Ok(None);
        }
        return Ok(Some(hash.to_string()));
    }
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let mut parts = l.split_whitespace();
        let hash = parts.next().unwrap_or_default();
        let name = parts.last().unwrap_or_default().trim_start_matches('*');
        if name == asset_name && !hash.is_empty() {
            return Ok(Some(hash.to_string()));
        }
    }
    Ok(None)
}
