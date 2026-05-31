#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub line: String,
    pub cursor: usize,
    pub words: Vec<String>,
    pub command: String,
    pub args: Vec<String>,
    pub current: String,
    pub previous: Option<String>,
    pub position: usize,
    pub cwd: String,
    pub is_flag: bool,
    pub flag: Option<String>,
}

impl CompletionContext {
    pub fn from_input(input: &str, cwd: String) -> Self {
        let line = input.to_string();
        let cursor = line.len();
        let mut words = shell_words::split(input).unwrap_or_default();
        let ends_space = input.chars().last().is_some_and(|c| c.is_whitespace());
        if ends_space {
            words.push(String::new());
        }

        let command = words.first().cloned().unwrap_or_default();
        let current = words.last().cloned().unwrap_or_default();
        let previous = if words.len() >= 2 {
            Some(words[words.len() - 2].clone())
        } else {
            None
        };

        let args = if words.len() >= 2 {
            words[1..words.len().saturating_sub(1)].to_vec()
        } else {
            Vec::new()
        };
        let position = args.len() + 1;
        let is_flag = current.starts_with('-');
        let flag = previous.clone().filter(|p| p.starts_with('-'));

        Self {
            line,
            cursor,
            words,
            command,
            args,
            current,
            previous,
            position,
            cwd,
            is_flag,
            flag,
        }
    }
}
