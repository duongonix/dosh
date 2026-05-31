use std::collections::BTreeSet;
use std::env;
use std::fs;

pub fn collect_path_commands() -> Vec<String> {
    let mut out = BTreeSet::new();
    let Some(path_var) = env::var_os("PATH") else {
        return Vec::new();
    };

    for dir in env::split_paths(&path_var) {
        let Ok(read_dir) = fs::read_dir(dir) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            if let Some(name) = entry.file_name().to_str() {
                let normalized = normalize_command_name(name);
                if !normalized.is_empty() {
                    out.insert(normalized);
                }
            }
        }
    }

    out.into_iter().collect()
}

pub fn normalize_command_name(name: &str) -> String {
    #[cfg(windows)]
    {
        let lowered = name.to_ascii_lowercase();
        for ext in [".exe", ".cmd", ".bat", ".ps1", ".com"] {
            if let Some(stripped) = lowered.strip_suffix(ext) {
                return stripped.to_string();
            }
        }
        lowered
    }

    #[cfg(not(windows))]
    {
        name.to_string()
    }
}
