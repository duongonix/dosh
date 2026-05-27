use crate::context::{PromptContext, detect_git, detect_project, detect_runtime_versions};
use crate::segments::SegmentRegistry;
use crate::theme::{PromptTheme, ThemeLoader};
use dosh_config::DoshPaths;
use nu_ansi_term::{Color, Style};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Default)]
pub struct PromptRenderResult {
    pub left: String,
    pub right: String,
    pub continuation: String,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    cwd: std::path::PathBuf,
    git: crate::context::GitContext,
    project: crate::context::ProjectContext,
    runtimes: crate::context::RuntimeVersions,
}

pub struct PromptEngine {
    theme: PromptTheme,
    registry: SegmentRegistry,
    cache: Option<CacheEntry>,
}

impl Default for PromptEngine {
    fn default() -> Self {
        Self::new("classic")
    }
}

impl PromptEngine {
    pub fn new(theme_name: &str) -> Self {
        let theme = ThemeLoader::load(theme_name, None);
        Self {
            theme,
            registry: SegmentRegistry::with_builtins(),
            cache: None,
        }
    }

    pub fn reload_theme(&mut self, theme_name: &str) {
        let file = DoshPaths::detect().ok().map(|p| p.theme_file());
        self.theme = ThemeLoader::load(theme_name, file.as_deref());
    }

    pub fn set_theme(&mut self, theme: PromptTheme) {
        self.theme = theme;
    }

    pub fn theme_name(&self) -> &str {
        &self.theme.name
    }

    pub fn segment_names(&self) -> Vec<String> {
        self.registry.names()
    }

    pub fn register_segment(&mut self, segment: Box<dyn crate::PromptSegment>) {
        self.registry.register(segment);
    }

    pub fn collect_context(
        &mut self,
        cwd: &std::path::Path,
        last_exit_code: i32,
        last_duration_ms: u128,
    ) -> PromptContext {
        let mut ctx = PromptContext::from_env(cwd, last_exit_code, last_duration_ms);
        let refresh = self.cache.as_ref().map(|c| c.cwd != cwd).unwrap_or(true);
        if refresh {
            let project = detect_project(cwd);
            let git = detect_git(cwd);
            let runtimes = detect_runtime_versions(&project);
            self.cache = Some(CacheEntry {
                cwd: cwd.to_path_buf(),
                git: git.clone(),
                project: project.clone(),
                runtimes: runtimes.clone(),
            });
        }
        if let Some(c) = &self.cache {
            ctx.git = c.git.clone();
            ctx.project = c.project.clone();
            ctx.runtimes = c.runtimes.clone();
        }
        ctx
    }

    pub fn render(&self, ctx: &PromptContext, term_width: usize) -> PromptRenderResult {
        let left = self.render_side(&self.theme.left, ctx);
        let right = if self.theme.right_prompt {
            self.render_side(&self.theme.right, ctx)
        } else {
            String::new()
        };
        let continuation = self.theme.continuation.clone();
        let right = if right.is_empty() {
            right
        } else {
            align_right(&left, &right, term_width)
        };
        PromptRenderResult {
            left: if self.theme.multiline {
                format!("{left}\n{continuation}")
            } else {
                format!("{left}{continuation}")
            },
            right,
            continuation,
        }
    }

    fn render_side(&self, segments: &[crate::theme::SegmentConfig], ctx: &PromptContext) -> String {
        let mut rendered: Vec<(String, String, String)> = Vec::new();
        for seg in segments {
            if !seg.enabled {
                continue;
            }
            if let Some(mut text) = self.registry.render(&seg.segment, ctx) {
                if !seg.icon.is_empty() {
                    text = format!("{} {}", seg.icon, text);
                }
                rendered.push((text, seg.fg.clone(), seg.bg.clone()));
            }
        }
        if rendered.is_empty() {
            return String::new();
        }
        let is_powerline =
            self.theme.separator == "\u{e0b0}" || self.theme.separator == "powerline";
        if !is_powerline {
            let mut out = Vec::new();
            for (text, fg, bg) in rendered {
                out.push(apply_style(&format!(" {text} "), &fg, &bg));
            }
            return out.join(&self.theme.separator);
        }
        let sep = "\u{e0b0}";
        let mut out = String::new();
        for (idx, (text, fg, bg)) in rendered.iter().enumerate() {
            out.push_str(&apply_style(&format!(" {text} "), fg, bg));
            let next_bg = rendered
                .get(idx + 1)
                .map(|(_, _, b)| b.as_str())
                .unwrap_or("");
            let sep_style = Style::new()
                .fg(parse_color(bg).unwrap_or(Color::White))
                .on(parse_color(next_bg).unwrap_or(Color::Default));
            out.push_str(&sep_style.paint(sep).to_string());
        }
        out
    }
}

fn align_right(left: &str, right: &str, width: usize) -> String {
    let lw = UnicodeWidthStr::width(left);
    let rw = UnicodeWidthStr::width(right);
    if lw + rw + 1 >= width {
        return right.to_string();
    }
    format!("{}{}", " ".repeat(width - lw - rw), right)
}

fn apply_style(text: &str, fg: &str, bg: &str) -> String {
    let mut style = Style::new();
    if let Some(c) = parse_color(fg) {
        style = style.fg(c);
    }
    if let Some(c) = parse_color(bg) {
        style = style.on(c);
    }
    style.paint(text).to_string()
}

fn parse_color(input: &str) -> Option<Color> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#')
        && hex.len() == 6
    {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }
    match s.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "purple" | "magenta" => Some(Color::Purple),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "darkgray" | "dark_gray" => Some(Color::DarkGray),
        "lightred" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightpurple" | "light_purple" | "lightmagenta" | "light_magenta" => {
            Some(Color::LightPurple)
        }
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        "lightgray" | "light_gray" => Some(Color::LightGray),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_default_prompt() {
        let mut engine = PromptEngine::new("classic");
        let cwd = std::env::current_dir().unwrap();
        let ctx = engine.collect_context(&cwd, 0, 12);
        let rendered = engine.render(&ctx, 120);
        assert!(!rendered.left.is_empty());
    }
}
