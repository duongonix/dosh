use anyhow::{anyhow, bail};
use dosh_value::{FilesizeValue, Record, Value};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmMode {
    Normal,
    Fast,
    Trash,
}

#[derive(Debug, Clone)]
pub struct RmOptions {
    pub recursive: bool,
    pub mode: RmMode,
}

#[derive(Debug, Clone, Default)]
pub struct ScanStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct DeleteSummary {
    pub deleted_files: u64,
    pub deleted_dirs: u64,
    pub skipped: u64,
    pub failed: Vec<(PathBuf, String)>,
    pub elapsed_ms: u128,
    pub mode: String,
}

#[derive(Debug, Clone, Default)]
pub struct DeletePlan {
    pub files: Vec<PathBuf>,
    pub dirs_desc: Vec<PathBuf>,
    pub stats: ScanStats,
}

impl DeleteSummary {
    pub fn to_record(&self) -> Record {
        let mut r = Record::new();
        r.insert("mode".into(), Value::String(self.mode.clone()));
        r.insert(
            "deleted_files".into(),
            Value::Int(self.deleted_files as i64),
        );
        r.insert("deleted_dirs".into(), Value::Int(self.deleted_dirs as i64));
        r.insert("skipped".into(), Value::Int(self.skipped as i64));
        r.insert("failed".into(), Value::Int(self.failed.len() as i64));
        r.insert("elapsed_ms".into(), Value::Int(self.elapsed_ms as i64));
        if !self.failed.is_empty() {
            let rows = self
                .failed
                .iter()
                .map(|(p, e)| {
                    let mut row = Record::new();
                    row.insert("path".into(), Value::String(p.display().to_string()));
                    row.insert("reason".into(), Value::String(e.clone()));
                    Value::Record(row)
                })
                .collect::<Vec<_>>();
            r.insert("failures".into(), Value::List(rows));
        }
        r
    }
}

pub fn detect_fast_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "node_modules" | "target" | "dist" | ".next" | ".nuxt" | ".svelte-kit" | "coverage"
    )
}

pub fn reject_protected_path(path: &Path) -> anyhow::Result<()> {
    let p = normalize_for_compare(path)?;
    if is_protected(&p) {
        bail!(
            "Refusing to delete protected system path: {}",
            path.display()
        );
    }
    Ok(())
}

fn normalize_for_compare(path: &Path) -> anyhow::Result<String> {
    let abs = if path.exists() {
        path.canonicalize()?
    } else if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut norm = abs
        .to_string_lossy()
        .replace('\\', "/")
        .replace("//?/", "")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    if norm.is_empty() {
        norm = "/".to_string();
    }
    Ok(norm)
}

fn is_protected(norm: &str) -> bool {
    #[cfg(windows)]
    {
        if norm.len() == 2 && norm.ends_with(':') {
            return true;
        }
        let protected = [
            "c:",
            "c:/",
            "c:/windows",
            "c:/program files",
            "c:/program files (x86)",
            "c:/users",
            "c:/windows/system32",
        ];
        protected.contains(&norm)
    }
    #[cfg(target_os = "macos")]
    {
        let protected = [
            "/",
            "/system",
            "/applications",
            "/bin",
            "/usr",
            "/etc",
            "/home",
        ];
        protected.contains(&norm)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let protected = ["/", "/bin", "/usr", "/etc", "/home"];
        protected.contains(&norm)
    }
}

pub fn scan_path(path: &Path) -> DeletePlan {
    let mut plan = DeletePlan::default();
    let mut dirs_with_depth = Vec::<(usize, PathBuf)>::new();
    if path.is_file() {
        let bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
        plan.files.push(path.to_path_buf());
        plan.stats.files = 1;
        plan.stats.bytes = bytes;
        return plan;
    }
    for e in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        let p = e.path().to_path_buf();
        if e.file_type().is_file() {
            let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            plan.files.push(p);
            plan.stats.files += 1;
            plan.stats.bytes += bytes;
        } else if e.file_type().is_dir() {
            dirs_with_depth.push((e.depth(), p));
            plan.stats.dirs += 1;
        }
    }
    dirs_with_depth.sort_by_key(|b| std::cmp::Reverse(b.0));
    plan.dirs_desc = dirs_with_depth.into_iter().map(|(_, p)| p).collect();
    plan
}

pub fn dry_run_record(target: &Path, mode: RmMode, stats: &ScanStats) -> Record {
    let mut r = Record::new();
    r.insert("path".into(), Value::String(target.display().to_string()));
    r.insert("mode".into(), Value::String(mode_name(mode)));
    r.insert("files".into(), Value::Int(stats.files as i64));
    r.insert("directories".into(), Value::Int(stats.dirs as i64));
    r.insert(
        "size".into(),
        Value::Filesize(FilesizeValue { bytes: stats.bytes }),
    );
    r.insert("deleted".into(), Value::Bool(false));
    r
}

