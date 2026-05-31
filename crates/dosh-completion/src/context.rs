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
    pub env: Vec<(String, String)>,
    pub is_flag: bool,
    pub flag: Option<String>,
    pub command_path: Option<String>,
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

        let seg_start = words
            .iter()
            .rposition(|w| w == "|")
            .map(|i| i + 1)
            .unwrap_or(0);
        let seg_words = if seg_start < words.len() {
            words[seg_start..].to_vec()
        } else {
            Vec::new()
        };

        let command = seg_words.first().cloned().unwrap_or_default();
        let current = seg_words.last().cloned().unwrap_or_default();
        let previous = if seg_words.len() >= 2 {
            Some(seg_words[seg_words.len() - 2].clone())
        } else {
            None
        };

        let args = if seg_words.len() >= 2 {
            seg_words[1..seg_words.len().saturating_sub(1)].to_vec()
        } else {
            Vec::new()
        };
        let position = args.len() + 1;
        let is_flag = current.starts_with('-');
        let flag = previous.clone().filter(|p| p.starts_with('-'));
        let env = std::env::vars().collect::<Vec<_>>();
        let command_path = resolve_command_path(&command);

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
            env,
            is_flag,
            flag,
            command_path,
        }
    }
}

fn resolve_command_path(command: &str) -> Option<String> {
    if command.is_empty() {
        return None;
    }
    let path_var = std::env::var_os("PATH")?;
    #[cfg(windows)]
    let exts = [".exe", ".cmd", ".bat", ".ps1", ".com"];
    for dir in std::env::split_paths(&path_var) {
        #[cfg(windows)]
        {
            for ext in exts {
                let p = dir.join(format!("{command}{ext}"));
                if p.is_file() {
                    return Some(p.display().to_string());
                }
            }
        }
        #[cfg(not(windows))]
        {
            let p = dir.join(command);
            if p.is_file() {
                return Some(p.display().to_string());
            }
        }
    }
    None
}
