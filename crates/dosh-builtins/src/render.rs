use dosh_value::{Record, Value};

#[derive(Debug, Clone, Copy)]
pub struct TableRenderOptions {
    pub unicode: bool,
    pub color: bool,
    pub max_width: usize,
    pub truncate: bool,
}

impl Default for TableRenderOptions {
    fn default() -> Self {
        Self {
            unicode: true,
            color: std::env::var_os("NO_COLOR").is_none(),
            max_width: detect_terminal_width(),
            truncate: true,
        }
    }
}

pub fn render_value_as_table(value: &Value, opts: TableRenderOptions) -> String {
    let rows = to_rows(value);
    if rows.is_empty() {
        return String::new();
    }

    let mut headers = Vec::new();
    for row in &rows {
        for key in row.keys() {
            if !headers.contains(key) {
                headers.push(key.clone());
            }
        }
    }

    let mut widths = headers
        .iter()
        .map(|h| h.chars().count())
        .collect::<Vec<_>>();
    for row in &rows {
        for (i, header) in headers.iter().enumerate() {
            let cell = row.get(header).map(|v| v.to_string()).unwrap_or_default();
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    fit_widths(&mut widths, opts.max_width, headers.len(), opts.truncate);

    if opts.unicode {
        render_unicode(&headers, &rows, &widths, opts)
    } else {
        render_ascii(&headers, &rows, &widths, opts)
    }
}

fn to_rows(value: &Value) -> Vec<Record> {
    match value {
        Value::Table(t) => t.rows.clone(),
        Value::Record(r) => vec![r.clone()],
        Value::List(items) => items
            .iter()
            .filter_map(|v| match v {
                Value::Record(r) => Some(r.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn detect_terminal_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(100)
}

fn fit_widths(widths: &mut [usize], max_width: usize, cols: usize, truncate: bool) {
    if cols == 0 || !truncate {
        return;
    }
    let sep = if cols > 1 { (cols - 1) * 3 } else { 0 };
    let mut total = widths.iter().sum::<usize>() + sep;
    if total <= max_width {
        return;
    }
    let min_col = 6usize;
    while total > max_width {
        if let Some((idx, _)) = widths.iter().enumerate().max_by_key(|(_, w)| **w) {
            if widths[idx] <= min_col {
                break;
            }
            widths[idx] -= 1;
            total -= 1;
        } else {
            break;
        }
    }
}

fn trunc(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count <= width {
        return s.to_string();
    }
    if width <= 1 {
        return "…".to_string();
    }
    let mut out = s.chars().take(width - 1).collect::<String>();
    out.push('…');
    out
}

fn header_text(text: &str, color: bool) -> String {
    if color {
        format!("\x1b[1;36m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn render_ascii(
    headers: &[String],
    rows: &[Record],
    widths: &[usize],
    opts: TableRenderOptions,
) -> String {
    let mut out = Vec::new();
    let hdr = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let padded = format!("{:<w$}", trunc(h, widths[i]), w = widths[i]);
            header_text(&padded, opts.color)
        })
        .collect::<Vec<_>>()
        .join(" | ");
    out.push(hdr);
    out.push(
        widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("-+-"),
    );
    for row in rows {
        let line = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let cell = row.get(h).map(|v| v.to_string()).unwrap_or_default();
                format!("{:<w$}", trunc(&cell, widths[i]), w = widths[i])
            })
            .collect::<Vec<_>>()
            .join(" | ");
        out.push(line);
    }
    out.join("\n")
}

fn render_unicode(
    headers: &[String],
    rows: &[Record],
    widths: &[usize],
    opts: TableRenderOptions,
) -> String {
    let mut out = Vec::new();
    let top = format!(
        "┌{}┐",
        widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┬")
    );
    let mid = format!(
        "├{}┤",
        widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┼")
    );
    let bot = format!(
        "└{}┘",
        widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┴")
    );
    out.push(top);
    out.push(format!(
        "│ {} │",
        headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let padded = format!("{:<w$}", trunc(h, widths[i]), w = widths[i]);
                header_text(&padded, opts.color)
            })
            .collect::<Vec<_>>()
            .join(" │ ")
    ));
    out.push(mid);
    for row in rows {
        out.push(format!(
            "│ {} │",
            headers
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let cell = row.get(h).map(|v| v.to_string()).unwrap_or_default();
                    format!("{:<w$}", trunc(&cell, widths[i]), w = widths[i])
                })
                .collect::<Vec<_>>()
                .join(" │ ")
        ));
    }
    out.push(bot);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_table_works() {
        let mut r = Record::new();
        r.insert("name".into(), Value::String("dosh".into()));
        let t = Value::Table(dosh_value::Table::new(vec![r]));
        let s = render_value_as_table(&t, TableRenderOptions::default());
        assert!(s.contains("name"));
    }
}
