use anyhow::Result;
use std::io::{self, IsTerminal, Write};

pub fn is_tty() -> bool {
    io::stdout().is_terminal()
}

pub fn ask_yes_no(prompt: &str, default_yes: bool) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let v = input.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok(default_yes);
    }
    Ok(matches!(v.as_str(), "y" | "yes"))
}

pub fn render_update_box(current: &str, latest: &str, body: &str) {
    let notes = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(4)
        .map(|l| l.trim().to_string())
        .collect::<Vec<_>>();
    let width = 45usize;
    let top = "─".repeat(width);
    println!("╭{top}╮");
    line("Dosh Update Available", width);
    println!("├{top}┤");
    line(&format!("Current : {current}"), width);
    line(&format!("Latest  : {latest}"), width);
    line("", width);
    line("Release Notes:", width);
    for n in notes {
        let text = if n.len() > width - 2 {
            n[..width - 5].to_string() + "..."
        } else {
            n
        };
        line(&text, width);
    }
    line("", width);
    line("Update now? [Y/n]", width);
    println!("╰{top}╯");
}

fn line(text: &str, width: usize) {
    let visible = text.chars().take(width).collect::<String>();
    let pad = width.saturating_sub(visible.chars().count());
    println!("│{}{}│", visible, " ".repeat(pad));
}
