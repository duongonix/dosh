mod context;
mod model;
mod path_commands;
mod providers;
mod rules;
mod script_provider;

use context::CompletionContext;
pub use model::CompletionItem;
use path_commands::{collect_path_commands, normalize_command_name};
use providers::context_aware_completions;
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
            sort_items(&mut rule_items, &ctx.current);
            rule_items.truncate(40);
            if !rule_items.is_empty() {
                return rule_items;
            }
        }

        let mut contextual = context_aware_completions(&ctx);
        contextual.retain(|i| value_matches(&i.value, &ctx.current));
        sort_items(&mut contextual, &ctx.current);
        if !contextual.is_empty() {
            contextual.truncate(40);
            return contextual;
        }

        let is_command_position =
            !input.chars().last().is_some_and(|c| c.is_whitespace()) && ctx.words.len() <= 1;
        if !is_command_position {
            return Vec::new();
        }

        let prefix = ctx.current.as_str();
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
                items.push(CompletionItem::new(cmd, Some("custom".to_string())));
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

        sort_items(&mut items, prefix);
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
            all.insert(cmd);
        }
        all.into_iter().collect()
    }

    pub fn reload(&self) {
        self.rules.reload();
    }

    pub fn list_rules(&self) -> Vec<String> {
        self.rules.list_rules()
    }

    pub fn show_rules_for(&self, command: &str) -> Vec<String> {
        self.rules.show_rules_for(command)
    }
}

fn value_matches(value: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let v = value.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    v.starts_with(&n) || fuzzy_contains(&v, &n)
}

fn fuzzy_contains(value: &str, needle: &str) -> bool {
    let mut it = value.chars();
    for ch in needle.chars() {
        if !it.any(|c| c == ch) {
            return false;
        }
    }
    true
}

fn sort_items(items: &mut [CompletionItem], needle: &str) {
    let n = needle.to_ascii_lowercase();
    items.sort_by(|a, b| {
        let ap = a.priority.unwrap_or(0);
        let bp = b.priority.unwrap_or(0);
        let av = a.value.to_ascii_lowercase();
        let bv = b.value.to_ascii_lowercase();
        let a_exact = av == n;
        let b_exact = bv == n;
        let a_prefix = av.starts_with(&n);
        let b_prefix = bv.starts_with(&n);
        b_exact
            .cmp(&a_exact)
            .then_with(|| b_prefix.cmp(&a_prefix))
            .then_with(|| bp.cmp(&ap))
            .then_with(|| av.cmp(&bv))
    });
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
    use dosh_env::EnvContext;

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

    #[test]
    fn no_duplicate_command_completion_after_space() {
        let engine = CompletionEngine::new();
        let env = EnvContext::new(std::env::current_dir().unwrap());
        let out = engine.complete("devflow ", &env);
        assert!(out.is_empty(), "should not suggest command name again");
    }

    #[test]
    fn completion_uses_segment_after_last_pipe() {
        let ctx = CompletionContext::from_input("ls | devflow t", ".".into());
        assert_eq!(ctx.command, "devflow");
        assert_eq!(ctx.current, "t");
    }
}
