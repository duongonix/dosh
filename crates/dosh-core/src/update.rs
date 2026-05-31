use anyhow::{Context, Result, anyhow};
use semver::Version;
use std::io::{self, Write};
use std::process::{Command, Stdio};

const REPO: &str = "duongonix/dosh";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current: String,
    pub latest_tag: String,
}

impl UpdateInfo {
    pub fn latest_version(&self) -> String {
        self.latest_tag.trim_start_matches('v').to_string()
    }
}

pub fn check_for_update() -> Result<Option<UpdateInfo>> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let latest_tag = fetch_latest_tag()?;
    let latest = latest_tag.trim_start_matches('v');

    let current_v =
        Version::parse(&current).with_context(|| format!("invalid current version: {current}"))?;
    let latest_v =
        Version::parse(latest).with_context(|| format!("invalid latest version: {latest}"))?;

    if latest_v > current_v {
        Ok(Some(UpdateInfo {
            current,
            latest_tag,
        }))
    } else {
        Ok(None)
    }
}

pub fn check_and_prompt_update(interactive: bool) -> Result<()> {
    let update = match check_for_update() {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let Some(info) = update else {
        return Ok(());
    };
    render_update_banner(&info);
    if !interactive {
        return Ok(());
    }
    if ask_yes_no("Update now? [y/N]: ")? {
        run_update_installer()?;
    }
    Ok(())
}

pub fn run_update_command() -> Result<()> {
    let check = check_for_update();
    let update = match check {
        Ok(v) => v,
        Err(err) => {
            println!(
                "\x1b[93mUpdate check unavailable:\x1b[0m {}",
                err.to_string().trim()
            );
            return Ok(());
        }
    };
    match update {
        Some(info) => {
            render_update_banner(&info);
            if ask_yes_no("Install this update now? [y/N]: ")? {
                run_update_installer()?;
            }
        }
        None => {
            println!("\x1b[92mDosh is up to date.\x1b[0m");
        }
    }
    Ok(())
}

fn fetch_latest_tag() -> Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", "dosh-update-check")
        .timeout(std::time::Duration::from_secs(3))
        .call()
        .map_err(|e| anyhow!("update check failed: {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| anyhow!("invalid update response: {e}"))?;

    extract_json_string_field(&text, "tag_name")
        .ok_or_else(|| anyhow!("update response missing tag_name"))
}

fn extract_json_string_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let idx = json.find(&needle)?;
    let tail = &json[idx + needle.len()..];
    let colon = tail.find(':')?;
    let mut s = tail[colon + 1..].trim_start();
    if !s.starts_with('"') {
        return None;
    }
    s = &s[1..];
    let end = s.find('"')?;
    Some(s[..end].to_string())
}

fn render_update_banner(info: &UpdateInfo) {
    let latest = info.latest_version();
    let width = 78usize;
    let line = "─".repeat(width);
    println!();
    println!("\x1b[36m┌{line}┐\x1b[0m");
    println!("{}", box_line("Dosh update available", width));
    println!(
        "{}",
        box_line(
            &format!("Current: {}   Latest: {}", info.current, latest),
            width
        )
    );
    let release_url = format!("https://github.com/{REPO}/releases/tag/{}", info.latest_tag);
    println!("{}", box_line(&format!("Release: {release_url}"), width));
    println!("\x1b[36m└{line}┘\x1b[0m");
}

fn box_line(text: &str, width: usize) -> String {
    let plain = text
        .chars()
        .take(width.saturating_sub(2))
        .collect::<String>();
    let pad = width
        .saturating_sub(2)
        .saturating_sub(plain.chars().count());
    format!(
        "\x1b[36m│\x1b[0m {}{}\x1b[36m│\x1b[0m",
        plain,
        " ".repeat(pad)
    )
}

fn ask_yes_no(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let v = input.trim().to_ascii_lowercase();
    Ok(matches!(v.as_str(), "y" | "yes"))
}

fn run_update_installer() -> Result<()> {
    #[cfg(windows)]
    {
        let status = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg("iwr -useb https://raw.githubusercontent.com/duongonix/dosh/main/scripts/install.ps1 | iex")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(anyhow!("installer exited with status {}", status));
        }
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("sh")
            .arg("-c")
            .arg("curl -fsSL https://raw.githubusercontent.com/duongonix/dosh/main/scripts/install.sh | sh")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(anyhow!("installer exited with status {}", status));
        }
    }
    println!("\x1b[92mUpdate completed.\x1b[0m Open a new terminal and run `dosh --version`.");
    Ok(())
}
