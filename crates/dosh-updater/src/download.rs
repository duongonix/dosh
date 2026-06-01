use anyhow::{Result, anyhow};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub fn download_to_path(url: &str, out_path: &Path, label: &str) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("dosh-updater")
        .build()?;
    let mut resp = client
        .get(url)
        .send()
        .map_err(|e| anyhow!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("download failed with status {}", resp.status()));
    }
    let total = resp.content_length();
    let pb = match total {
        Some(n) => {
            let pb = ProgressBar::new(n);
            pb.set_style(
                ProgressStyle::with_template(
                    "{spinner:.cyan} {msg}\n{bar:40.cyan/blue} {percent:>3}%\n{bytes}/{total_bytes} {bytes_per_sec} ETA {eta}",
                )?
                .progress_chars("##-"),
            );
            pb
        }
        None => {
            let pb = ProgressBar::new_spinner();
            pb.set_style(ProgressStyle::with_template(
                "{spinner:.cyan} {msg} {bytes}",
            )?);
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            pb
        }
    };
    pb.set_message(format!("Downloading {label}"));
    let mut file = File::create(out_path)?;
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = resp.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        pb.inc(n as u64);
    }
    pb.finish_with_message(format!("Downloaded {label}"));
    Ok(())
}
