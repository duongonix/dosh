use crate::context::CompletionContext;
use crate::model::CompletionItem;
use std::process::Command;
use std::time::Duration;

pub fn context_aware_completions(ctx: &CompletionContext) -> Vec<CompletionItem> {
    if ctx.words.is_empty() {
        return Vec::new();
    }
    match ctx.command.as_str() {
        "git" => git_completions(ctx),
        "cargo" => cargo_completions(ctx),
        "npm" | "pnpm" => npm_like_completions(ctx),
        "docker" => docker_completions(ctx),
        _ => Vec::new(),
    }
}

fn git_completions(ctx: &CompletionContext) -> Vec<CompletionItem> {
    if ctx.position == 1 {
        return vec![
            item("add", "subcommand"),
            item("branch", "subcommand"),
            item("checkout", "subcommand"),
            item("switch", "subcommand"),
            item("commit", "subcommand"),
            item("pull", "subcommand"),
            item("push", "subcommand"),
            item("rebase", "subcommand"),
            item("status", "subcommand"),
        ];
    }
    let sub = ctx.args.first().map(String::as_str).unwrap_or_default();
    if matches!(sub, "checkout" | "switch" | "merge" | "rebase") && ctx.position >= 2 {
        return run_lines("git", &["branch", "--format=%(refname:short)"])
            .into_iter()
            .map(|v| item(&v, "branch"))
            .collect();
    }
    Vec::new()
}

fn cargo_completions(ctx: &CompletionContext) -> Vec<CompletionItem> {
    if ctx.position == 1 {
        return vec![
            item("build", "subcommand"),
            item("check", "subcommand"),
            item("clippy", "subcommand"),
            item("doc", "subcommand"),
            item("fmt", "subcommand"),
            item("run", "subcommand"),
            item("test", "subcommand"),
            item("bench", "subcommand"),
        ];
    }
    Vec::new()
}

fn npm_like_completions(ctx: &CompletionContext) -> Vec<CompletionItem> {
    if ctx.position == 1 {
        return vec![
            item("run", "subcommand"),
            item("install", "subcommand"),
            item("test", "subcommand"),
            item("build", "subcommand"),
        ];
    }
    let sub = ctx.args.first().map(String::as_str).unwrap_or_default();
    if sub == "run" && ctx.position == 2 {
        let mut from_npm = run_lines("npm", &["run", "--silent"]);
        if from_npm.is_empty() {
            from_npm = run_lines("pnpm", &["run", "--silent"]);
        }
        return from_npm
            .into_iter()
            .filter(|l| !l.contains("Lifecycle") && !l.contains("Commands"))
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .map(|v| item(&v, "script"))
            .collect();
    }
    Vec::new()
}

fn docker_completions(ctx: &CompletionContext) -> Vec<CompletionItem> {
    if ctx.position == 1 {
        return vec![
            item("build", "subcommand"),
            item("compose", "subcommand"),
            item("exec", "subcommand"),
            item("images", "subcommand"),
            item("logs", "subcommand"),
            item("ps", "subcommand"),
            item("pull", "subcommand"),
            item("push", "subcommand"),
            item("run", "subcommand"),
        ];
    }
    Vec::new()
}

fn run_lines(program: &str, args: &[&str]) -> Vec<String> {
    let output = Command::new(program).args(args).output();
    let Ok(output) = output else {
        return Vec::new();
    };
    let _timeout = Duration::from_millis(250);
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn item(value: &str, kind: &str) -> CompletionItem {
    CompletionItem {
        value: value.to_string(),
        description: None,
        kind: Some(kind.to_string()),
        icon: None,
        insert_text: None,
        priority: Some(5),
    }
}
