mod context;
mod model;
mod path_commands;
mod rules;

use context::CompletionContext;
pub use model::CompletionItem;
use path_commands::{collect_path_commands, normalize_command_name};
use rules::CompletionRulesStore;

use dosh_builtins::BuiltinRegistry;
use dosh_config::DoshPaths;
use dosh_env::EnvContext;
use std::collections::BTreeSet;
use std::fs;

#[derive(Debug, Default)]
pub struct CompletionEngine {
    builtins: Vec<String>,
    rules: CompletionRulesStore,
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
            "export", "module", "mod", "test",
        ] {
            builtins.push(kw.to_string());
        }
        Self {
            builtins,
            rules: CompletionRulesStore::load(),
        }
    }

    pub fn complete(&self, input: &str, env_ctx: &EnvContext) -> Vec<CompletionItem> {
        let ctx = CompletionContext::from_input(input, env_ctx.cwd().display().to_string());

        if let Some(mut rule_items) = self.rules.complete(&ctx) {
            rule_items.retain(|i| i.value.starts_with(&ctx.current));
            rule_items.truncate(40);
            if !rule_items.is_empty() {
                return rule_items;
            }
        }

        let prefix = input.trim();
        if prefix.is_empty() {
            return Vec::new();
        }

        let mut seen = BTreeSet::new();
        let mut items = Vec::new();

        for b in &self.builtins {
            if b.starts_with(prefix) && seen.insert(b.clone()) {
                items.push(CompletionItem::new(b.clone(), Some("builtin".to_string())));
            }
        }

        for cmd in collect_path_commands() {
            if cmd.starts_with(prefix) && seen.insert(cmd.clone()) {
                items.push(CompletionItem::new(cmd, Some("external".to_string())));
            }
        }

        for cmd in self.rules.custom_commands() {
            if cmd.starts_with(prefix) && seen.insert(cmd.clone()) {
                items.push(CompletionItem::new(
                    cmd.to_string(),
                    Some("custom".to_string()),
                ));
            }
        }

        if let Ok(rd) = fs::read_dir(env_ctx.cwd()) {
            for entry in rd.flatten() {
                if let Some(name) = entry.file_name().to_str()
                    && name.starts_with(prefix)
                    && seen.insert(name.to_string())
                {
                    items.push(CompletionItem::new(
                        name.to_string(),
                        Some("path".to_string()),
                    ));
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
            all.insert(cmd.to_string());
        }
        for cmd in self.rules.custom_commands() {
            all.insert(cmd.to_string());
        }
        all.into_iter().collect()
    }
}

pub fn command_name_from_path(path: &std::path::Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    Some(normalize_command_name(stem))
}

pub fn load_custom_command_names() -> Vec<String> {
    let mut out = BTreeSet::new();
    if let Ok(paths) = DoshPaths::detect()
        && let Ok(rd) = fs::read_dir(paths.commands_dir())
    {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("dosh")
                && let Some(name) = command_name_from_path(&p)
            {
                out.insert(name);
            }
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleTarget;

    #[test]
    fn command_name_normalize() {
        let p = std::path::PathBuf::from("search-car.dosh");
        assert_eq!(command_name_from_path(&p).as_deref(), Some("search-car"));
    }

    #[test]
    fn context_position_for_arg() {
        let ctx = CompletionContext::from_input("search-car Toy", ".".into());
        assert_eq!(ctx.command, "search-car");
        assert_eq!(ctx.position, 1);
        assert_eq!(ctx.current, "Toy");
    }

    #[test]
    fn rule_target_matching() {
        let target = RuleTarget::Arg(2);
        assert!(target.matches(2, false, None, ""));
        assert!(!target.matches(1, false, None, ""));
    }
}
