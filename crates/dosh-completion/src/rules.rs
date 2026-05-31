use crate::context::CompletionContext;
use crate::load_custom_command_names;
use crate::model::CompletionItem;
use dosh_config::DoshPaths;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleTarget {
    Any,
    Arg(usize),
    Flags,
    Option(String),
}

impl RuleTarget {
    pub fn matches(
        &self,
        position: usize,
        is_flag: bool,
        prev_flag: Option<&str>,
        current: &str,
    ) -> bool {
        match self {
            RuleTarget::Any => true,
            RuleTarget::Arg(n) => !is_flag && *n == position,
            RuleTarget::Flags => is_flag,
            RuleTarget::Option(flag) => {
                prev_flag == Some(flag.as_str()) || current == flag.as_str()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuleProvider {
    Static(Vec<CompletionItem>),
    Call { fn_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRule {
    pub pattern_words: Vec<String>,
    pub target: RuleTarget,
    provider: RuleProvider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportFn {
    StaticList(Vec<CompletionItem>),
    MatchArg0 {
        map: BTreeMap<String, Vec<CompletionItem>>,
        default: Vec<CompletionItem>,
    },
}

#[derive(Debug, Clone, Default)]
struct FileScriptModel {
    exports: BTreeMap<String, ExportFn>,
    rules: Vec<CompletionRule>,
}

#[derive(Debug, Default)]
pub struct CompletionRulesStore {
    files: Vec<FileScriptModel>,
    custom_commands: Vec<String>,
}

impl CompletionRulesStore {
    pub fn load() -> Self {
        let mut store = Self::default();
        store.custom_commands = load_custom_command_names();
        if let Ok(paths) = DoshPaths::detect() {
            load_models_from_dir(paths.commands_dir().as_path(), &mut store.files);
        }
        store
    }

    pub fn complete(&self, ctx: &CompletionContext) -> Option<Vec<CompletionItem>> {
        let prev_flag = ctx.previous.as_deref().filter(|p| p.starts_with('-'));
        let mut best_len = 0usize;
        let mut out: Option<Vec<CompletionItem>> = None;

        for file in &self.files {
            for rule in &file.rules {
                if !pattern_match(&rule.pattern_words, &ctx.words) {
                    continue;
                }
                if !rule
                    .target
                    .matches(ctx.position, ctx.is_flag, prev_flag, &ctx.current)
                {
                    continue;
                }
                let items = eval_provider(&rule.provider, &file.exports, ctx);
                if items.is_empty() {
                    continue;
                }
                if rule.pattern_words.len() >= best_len {
                    best_len = rule.pattern_words.len();
                    out = Some(items);
                }
            }
        }
        out
    }

    pub fn custom_commands(&self) -> &[String] {
        &self.custom_commands
    }
}

fn eval_provider(
    provider: &RuleProvider,
    exports: &BTreeMap<String, ExportFn>,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    match provider {
        RuleProvider::Static(items) => items.clone(),
        RuleProvider::Call { fn_name } => {
            let Some(fun) = exports.get(fn_name) else {
                return Vec::new();
            };
            match fun {
                ExportFn::StaticList(items) => items.clone(),
                ExportFn::MatchArg0 { map, default } => ctx
                    .args
                    .first()
                    .and_then(|k| map.get(k))
                    .cloned()
                    .unwrap_or_else(|| default.clone()),
            }
        }
    }
}

fn pattern_match(pattern_words: &[String], input_words: &[String]) -> bool {
    if pattern_words.is_empty() || input_words.is_empty() {
        return false;
    }
    if pattern_words.len() > input_words.len() {
        return false;
    }
    pattern_words
        .iter()
        .zip(input_words.iter())
        .all(|(a, b)| a == b)
}

fn load_models_from_dir(dir: &Path, out: &mut Vec<FileScriptModel>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("dosh") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            out.push(parse_file_model(&text));
        }
    }
}

fn parse_file_model(text: &str) -> FileScriptModel {
    let exports = parse_export_functions(text);
    let rules = parse_completion_rules(text);
    FileScriptModel { exports, rules }
}

fn parse_export_functions(text: &str) -> BTreeMap<String, ExportFn> {
    let mut out = BTreeMap::new();
    let lines = text.lines().collect::<Vec<_>>();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].trim();
        if !line.starts_with("export fn ") {
            i += 1;
            continue;
        }
        let Some((name, _params)) = parse_fn_sig(line) else {
            i += 1;
            continue;
        };
        let mut body = String::new();
        let mut depth = line.chars().filter(|c| *c == '{').count() as i32
            - line.chars().filter(|c| *c == '}').count() as i32;
        i += 1;
        while i < lines.len() {
            let l = lines[i];
            depth += l.chars().filter(|c| *c == '{').count() as i32;
            depth -= l.chars().filter(|c| *c == '}').count() as i32;
            if depth <= 0 {
                break;
            }
            body.push_str(l);
            body.push('\n');
            i += 1;
        }
        if let Some(fun) = parse_export_body(&body) {
            out.insert(name, fun);
        }
        i += 1;
    }
    out
}

fn parse_fn_sig(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("export fn ")?.trim();
    let open = rest.find('(')?;
    let close = rest[open + 1..].find(')')? + open + 1;
    let name = rest[..open].trim().to_string();
    let params = rest[open + 1..close].trim().to_string();
    Some((name, params))
}

fn parse_export_body(body: &str) -> Option<ExportFn> {
    let trimmed = body.trim();
    if let Some(match_idx) = trimmed.find("match $ctx.args.0") {
        let after = &trimmed[match_idx..];
        return parse_match_arg0(after);
    }
    if let Some(items) = parse_inline_list(trimmed) {
        return Some(ExportFn::StaticList(items));
    }
    None
}

fn parse_match_arg0(text: &str) -> Option<ExportFn> {
    let open = text.find('{')?;
    let close = text.rfind('}')?;
    if close <= open {
        return None;
    }
    let inner = &text[open + 1..close];
    let mut map = BTreeMap::new();
    let mut default = Vec::new();
    for (key, list_text) in extract_match_arms(inner) {
        let items = parse_inline_list(&list_text).unwrap_or_default();
        if key == "_" {
            default = items;
        } else {
            map.insert(key, items);
        }
    }
    Some(ExportFn::MatchArg0 { map, default })
}

fn extract_match_arms(inner: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars = inner.chars().collect::<Vec<_>>();
    let mut i = 0usize;

    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let key = if chars[i] == '"' {
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let k = chars[start..i].iter().collect::<String>();
            if i < chars.len() {
                i += 1;
            }
            k
        } else if chars[i] == '_' {
            i += 1;
            "_".to_string()
        } else {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        };

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i + 1 >= chars.len() || chars[i] != '=' || chars[i + 1] != '>' {
            continue;
        }
        i += 2;

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '[' {
            continue;
        }

        let start = i;
        let mut depth = 0i32;
        let mut in_quote: Option<char> = None;
        let mut escaped = false;
        while i < chars.len() {
            let ch = chars[i];
            if let Some(q) = in_quote {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == q {
                    in_quote = None;
                }
                i += 1;
                continue;
            }
            match ch {
                '"' | '\'' => in_quote = Some(ch),
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        let list_text = chars[start..i].iter().collect::<String>();
        out.push((key, list_text));
    }

    out
}

fn parse_completion_rules(text: &str) -> Vec<CompletionRule> {
    let mut out = Vec::new();
    let lines = text.lines().collect::<Vec<_>>();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i].trim();
        if !line.starts_with("complete ") {
            i += 1;
            continue;
        }
        let Some((header, open_idx)) = split_header_body_start(line) else {
            i += 1;
            continue;
        };
        let opener = line.chars().nth(open_idx).unwrap_or(' ');
        let closer = if opener == '[' { ']' } else { '}' };
        let mut body = String::new();

        if let Some(rest) = line[open_idx + 1..].split(closer).next()
            && line[open_idx + 1..].contains(closer)
        {
            body = rest.to_string();
        } else {
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                if let Some(pos) = l.find(closer) {
                    body.push_str(&l[..pos]);
                    break;
                }
                body.push_str(l);
                body.push('\n');
                i += 1;
            }
        }

        if let Some(rule) = parse_rule(header, &body, opener) {
            out.push(rule);
        }
        i += 1;
    }
    out
}

fn split_header_body_start(line: &str) -> Option<(&str, usize)> {
    let b = line.find('[');
    let c = line.find('{');
    match (b, c) {
        (Some(x), Some(y)) => Some(if x < y {
            (&line[..x], x)
        } else {
            (&line[..y], y)
        }),
        (Some(x), None) => Some((&line[..x], x)),
        (None, Some(y)) => Some((&line[..y], y)),
        _ => None,
    }
}

fn parse_rule(header: &str, body: &str, opener: char) -> Option<CompletionRule> {
    let tokens = shell_words::split(header).ok()?;
    if tokens.len() < 2 || tokens[0] != "complete" {
        return None;
    }
    let pattern_words = shell_words::split(&tokens[1]).ok()?;
    let target = if tokens.len() >= 4 && tokens[2] == "arg" {
        RuleTarget::Arg(tokens[3].parse::<usize>().ok()?)
    } else if tokens.len() >= 3 && tokens[2] == "flags" {
        RuleTarget::Flags
    } else if tokens.len() >= 4 && tokens[2] == "option" {
        RuleTarget::Option(tokens[3].clone())
    } else {
        RuleTarget::Any
    };

    let provider = if opener == '[' {
        RuleProvider::Static(parse_inline_list(body)?)
    } else {
        let call = body.trim();
        let fn_name = call.split_whitespace().next()?.to_string();
        RuleProvider::Call { fn_name }
    };

    Some(CompletionRule {
        pattern_words,
        target,
        provider,
    })
}

fn parse_inline_list(text: &str) -> Option<Vec<CompletionItem>> {
    let src = text.trim();
    let inner = if src.starts_with('[') && src.ends_with(']') {
        &src[1..src.len() - 1]
    } else {
        src
    };
    let mut set = BTreeSet::new();
    let mut out = Vec::new();
    for part in split_list_items(inner) {
        let line = part.trim().trim_end_matches(',').trim();
        if let Some(v) = quoted_value(line)
            && set.insert(v.clone())
        {
            out.push(CompletionItem::new(v, None));
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn split_list_items(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    for ch in inner.chars() {
        if let Some(q) = in_quote {
            buf.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == q {
                in_quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                in_quote = Some(ch);
                buf.push(ch);
            }
            ',' | '\n' | '\r' => {
                let t = buf.trim();
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
                buf.clear();
            }
            _ => buf.push(ch),
        }
    }
    let t = buf.trim();
    if !t.is_empty() {
        parts.push(t.to_string());
    }
    parts
}

fn quoted_value(line: &str) -> Option<String> {
    if line.len() >= 2 && line.starts_with('"') && line.ends_with('"') {
        return Some(line[1..line.len() - 1].to_string());
    }
    if line.len() >= 2 && line.starts_with('\'') && line.ends_with('\'') {
        return Some(line[1..line.len() - 1].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_static_list_rule() {
        let src = r#"
complete "theme use" [
  "nord"
  "dracula"
]
"#;
        let rules = parse_completion_rules(src);
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn parse_inline_csv_list_items() {
        let items = parse_inline_list(r#"["Toyota", "Honda", "BMW"]"#).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].value, "Toyota");
        assert_eq!(items[1].value, "Honda");
        assert_eq!(items[2].value, "BMW");
    }

    #[test]
    fn parse_dynamic_call_rule() {
        let src = r#"
complete "search-car" arg 1 {
  brands
}
"#;
        let rules = parse_completion_rules(src);
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn parse_export_match_ctx_arg0() {
        let src = r#"
export fn models($ctx) {
  match $ctx.args.0 {
    "Toyota" => ["Camry", "Corolla"]
    _ => []
  }
}
"#;
        let m = parse_export_functions(src);
        assert!(m.contains_key("models"));
    }

    #[test]
    fn parse_export_match_ctx_inline_arms() {
        let src = r#"
export fn models($ctx) {
  match $ctx.args.0 {
    "Toyota" => ["Camry", "Corolla"]
    "Honda" => ["Civic", "CR-V"]
    "BMW" => ["X3", "X5"]
    _ => []
  }
}
"#;
        let m = parse_export_functions(src);
        let fun = m.get("models").expect("models");
        if let ExportFn::MatchArg0 { map, .. } = fun {
            let bmw = map.get("BMW").expect("BMW arm");
            assert_eq!(bmw.len(), 2);
            assert_eq!(bmw[0].value, "X3");
            assert_eq!(bmw[1].value, "X5");
        } else {
            panic!("expected match function");
        }
    }
}
