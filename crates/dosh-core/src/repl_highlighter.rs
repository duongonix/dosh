use nu_ansi_term::{Color, Style};
use reedline::{Highlighter, StyledText};
use std::collections::HashSet;

pub struct DoshReedlineHighlighter {
    known_commands: HashSet<String>,
}

impl DoshReedlineHighlighter {
    pub fn new(known_commands: Vec<String>) -> Self {
        Self {
            known_commands: known_commands.into_iter().collect(),
        }
    }
}

impl Highlighter for DoshReedlineHighlighter {
    fn highlight(&self, line: &str, _cursor: usize) -> StyledText {
        let mut styled = StyledText::new();

        if line.trim().is_empty() {
            styled.push((Style::new(), line.to_string()));
            return styled;
        }
        let mut i = 0usize;
        let bytes = line.as_bytes();
        let mut expect_command = true;

        while i < line.len() {
            let c = bytes[i] as char;

            if c.is_whitespace() {
                let start = i;
                i += 1;
                while i < line.len() && (bytes[i] as char).is_whitespace() {
                    i += 1;
                }
                styled.push((Style::new(), line[start..i].to_string()));
                continue;
            }

            if let Some((op, len)) = read_operator(&line[i..]) {
                styled.push((Style::new().fg(Color::Yellow), op.to_string()));
                i += len;
                expect_command = matches!(op, "|" | ";" | "&&" | "||");
                continue;
            }

            let start = i;
            i += 1;
            while i < line.len() {
                let ch = bytes[i] as char;
                if ch.is_whitespace() || is_operator_start(ch) {
                    break;
                }
                i += 1;
            }
            let token = &line[start..i];

            if expect_command {
                if is_known_command(token, &self.known_commands) {
                    styled.push((Style::new().fg(Color::Green), token.to_string()));
                } else {
                    styled.push((Style::new().fg(Color::Red), token.to_string()));
                }
                expect_command = false;
            } else if is_flag_token(token) {
                styled.push((Style::new().fg(Color::DarkGray), token.to_string()));
            } else if is_field_like_token(token) {
                styled.push((Style::new().fg(Color::Blue), token.to_string()));
            } else {
                styled.push((Style::new(), token.to_string()));
            }
        }

        styled
    }
}

fn is_operator_start(c: char) -> bool {
    matches!(c, '|' | '>' | '<' | '&' | ';')
}

fn read_operator(s: &str) -> Option<(&str, usize)> {
    if s.starts_with(">>") {
        return Some((">>", 2));
    }
    if s.starts_with("&&") {
        return Some(("&&", 2));
    }
    if s.starts_with("||") {
        return Some(("||", 2));
    }
    let ch = s.chars().next()?;
    if matches!(ch, '|' | '>' | '<' | '&' | ';') {
        Some((&s[..ch.len_utf8()], ch.len_utf8()))
    } else {
        None
    }
}

fn is_known_command(token: &str, known: &HashSet<String>) -> bool {
    let normalized = normalize_for_match(token);
    known.contains(token)
        || known.contains(&normalized)
        || token.starts_with(':')
        || token == "prompt"
        || token == "if"
        || token == "for"
        || token == "match"
        || token == "let"
        || token == "fn"
        || token == "module"
        || token == "mod"
        || token == "import"
}

fn is_flag_token(token: &str) -> bool {
    token.starts_with('-') && token.len() > 1
}

fn is_field_like_token(token: &str) -> bool {
    matches!(
        token,
        "name"
            | "size"
            | "modified"
            | "created"
            | "updated"
            | "type"
            | "path"
            | "pid"
            | "cpu"
            | "memory"
            | "status"
    )
}

fn normalize_for_match(token: &str) -> String {
    #[cfg(windows)]
    {
        let lowered = token.to_ascii_lowercase();
        for ext in [".exe", ".cmd", ".bat", ".ps1", ".com"] {
            if let Some(stripped) = lowered.strip_suffix(ext) {
                return stripped.to_string();
            }
        }
        lowered
    }

    #[cfg(not(windows))]
    {
        token.to_string()
    }
}
