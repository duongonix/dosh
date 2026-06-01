use anyhow::{Result, anyhow};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseResponse {
    pub tag_name: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub fn fetch_latest_release(repo: &str) -> Result<ReleaseResponse> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::blocking::Client::builder()
        .user_agent("dosh-updater")
        .build()?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| anyhow!("cannot reach GitHub: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("GitHub API error: {}", resp.status()));
    }
    let release = resp
        .json::<ReleaseResponse>()
        .map_err(|e| anyhow!("invalid release response: {e}"))?;
    Ok(release)
}

pub fn find_asset<'a>(assets: &'a [ReleaseAsset], name: &str) -> Option<&'a ReleaseAsset> {
    assets.iter().find(|a| a.name == name)
}

pub fn find_checksums_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS") || a.name.ends_with(".sha256"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_assets() {
        let assets = vec![
            ReleaseAsset {
                name: "a".to_string(),
                browser_download_url: "u".to_string(),
            },
            ReleaseAsset {
                name: "SHA256SUMS".to_string(),
                browser_download_url: "s".to_string(),
            },
        ];
        assert!(find_asset(&assets, "a").is_some());
        assert!(find_checksums_asset(&assets).is_some());
    }
}
