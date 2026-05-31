mod repl_highlighter;
mod ui_error;

use dosh_completion::CompletionEngine;
use dosh_config::DoshPaths;
use dosh_env::EnvContext;
use dosh_highlight::Highlighter;
use dosh_history::HistoryStore;
use dosh_parser::Parser;
use dosh_prompt::PromptEngine;
use dosh_runtime::{Runtime, RuntimeOutcome};
use nu_ansi_term::{Color, Style};
use reedline::{
    ColumnarMenu, DefaultCompleter, DefaultHinter, DefaultPrompt, DefaultPromptSegment,
    DefaultValidator, Emacs, FileBackedHistory, KeyCode, KeyModifiers, ListMenu, MenuBuilder,
    Reedline, ReedlineEvent, ReedlineMenu, Signal, default_emacs_keybindings,
};
use repl_highlighter::DoshReedlineHighlighter;
use std::path::PathBuf;
use std::io::Write;
use std::time::Instant;
use ui_error::format_error_report;

pub type ShellResult<T = ()> = anyhow::Result<T>;

#[derive(Debug, Clone)]
pub struct ShellConfig {
    pub interactive: bool,
    pub command: Option<String>,
    pub start_path: Option<PathBuf>,
    pub login: bool,
    pub no_config: bool,
    pub safe_mode: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            interactive: true,
            command: None,
            start_path: None,
            login: false,
            no_config: false,
            safe_mode: false,
        }
    }
}

pub struct Shell {
    config: ShellConfig,
    parser: Parser,
    runtime: Runtime,
    env: EnvContext,
    history: HistoryStore,
    completion: CompletionEngine,
    prompt_engine: PromptEngine,
    highlighter: Highlighter,
    last_exit_code: i32,
    last_duration_ms: u128,
    initialized: bool,
}

impl Shell {
    pub fn new() -> Self {
        Self::with_config(ShellConfig::default())
    }

    pub fn with_config(config: ShellConfig) -> Self {
        let env = default_shell_env(config.start_path.clone());
        Self {
            config,
            parser: Parser::new(),
            runtime: Runtime::new(),
            env,
            history: init_history_store(),
            completion: CompletionEngine::new(),
            prompt_engine: PromptEngine::new("classic"),
            highlighter: Highlighter,
            last_exit_code: 0,
            last_duration_ms: 0,
            initialized: false,
        }
    }

    pub fn run(&mut self) -> ShellResult {
        self.initialize_session()?;
        if let Some(command) = self.config.command.clone() {
            self.execute_line(&command)?;
            return Ok(());
        }

        if !self.config.interactive {
            return Ok(());
        }

        self.repl()
    }

    fn repl(&mut self) -> ShellResult {
        let mut line_editor = self.build_line_editor()?;

        loop {
            let term_width = crossterm::terminal::size()
                .map(|(w, _)| w as usize)
                .unwrap_or(120);
            let pctx = self.prompt_engine.collect_context(
                self.env.cwd(),
                self.last_exit_code,
                self.last_duration_ms,
            );
            let rendered = self.prompt_engine.render(&pctx, term_width);
            let prompt = DefaultPrompt::new(
                DefaultPromptSegment::Basic(rendered.left),
                if rendered.right.is_empty() {
                    DefaultPromptSegment::Empty
                } else {
                    DefaultPromptSegment::Basic(rendered.right)
                },
            );

            let trimmed = match line_editor.read_line(&prompt) {
                Ok(Signal::Success(line)) => line.trim().to_string(),
                Ok(Signal::CtrlD) => break,
                Ok(Signal::CtrlC) => {
                    self.last_exit_code = 130;
                    continue;
                }
                Err(err) => {
                    self.last_exit_code = 1;
                    eprintln!(
                        "{}",
                        format_error_report(&anyhow::anyhow!("reedline error: {err}"))
                    );
                    continue;
                }
            };
            if trimmed.is_empty() {
                continue;
            }

            let _highlighted = self.highlighter.highlight_line(&trimmed);

            match self.execute_line(&trimmed) {
                Ok(outcome) => {
                    self.last_exit_code = outcome.exit_code;
                    if outcome.should_exit {
                        break;
                    }
                }
                Err(err) => {
                    self.last_exit_code = 1;
                    eprintln!("{}", format_error_report(&err));
                    continue;
                }
            }
        }

        Ok(())
    }

