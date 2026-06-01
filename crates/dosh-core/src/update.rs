use anyhow::Result;
use dosh_config::DoshPaths;
use dosh_updater::{
    Checkpoint, DEFAULT_REPO, UpdateConfig, check_latest, load_checkpoint, save_checkpoint,
    should_check, update,
};
use std::time::{SystemTime, UNIX_EPOCH};

const UPDATE_INTERVAL_SECS: u64 = 24 * 60 * 60;

pub fn check_and_prompt_update(interactive: bool, auto_mode: bool) -> Result<()> {
    let config = UpdateConfig {
        repo: DEFAULT_REPO.to_string(),
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        bin_name: "dosh".to_string(),
        interactive,
    };
    let now = now_unix_secs();
    let path = update_state_path()?;
    if auto_mode {
        let cp = load_checkpoint(&path).ok().flatten();
        if !should_check(now, cp.as_ref(), UPDATE_INTERVAL_SECS) {
            return Ok(());
        }
        let latest = check_latest(&config);
        match latest {
            Ok(Some(info)) => {
                println!(
                    "Update available: v{} -> v{} (run `dosh update`)",
                    info.current_version, info.latest_version
                );
                let cp = Checkpoint {
                    last_checked_unix_secs: now,
                    last_latest_version: Some(info.latest_version),
                };
                let _ = save_checkpoint(&path, &cp);
            }
            Ok(None) => {
                let cp = Checkpoint {
                    last_checked_unix_secs: now,
                    last_latest_version: None,
                };
                let _ = save_checkpoint(&path, &cp);
            }
            Err(_) => {}
        }
        return Ok(());
    }

    let res = update(config)?;
    println!("{}", res.message);
    let cp = Checkpoint {
        last_checked_unix_secs: now,
        last_latest_version: Some(res.latest_version),
    };
    let _ = save_checkpoint(&path, &cp);
    Ok(())
}

pub fn run_update_command() -> Result<()> {
    check_and_prompt_update(true, false)
}

fn update_state_path() -> Result<std::path::PathBuf> {
    let p = DoshPaths::detect()?;
    Ok(p.cache_dir().join("update-check.json"))
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
