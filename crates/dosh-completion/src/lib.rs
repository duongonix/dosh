use dosh_builtins::BuiltinRegistry;
use dosh_env::EnvContext;
use std::collections::BTreeSet;
use std::env;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub value: String,
    pub description: Option<String>,
}

#[derive(Debug, Default)]
pub struct CompletionEngine {
    builtins: Vec<String>,
}

impl CompletionEngine {
    pub fn new() -> Self {
        let builtin_names: Vec<String> = BuiltinRegistry::new()
            .metadata(None)
            .into_iter()
            .map(|m| m.name.to_string())
            .collect::<Vec<String>>();
        let mut builtins = builtin_names;
        for kw in [
            "fn", "if", "else", "for", "in", "match", "return", "break", "continue", "use",
            "export", "module", "test",
        ] {
            builtins.push(kw.to_string());
        }
        Self { builtins }
    }

    pub fn complete(&self, input: &str, env_ctx: &EnvContext) -> Vec<CompletionItem> {
        let prefix = input.trim();
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut seen = BTreeSet::new();
        let mut items = Vec::new();

        for b in &self.builtins {
            if b.starts_with(prefix) && seen.insert(b.clone()) {
                items.push(CompletionItem {
                    value: b.clone(),
                    description: Some("builtin".to_string()),
                });
            }
        }

        for cmd in collect_path_commands() {
            if cmd.starts_with(prefix) && seen.insert(cmd.clone()) {
                items.push(CompletionItem {
                    value: cmd,
                    description: Some("external".to_string()),
                });
            }
        }

        if let Ok(rd) = fs::read_dir(env_ctx.cwd()) {
            for entry in rd.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(prefix) && seen.insert(name.to_string()) {
                        items.push(CompletionItem {
                            value: name.to_string(),
                            description: Some("path".to_string()),
                        });
                    }
                }
            }
        }

        items.truncate(20);
        items
    }

    pub fn candidate_words(&self) -> Vec<String> {
        let mut all = BTreeSet::new();
        for b in &self.builtins {
            all.insert(b.clone());
        }
        for cmd in collect_path_commands() {
            all.insert(cmd);
        }
        all.into_iter().collect()
    }
}

fn collect_path_commands() -> Vec<String> {
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

fn normalize_command_name(name: &str) -> String {
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
