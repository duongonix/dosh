use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTheme {
    pub name: String,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default)]
    pub right_prompt: bool,
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default = "default_continuation")]
    pub continuation: String,
    #[serde(default)]
    pub left: Vec<SegmentConfig>,
    #[serde(default)]
    pub right: Vec<SegmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentConfig {
    pub segment: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub fg: String,
    #[serde(default)]
    pub bg: String,
    #[serde(default)]
    pub enabled: bool,
}

impl Default for PromptTheme {
    fn default() -> Self {
        Self::builtin("classic")
    }
}

impl PromptTheme {
    pub fn builtin(name: &str) -> Self {
        match name {
            "classic" => Self {
                name: "classic".into(),
                multiline: false,
                right_prompt: false,
                separator: "\u{e0b0}".into(),
                continuation: "❯ ".into(),
                left: vec![
                    SegmentConfig {
                        segment: "os".into(),
                        icon: "▣".into(),
                        fg: "#0B0F14".into(),
                        bg: "#D6E0E5".into(),
                        enabled: true,
                    },
                    SegmentConfig {
                        segment: "cwd".into(),
                        icon: "".into(),
                        fg: "#EAF3FF".into(),
                        bg: "#2E5FA7".into(),
                        enabled: true,
                    },
                ],
                right: vec![],
            },
            "minimal" => Self {
                name: "minimal".into(),
                multiline: false,
                right_prompt: false,
                separator: " ".into(),
                continuation: "❯ ".into(),
                left: vec![
                    SegmentConfig::named("cwd"),
                    SegmentConfig::named("git"),
                    SegmentConfig::named("status"),
                ],
                right: vec![],
            },
            "powerline" => Self {
                name: "powerline".into(),
                multiline: true,
                right_prompt: true,
                separator: "\u{e0b0}".into(),
                continuation: "╰─❯ ".into(),
                left: vec![
                    SegmentConfig::named("os"),
                    SegmentConfig::named("cwd"),
                    SegmentConfig::named("git"),
                ],
                right: vec![
                    SegmentConfig::named("duration"),
                    SegmentConfig::named("status"),
                    SegmentConfig::named("time"),
                ],
            },
            "modern" => Self {
                name: "modern".into(),
                multiline: false,
                right_prompt: true,
                separator: " | ".into(),
                continuation: "❯ ".into(),
                left: vec![
                    SegmentConfig::named("cwd"),
                    SegmentConfig::named("git"),
                    SegmentConfig::named("project"),
                ],
                right: vec![
                    SegmentConfig::named("runtime"),
                    SegmentConfig::named("duration"),
                ],
            },
            "compact" => Self {
                name: "compact".into(),
                multiline: false,
                right_prompt: false,
                separator: " ".into(),
                continuation: "❯ ".into(),
                left: vec![SegmentConfig::named("cwd"), SegmentConfig::named("git")],
                right: vec![],
            },
            _ => Self {
                name: "classic".into(),
                multiline: false,
                right_prompt: false,
                separator: "\u{e0b0}".into(),
                continuation: "❯ ".into(),
                left: vec![
                    SegmentConfig {
                        segment: "os".into(),
                        icon: "▣".into(),
                        fg: "#0B0F14".into(),
                        bg: "#D6E0E5".into(),
                        enabled: true,
                    },
                    SegmentConfig {
                        segment: "cwd".into(),
                        icon: "".into(),
                        fg: "#EAF3FF".into(),
                        bg: "#2E5FA7".into(),
                        enabled: true,
                    },
                ],
                right: vec![],
            },
        }
    }
}

impl SegmentConfig {
    pub fn named(name: &str) -> Self {
        Self {
            segment: name.to_string(),
            icon: String::new(),
            fg: String::new(),
            bg: String::new(),
            enabled: true,
        }
    }
}

fn default_separator() -> String {
    " ".to_string()
}

fn default_continuation() -> String {
    "❯ ".to_string()
}

#[derive(Debug, Default)]
pub struct ThemeLoader;

impl ThemeLoader {
    pub fn load(theme_name: &str, path: Option<&std::path::Path>) -> PromptTheme {
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        if let Some(p) = path {
            candidates.push(p.to_path_buf());
        }
        if let Ok(paths) = dosh_config::DoshPaths::detect() {
            candidates.push(paths.themes_dir().join(format!("{theme_name}.toml")));
            candidates.push(paths.theme_file());
        }
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let home_path = std::path::PathBuf::from(home);
            candidates.push(
                home_path
                    .join(".config")
                    .join("dosh")
                    .join("themes")
                    .join(format!("{theme_name}.toml")),
            );
            candidates.push(home_path.join(".config").join("dosh").join("theme.toml"));
        }
        for p in candidates {
            if p.exists()
                && let Ok(text) = std::fs::read_to_string(&p)
                && let Ok(theme) = toml::from_str::<PromptTheme>(&text)
            {
                return theme;
            }
        }
        PromptTheme::builtin(theme_name)
    }
}
