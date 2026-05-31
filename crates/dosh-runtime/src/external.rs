use std::path::{Path, PathBuf};

pub(crate) fn resolve_program_for_spawn(name: &str) -> String {
    #[cfg(windows)]
    {
        resolve_program_for_spawn_windows(name).unwrap_or_else(|| name.to_string())
    }
    #[cfg(not(windows))]
    {
        name.to_string()
    }
}

#[cfg(windows)]
fn resolve_program_for_spawn_windows(name: &str) -> Option<String> {
    if name.trim().is_empty() {
        return None;
    }

    let path = Path::new(name);
    let has_ext = path.extension().is_some();
    if has_ext {
        return Some(name.to_string());
    }

    // Relative/absolute path without extension: try PATHEXT in-place first.
    if path.is_absolute() || name.contains('\\') || name.contains('/') {
        return resolve_with_extensions(path).or_else(|| Some(name.to_string()));
    }

    // Bare command: search PATH + PATHEXT explicitly.
    let exts = pathext_list();
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    Some(name.to_string())
}

#[cfg(windows)]
fn resolve_with_extensions(path: &Path) -> Option<String> {
    for ext in pathext_list() {
        let candidate = with_ext(path, &ext);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

#[cfg(windows)]
fn with_ext(path: &Path, ext_with_dot: &str) -> PathBuf {
    let ext = ext_with_dot.trim_start_matches('.');
    let mut out = path.to_path_buf();
    out.set_extension(ext);
    out
}

#[cfg(windows)]
fn pathext_list() -> Vec<String> {
    let raw = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC;.PS1".into());
    raw.split(';')
        .filter_map(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_ascii_lowercase())
            }
        })
        .collect()
}

