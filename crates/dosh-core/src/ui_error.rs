use anyhow::Error;
use nu_ansi_term::Color;

pub(crate) fn format_error_report(err: &Error) -> String {
    let mut out = String::new();
    out.push_str(&Color::Red.bold().paint("error").to_string());
    out.push_str(": ");
    out.push_str(&err.to_string());

    let chain: Vec<String> = err
        .chain()
        .skip(1)
        .map(std::string::ToString::to_string)
        .collect();
    if !chain.is_empty() {
        out.push('\n');
        out.push_str(&Color::Yellow.paint("caused by:").to_string());
        for (idx, cause) in chain.iter().enumerate() {
            out.push('\n');
            out.push_str(&format!("  {}. {}", idx + 1, cause));
        }
    }

    if let Some(hint) = build_hint(err) {
        out.push('\n');
        out.push_str(&Color::Cyan.paint("hint").to_string());
        out.push_str(": ");
        out.push_str(&hint);
    }

    out
}

fn build_hint(err: &Error) -> Option<String> {
    let text = err.to_string().to_ascii_lowercase();
    if text.contains("program not found") || text.contains("command not found") {
        return Some(
            "check the command name, or use ^<command> to run it as an external command"
                .to_string(),
        );
    }
    if text.contains("permission denied") || text.contains("access is denied") {
        return Some(
            "check file/directory permissions, or run the terminal with appropriate privileges"
                .to_string(),
        );
    }
    if text.contains("no such file")
        || text.contains("not found")
        || text.contains("cannot find the path")
    {
        return Some("check your current directory (`pwd`) and the path you entered".to_string());
    }
    if text.contains("parse") || text.contains("unexpected token") || text.contains("unterminated")
    {
        return Some("check command syntax, especially quotes, pipes, and parentheses".to_string());
    }
    None
}