    fn execute_line(&mut self, line: &str) -> ShellResult<RuntimeOutcome> {
        let started = Instant::now();
        let _ = self.history.add(line);
        let _ = append_reedline_history_entry(line);

        if let Some(outcome) = self.run_prompt_meta_commands(line) {
            self.last_exit_code = outcome.exit_code;
            return Ok(outcome);
        }

        if let Some(outcome) = self.run_phase2_meta_commands(line) {
            self.last_exit_code = outcome.exit_code;
            return Ok(outcome);
        }

        let script = self.parser.parse_line(line)?;
        let outcome = self.runtime.execute(&script, &mut self.env)?;

        if let Some(out) = &outcome.output {
            println!("{out}");
        }

        self.last_exit_code = outcome.exit_code;
        self.last_duration_ms = started.elapsed().as_millis();
        Ok(outcome)
    }

    fn initialize_session(&mut self) -> ShellResult {
        if self.initialized {
            return Ok(());
        }
        self.initialized = true;
        if self.config.no_config || self.config.safe_mode {
            return Ok(());
        }
        let Ok(paths) = DoshPaths::detect() else {
            return Ok(());
        };
        for dir in [paths.configs_dir(), paths.cache_dir(), paths.plugins_dir()] {
            let _ = std::fs::create_dir_all(dir);
        }
        let aliases_file = paths.aliases_file();
        if aliases_file.exists() {
            let content = std::fs::read_to_string(&aliases_file)?;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let _ = self.execute_line(trimmed);
            }
        }
        let startup_file = paths.startup_file();
        if startup_file.exists() {
            let content = std::fs::read_to_string(&startup_file)?;
            match self.parser.parse_script(&content) {
                Ok(script) => {
                    if let Err(err) = self.runtime.execute(&script, &mut self.env) {
                        eprintln!(
                            "{}",
                            format_error_report(&anyhow::anyhow!("startup script failed: {err}"))
                        );
                    }
                }
                Err(err) => eprintln!(
                    "{}",
                    format_error_report(&anyhow::anyhow!("startup parse failed: {err}"))
                ),
            }
        }
        Ok(())
    }

    fn run_phase2_meta_commands(&self, line: &str) -> Option<RuntimeOutcome> {
        let trimmed = line.trim();

        if let Some(prefix) = trimmed.strip_prefix(":complete ") {
            let suggestions = self.completion.complete(prefix, &self.env);
            for item in suggestions {
                println!("{}\t{}", item.value, item.description.unwrap_or_default());
            }
            return Some(RuntimeOutcome::ok());
        }

        if let Some(query) = trimmed.strip_prefix(":history ") {
            for item in self.history.fuzzy_search(query, 10) {
                println!("{item}");
            }
            return Some(RuntimeOutcome::ok());
        }

        None
    }

    fn run_prompt_meta_commands(&mut self, line: &str) -> Option<RuntimeOutcome> {
        let trimmed = line.trim();
        if !trimmed.starts_with("prompt ") {
            return None;
        }
        let parts = shell_words::split(trimmed).ok()?;
        let sub = parts.get(1).map(|s| s.as_str()).unwrap_or("show");
        match sub {
            "show" => {
                let ctx = self.prompt_engine.collect_context(
                    self.env.cwd(),
                    self.last_exit_code,
                    self.last_duration_ms,
                );
                let r = self.prompt_engine.render(&ctx, 120);
                println!("{}", r.left);
            }
            "reload" => {
                let name = self.prompt_engine.theme_name().to_string();
                self.prompt_engine.reload_theme(&name);
                println!("prompt reloaded");
            }
            "theme" => {
                let name = parts.get(2).map(|s| s.as_str()).unwrap_or("classic");
                self.prompt_engine.reload_theme(name);
                println!("prompt theme: {name}");
            }
            "segments" => {
                for s in self.prompt_engine.segment_names() {
                    println!("{s}");
                }
            }
            "doctor" => {
                let nerd_font = std::env::var("TERM").unwrap_or_default().contains("xterm");
                let unicode_ok = true;
                println!("theme={}", self.prompt_engine.theme_name());
                println!("nerd_font_hint={nerd_font}");
                println!("unicode_hint={unicode_ok}");
            }
            "preview" => {
                let name = parts.get(2).map(|s| s.as_str()).unwrap_or("classic");
                let mut engine = PromptEngine::new(name);
                let ctx = engine.collect_context(
                    self.env.cwd(),
                    self.last_exit_code,
                    self.last_duration_ms,
                );
                let r = engine.render(&ctx, 120);
                println!("{}", r.left);
            }
            _ => println!("prompt expects: show|reload|theme|segments|doctor|preview"),
        }
        Some(RuntimeOutcome::ok())
    }

    fn build_line_editor(&self) -> ShellResult<Reedline> {
        let history_path = reedline_history_path();
        let history = Box::new(FileBackedHistory::with_file(2_000, history_path)?);

        let commands = self.completion.candidate_words();
        let completer = Box::new(DefaultCompleter::new_with_wordlen(commands, 2));
        let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
        let history_menu = Box::new(ListMenu::default().with_name("history_menu"));

        let mut keybindings = default_emacs_keybindings();
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu("completion_menu".to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );
        keybindings.add_binding(
            KeyModifiers::CONTROL,
            KeyCode::Char('r'),
            ReedlineEvent::Menu("history_menu".to_string()),
        );
        let edit_mode = Box::new(Emacs::new(keybindings));

        let editor = Reedline::create()
            .with_history(history)
            .with_completer(completer)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_menu(ReedlineMenu::HistoryMenu(history_menu))
            .with_edit_mode(edit_mode)
            .with_quick_completions(true)
            .with_partial_completions(true)
            .with_hinter(Box::new(
                DefaultHinter::default().with_style(Style::new().italic().fg(Color::DarkGray)),
            ))
            .with_highlighter(Box::new(DoshReedlineHighlighter::new(
                self.completion.candidate_words(),
            )))
            .with_validator(Box::new(DefaultValidator));

        Ok(editor)
    }
}

