use dosh_completion::CompletionEngine;
use dosh_env::EnvContext;
use reedline::{Completer, Span, Suggestion};
use std::path::PathBuf;

pub struct DoshReedlineCompleter {
    engine: CompletionEngine,
    cwd: PathBuf,
}

impl DoshReedlineCompleter {
    pub fn new(engine: CompletionEngine, cwd: PathBuf) -> Self {
        Self { engine, cwd }
    }
}

impl Completer for DoshReedlineCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let pos = pos.min(line.len());
        let prefix = &line[..pos];
        let start = find_token_start(prefix);
        let token = &prefix[start..pos];
        let env = EnvContext::new(self.cwd.clone());
        self.engine
            .complete(prefix, &env)
            .into_iter()
            .map(|item| {
                let description = render_description(&item);
                let base = item
                    .insert_text
                    .clone()
                    .unwrap_or_else(|| item.value.clone());
                let (value, append_whitespace) = if base == token && !line.ends_with(' ') {
                    (format!("{base} "), false)
                } else {
                    (base, true)
                };
                Suggestion {
                    value,
                    description,
                    style: None,
                    extra: None,
                    span: Span::new(start, pos),
                    append_whitespace,
                }
            })
            .collect()
    }
}

fn render_description(item: &dosh_completion::CompletionItem) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(icon) = &item.icon {
        parts.push(icon.clone());
    }
    if let Some(kind) = &item.kind {
        parts.push(kind.clone());
    }
    if let Some(desc) = &item.description {
        parts.push(desc.clone());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn find_token_start(s: &str) -> usize {
    let mut start = s.len();
    for (idx, ch) in s.char_indices().rev() {
        if ch.is_whitespace() {
            break;
        }
        start = idx;
    }
    start
}
