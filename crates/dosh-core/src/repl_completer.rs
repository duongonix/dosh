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
        let env = EnvContext::new(self.cwd.clone());
        self.engine
            .complete(prefix, &env)
            .into_iter()
            .map(|item| Suggestion {
                value: item.insert_text.unwrap_or(item.value),
                description: item.description,
                style: None,
                extra: None,
                span: Span::new(start, pos),
                append_whitespace: true,
            })
            .collect()
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