fn mode_name(mode: RmMode) -> String {
    match mode {
        RmMode::Normal => "normal",
        RmMode::Fast => "fast",
        RmMode::Trash => "trash",
    }
    .to_string()
}

pub fn execute_delete(target: &Path, options: &RmOptions) -> anyhow::Result<DeleteSummary> {
    reject_protected_path(target)?;
    let start = Instant::now();
    let mut summary = DeleteSummary {
        mode: mode_name(options.mode),
        ..DeleteSummary::default()
    };
    if options.mode == RmMode::Trash {
        trash::delete(target).map_err(|e| anyhow!("{e}"))?;
        summary.deleted_files = 1;
        summary.elapsed_ms = start.elapsed().as_millis();
        return Ok(summary);
    }
    if target.is_file() {
        match delete_file_with_retry(target) {
            Ok(_) => summary.deleted_files = 1,
            Err(e) => summary.failed.push((target.to_path_buf(), e.to_string())),
        }
        summary.elapsed_ms = start.elapsed().as_millis();
        return Ok(summary);
    }
    if !options.recursive {
        fs::remove_dir(target)?;
        summary.deleted_dirs = 1;
        summary.elapsed_ms = start.elapsed().as_millis();
        return Ok(summary);
    }

    let plan = scan_path(target);
    if options.mode == RmMode::Fast {
        let deleted_files = AtomicU64::new(0);
        let skipped = AtomicU64::new(0);
        let failures = std::sync::Mutex::new(Vec::<(PathBuf, String)>::new());
        plan.files
            .par_iter()
            .for_each(|file| match delete_file_with_retry(file) {
                Ok(_) => {
                    deleted_files.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut g) = failures.lock() {
                        g.push((file.clone(), e.to_string()));
                    }
                }
            });
        summary.deleted_files = deleted_files.load(Ordering::Relaxed);
        summary.skipped = skipped.load(Ordering::Relaxed);
        if let Ok(mut f) = failures.lock() {
            summary.failed.append(&mut f);
        }
    } else {
        for file in &plan.files {
            match delete_file_with_retry(file) {
                Ok(_) => summary.deleted_files += 1,
                Err(e) => summary.failed.push((file.clone(), e.to_string())),
            }
        }
    }

    for dir in &plan.dirs_desc {
        if delete_dir_if_empty(dir).is_ok() {
            summary.deleted_dirs += 1;
        }
    }
    summary.elapsed_ms = start.elapsed().as_millis();
    Ok(summary)
}

fn delete_dir_if_empty(dir: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        if let Ok(md) = fs::metadata(dir) {
            let mut perms = md.permissions();
            if perms.readonly() {
                perms.set_readonly(false);
                let _ = fs::set_permissions(dir, perms);
            }
        }
    }
    fs::remove_dir(dir)
}

fn delete_file_with_retry(path: &Path) -> std::io::Result<()> {
    let delays = [
        Duration::from_millis(50),
        Duration::from_millis(100),
        Duration::from_millis(250),
        Duration::from_millis(500),
        Duration::from_millis(1000),
    ];
    for (i, delay) in delays.iter().enumerate() {
        #[cfg(windows)]
        if let Ok(md) = fs::metadata(path) {
            let mut perms = md.permissions();
            if perms.readonly() {
                perms.set_readonly(false);
                let _ = fs::set_permissions(path, perms);
            }
        }
        match fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                if i == delays.len() - 1 {
                    return Err(e);
                }
                thread::sleep(*delay);
            }
        }
    }
    Err(std::io::Error::other("unknown delete retry error"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fast_candidate_detection() {
        assert!(detect_fast_candidate(Path::new("node_modules")));
        assert!(!detect_fast_candidate(Path::new("src")));
    }

    #[test]
    fn scan_counts_files() {
        let dir = tempdir().expect("tmp");
        fs::write(dir.path().join("a.txt"), b"x").expect("write");
        fs::create_dir_all(dir.path().join("nested")).expect("mkdir");
        fs::write(dir.path().join("nested").join("b.txt"), b"yy").expect("write");
        let plan = scan_path(dir.path());
        assert_eq!(plan.stats.files, 2);
        assert!(plan.stats.bytes >= 3);
    }

    #[test]
    fn protected_path_is_rejected() {
        #[cfg(windows)]
        {
            assert!(reject_protected_path(Path::new("C:\\")).is_err());
        }
        #[cfg(unix)]
        {
            assert!(reject_protected_path(Path::new("/")).is_err());
        }
    }
}
