use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

struct HlCtx {
    ps: SyntaxSet,
    theme: Theme,
}

static HL: OnceLock<HlCtx> = OnceLock::new();
static CACHE: OnceLock<Mutex<HashMap<u64, Line<'static>>>> = OnceLock::new();

fn ctx() -> &'static HlCtx {
    HL.get_or_init(|| {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let theme = ts
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .or_else(|| ts.themes.values().next().cloned())
            .unwrap_or_default();
        HlCtx { ps, theme }
    })
}

pub fn highlight_line(path: &Path, line: &str) -> Line<'static> {
    let c = ctx();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let key = cache_key(&ext, line);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(cached) = guard.get(&key)
    {
        return cached.clone();
    }

    let syntax = syntax_for_ext(&c.ps, &ext);
    let mut h = HighlightLines::new(syntax, &c.theme);
    let rendered = match h.highlight_line(line, &c.ps) {
        Ok(ranges) => {
            let spans = ranges
                .into_iter()
                .map(|(style, text)| Span::styled(text.to_string(), to_ratatui_style(style)))
                .collect::<Vec<_>>();
            Line::from(spans)
        }
        Err(_) => Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::White),
        )),
    };
    if let Ok(mut guard) = cache.lock() {
        if guard.len() > 8_192 {
            guard.clear();
        }
        guard.insert(key, rendered.clone());
    }
    rendered
}

fn syntax_for_ext<'a>(ps: &'a SyntaxSet, ext: &str) -> &'a SyntaxReference {
    ps.find_syntax_by_extension(ext)
        .unwrap_or_else(|| ps.find_syntax_plain_text())
}

fn cache_key(ext: &str, line: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ext.hash(&mut hasher);
    line.hash(&mut hasher);
    hasher.finish()
}

fn to_ratatui_style(s: SynStyle) -> Style {
    let fg = Color::Rgb(s.foreground.r, s.foreground.g, s.foreground.b);
    let mut st = Style::default().fg(fg);
    if s.font_style
        .contains(syntect::highlighting::FontStyle::BOLD)
    {
        st = st.add_modifier(Modifier::BOLD);
    }
    if s.font_style
        .contains(syntect::highlighting::FontStyle::ITALIC)
    {
        st = st.add_modifier(Modifier::ITALIC);
    }
    if s.font_style
        .contains(syntect::highlighting::FontStyle::UNDERLINE)
    {
        st = st.add_modifier(Modifier::UNDERLINED);
    }
    st
}
