#[derive(Debug, Default)]
pub struct Highlighter;

impl Highlighter {
    pub fn highlight_line(&self, line: &str) -> String {
        let trimmed = line.trim_start();
        let builtin = [
            "cd", "pwd", "echo", "exit", "print", "first", "last", "slice", "where", "sort-by",
            "table", "assert",
        ]
        .iter()
        .any(|cmd| trimmed == *cmd || trimmed.starts_with(&format!("{cmd} ")));
        let keyword = [
            "fn ", "if ", "else", "for ", "match ", "return", "break", "continue", "use ",
            "export ", "module ", "mod ", "test ",
        ]
        .iter()
        .any(|kw| trimmed.starts_with(kw));

        if builtin {
            format!("\x1b[32m{line}\x1b[0m")
        } else if keyword {
            format!("\x1b[36m{line}\x1b[0m")
        } else if trimmed.starts_with('#') {
            format!("\x1b[90m{line}\x1b[0m")
        } else {
            line.to_string()
        }
    }
}
