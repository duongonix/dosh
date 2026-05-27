use crate::app::EditorApp;
use crate::highlight::highlight_line;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &EditorApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(frame.area());

    let editor_area = chunks[0];
    let footer_area = chunks[1];
    let height = editor_area.height.saturating_sub(2) as usize;
    let width = editor_area.width.saturating_sub(2) as usize;
    let lnw = line_number_width(app.buffer.lines.len()) + 3;
    let text_w = width.saturating_sub(lnw);

    let mut lines = Vec::new();
    for i in 0..height {
        let row = app.scroll_y + i;
        if row >= app.buffer.lines.len() {
            lines.push(Line::from(""));
            continue;
        }
        let mut prefix = format!("{:>lnw$}", row + 1, lnw = lnw - 2);
        prefix.push(' ');
        let marker = if app.error_lines.contains(&row) {
            Span::styled("!", Style::default().fg(Color::Red))
        } else {
            Span::styled(" ", Style::default().fg(Color::DarkGray))
        };
        let raw = app.buffer.line(row);
        let visible = raw
            .chars()
            .skip(app.scroll_x)
            .take(text_w)
            .collect::<String>();
        let mut highlighted = highlight_line(&app.buffer.path, &visible).spans;
        let mut row_spans = vec![
            Span::styled(prefix, Style::default().fg(Color::DarkGray)),
            marker,
            Span::raw(" "),
        ];
        row_spans.append(&mut highlighted);
        lines.push(Line::from(row_spans));
    }

    let editor = Paragraph::new(lines).block(
        Block::default()
            .title(format!("dedit  {}", app.buffer.path.display()))
            .borders(Borders::ALL),
    );
    frame.render_widget(editor, editor_area);

    let mut status = format!(
        "{}  Ln {}, Col {}  [{}]  {}",
        if app.buffer.dirty {
            "● modified"
        } else {
            "saved"
        },
        app.cursor_y + 1,
        app.cursor_x + 1,
        app.mode_name(),
        "Ctrl+S Save | Ctrl+Q Quit | Ctrl+F Find | Tab Complete"
    );
    if let Some(s) = &app.suggestion {
        status.push_str(&format!(" | suggest: {}", s));
    }
    let footer = Paragraph::new(status).style(Style::default().fg(Color::Black).bg(Color::Cyan));
    frame.render_widget(footer, footer_area);

    let cx = (app.cursor_x.saturating_sub(app.scroll_x) + lnw + 2) as u16 + editor_area.x;
    let cy = (app.cursor_y.saturating_sub(app.scroll_y) + 1) as u16 + editor_area.y;
    frame.set_cursor_position((cx, cy));
}

fn line_number_width(max_line: usize) -> usize {
    max_line.max(1).to_string().len().max(3)
}