fn append_reedline_history_entry(line: &str) -> ShellResult {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let path = reedline_history_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{trimmed}")?;
    Ok(())
}

fn default_shell_env(start_path: Option<PathBuf>) -> EnvContext {
    if let Some(path) = start_path.filter(|p| p.exists() && p.is_dir()) {
        return EnvContext::new(path);
    }
    EnvContext::from_current_dir().unwrap_or_else(|_| EnvContext::new(".".into()))
}

fn init_history_store() -> HistoryStore {
    if let Ok(paths) = DoshPaths::detect() {
        let db_path = paths.history_db_file();
        if let Ok(store) = HistoryStore::new_persistent(&db_path) {
            return store;
        }
    }

    HistoryStore::new()
}

fn reedline_history_path() -> PathBuf {
    if let Ok(paths) = DoshPaths::detect() {
        let path = paths.history_text_file();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return path;
    }
    PathBuf::from(".dosh_reedline.history")
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_default_is_interactive() {
        let shell = Shell::new();
        assert!(shell.config.interactive);
    }

    #[test]
    fn shell_can_run_single_command_mode() {
        let mut shell = Shell::with_config(ShellConfig {
            interactive: false,
            command: Some("echo test".to_string()),
            start_path: None,
            login: false,
            no_config: true,
            safe_mode: true,
        });
        assert!(shell.run().is_ok());
    }
}
