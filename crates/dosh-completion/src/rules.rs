use crate::context::CompletionContext;
use crate::load_custom_command_names;
use crate::model::CompletionItem;
use crate::script_provider::{ProviderScript, eval_provider_script};
use dosh_config::DoshPaths;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

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
    Call {
        fn_name: String,
        args: Vec<ProviderArg>,
    },
    Script(ProviderScript),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProviderArg {
    Ctx,
    CtxPath(Vec<String>),
    Literal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleOptions {
    pub cache_ttl: Option<Duration>,
    pub timeout: Duration,
    pub priority: i64,
    pub no_filter: bool,
}

impl Default for RuleOptions {
    fn default() -> Self {
        Self {
            cache_ttl: None,
            timeout: Duration::from_millis(250),
            priority: 0,
            no_filter: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionRule {
    pub pattern_words: Vec<String>,
    pub target: RuleTarget,
    pub options: RuleOptions,
    provider: RuleProvider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportFn {
    StaticList(Vec<CompletionItem>),
    MatchArg0 {
        map: BTreeMap<String, ExportValue>,
        default: ExportValue,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportValue {
    Items(Vec<CompletionItem>),
    Script(String),
}

#[derive(Debug, Clone, Default)]
struct FileScriptModel {
    exports: BTreeMap<String, ExportFn>,
    rules: Vec<CompletionRule>,
}

#[derive(Debug, Clone)]
struct CachedFileModel {
    mtime: Option<SystemTime>,
    model: FileScriptModel,
}

#[derive(Debug, Clone)]
struct CachedProviderResult {
    expires_at: Instant,
    items: Vec<CompletionItem>,
}

#[derive(Debug, Default)]
struct StoreState {
    files: HashMap<PathBuf, CachedFileModel>,
    provider_cache: HashMap<String, CachedProviderResult>,
    custom_commands: Vec<String>,
    last_scan: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct CompletionRulesStore {
    roots: Vec<PathBuf>,
    state: Mutex<StoreState>,
}

impl CompletionRulesStore {
    pub fn load() -> Self {
        let mut roots = Vec::new();
        if let Ok(paths) = DoshPaths::detect() {
            roots.push(paths.commands_dir());
            roots.push(paths.completions_dir());
            let plugins = paths.plugins_dir();
            if let Ok(rd) = fs::read_dir(&plugins) {
                for entry in rd.flatten() {
                    let p = entry.path().join("completions");
                    if p.exists() && p.is_dir() {
                        roots.push(p);
                    }
                }
            }
        }
        let store = Self {
            roots,
            state: Mutex::new(StoreState::default()),
        };
        store.reload();
        store
    }

    pub fn reload(&self) {
        let mut state = self.state.lock().expect("completion store lock");
        state.files.clear();
        state.provider_cache.clear();
        state.last_scan = None;
        drop(state);
        self.refresh_files();
    }

    pub fn complete(&self, ctx: &CompletionContext) -> Option<Vec<CompletionItem>> {
        self.refresh_files();
        let mut state = self.state.lock().expect("completion store lock");
        let prev_flag = ctx.previous.as_deref().filter(|p| p.starts_with('-'));
        let mut best_len = 0usize;
        let mut best_prio = i64::MIN;
        let mut out: Option<Vec<CompletionItem>> = None;

        let snapshot = state
            .files
            .values()
            .map(|f| (f.model.exports.clone(), f.model.rules.clone()))
            .collect::<Vec<_>>();
        for (exports, rules) in snapshot {
            for rule in &rules {
                if matches!(rule.target, RuleTarget::Arg(_))
                    && ctx.words.len() == 1
                    && ctx.current == ctx.command
                {
                    continue;
                }
                if !pattern_match(&rule.pattern_words, &ctx.words) {
                    continue;
                }
                if !rule
                    .target
                    .matches(ctx.position, ctx.is_flag, prev_flag, &ctx.current)
                {
                    continue;
                }
                let mut items = self.eval_provider_cached(&mut state, rule, &exports, ctx);
                if !rule.options.no_filter {
                    items.retain(|i| completion_value_matches(&i.value, &ctx.current));
                }
                if items.is_empty() {
                    continue;
                }
                for item in &mut items {
                    if item.priority.is_none() && rule.options.priority != 0 {
                        item.priority = Some(rule.options.priority);
                    }
                }
                if rule.pattern_words.len() > best_len
                    || (rule.pattern_words.len() == best_len && rule.options.priority >= best_prio)
                {
                    best_len = rule.pattern_words.len();
                    best_prio = rule.options.priority;
                    out = Some(items);
                }
            }
        }
        out
    }

    pub fn custom_commands(&self) -> Vec<String> {
        let state = self.state.lock().expect("completion store lock");
        state.custom_commands.clone()
    }

    pub fn list_rules(&self) -> Vec<String> {
        self.refresh_files();
        let state = self.state.lock().expect("completion store lock");
        let mut out = Vec::new();
        for file in state.files.values() {
            for rule in &file.model.rules {
                out.push(format!(
                    "{} [{:?}]",
                    rule.pattern_words.join(" "),
                    rule.target
                ));
            }
        }
        out.sort();
        out
    }

    pub fn show_rules_for(&self, command: &str) -> Vec<String> {
        self.refresh_files();
        let state = self.state.lock().expect("completion store lock");
        let mut out = Vec::new();
        for file in state.files.values() {
            for rule in &file.model.rules {
                if rule
                    .pattern_words
                    .first()
                    .is_some_and(|w| w.eq_ignore_ascii_case(command))
                {
                    out.push(format!(
                        "pattern=`{}` target={:?} cache={:?} timeout={}ms priority={} no_filter={}",
                        rule.pattern_words.join(" "),
                        rule.target,
                        rule.options.cache_ttl,
                        rule.options.timeout.as_millis(),
                        rule.options.priority,
                        rule.options.no_filter
                    ));
                }
            }
        }
        out
    }
}

impl CompletionRulesStore {
    fn eval_provider_cached(
        &self,
        state: &mut StoreState,
        rule: &CompletionRule,
        provider_exports: &BTreeMap<String, ExportFn>,
        ctx: &CompletionContext,
    ) -> Vec<CompletionItem> {
        let cache_key = if rule.options.cache_ttl.is_some() {
            Some(format!(
                "{}::{:?}::{:?}::{}::{}",
                ctx.command, rule.pattern_words, rule.target, ctx.current, ctx.position
            ))
        } else {
            None
        };

        if let Some(key) = cache_key.as_ref()
            && let Some(cached) = state.provider_cache.get(key)
            && cached.expires_at > Instant::now()
        {
            return cached.items.clone();
        }

        let timeout = rule.options.timeout;
        let provider = rule.provider.clone();
        let exports = provider_exports.clone();
        let ctx_owned = ctx.clone();

        let items = eval_provider_with_timeout(provider, exports, ctx_owned, timeout);

        if let (Some(key), Some(ttl)) = (cache_key, rule.options.cache_ttl)
            && !items.is_empty()
        {
            state.provider_cache.insert(
                key,
                CachedProviderResult {
                    expires_at: Instant::now() + ttl,
                    items: items.clone(),
                },
            );
        }
        items
    }
}

fn eval_provider_with_timeout(
    provider: RuleProvider,
    exports: BTreeMap<String, ExportFn>,
    ctx: CompletionContext,
    timeout: Duration,
) -> Vec<CompletionItem> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| eval_provider_inner(&provider, &exports, &ctx));
        let _ = tx.send(result.unwrap_or_default());
    });
    rx.recv_timeout(timeout).unwrap_or_default()
}

fn eval_provider_inner(
    provider: &RuleProvider,
    exports: &BTreeMap<String, ExportFn>,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    match provider {
        RuleProvider::Static(items) => items.clone(),
        RuleProvider::Call { fn_name, args } => {
            let Some(fun) = exports.get(fn_name) else {
                return Vec::new();
            };
            let arg0 = args.first().and_then(|a| resolve_provider_arg(a, ctx));
            match fun {
                ExportFn::StaticList(items) => items.clone(),
                ExportFn::MatchArg0 { map, default } => ctx
                    .args
                    .first()
                    .or(arg0.as_ref())
                    .and_then(|k| map.get(k))
                    .map(|v| eval_export_value(v, ctx))
                    .unwrap_or_else(|| eval_export_value(default, ctx)),
            }
        }
        RuleProvider::Script(script) => eval_provider_script(script, ctx),
    }
}

fn eval_export_value(v: &ExportValue, ctx: &CompletionContext) -> Vec<CompletionItem> {
    match v {
        ExportValue::Items(items) => items.clone(),
        ExportValue::Script(source) => eval_provider_script(
            &ProviderScript {
                source: source.clone(),
            },
            ctx,
        ),
    }
}

fn resolve_provider_arg(arg: &ProviderArg, ctx: &CompletionContext) -> Option<String> {
    match arg {
        ProviderArg::Literal(v) => Some(v.clone()),
        ProviderArg::Ctx => ctx
            .args
            .first()
            .cloned()
            .or_else(|| Some(ctx.current.clone())),
        ProviderArg::CtxPath(path) => resolve_ctx_path(ctx, path),
    }
}

fn resolve_ctx_path(ctx: &CompletionContext, path: &[String]) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let first = path[0].as_str();
    match first {
        "current" => Some(ctx.current.clone()),
        "command" => Some(ctx.command.clone()),
        "cwd" => Some(ctx.cwd.clone()),
        "line" => Some(ctx.line.clone()),
        "previous" => ctx.previous.clone(),
        "command_path" => ctx.command_path.clone(),
        "args" => {
            if path.len() >= 2 {
                if let Ok(idx) = path[1].parse::<usize>() {
                    return ctx.args.get(idx).cloned();
                }
            }
            None
        }
        _ => None,
    }
}

fn completion_value_matches(value: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let v = value.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    v.starts_with(&n) || fuzzy_contains(&v, &n)
}

fn fuzzy_contains(value: &str, needle: &str) -> bool {
    let mut it = value.chars();
    for ch in needle.chars() {
        if !it.any(|c| c == ch) {
            return false;
        }
    }
    true
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

fn load_models_from_dir(dir: &Path, out: &mut Vec<(PathBuf, FileScriptModel)>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("dosh") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            out.push((path, parse_file_model(&text)));
        }
    }
}

impl CompletionRulesStore {
    fn refresh_files(&self) {
        let mut state = self.state.lock().expect("completion store lock");
        let now = Instant::now();
        if let Some(last) = state.last_scan
            && now.saturating_duration_since(last) < Duration::from_millis(300)
        {
            return;
        }
        state.last_scan = Some(now);
        state.custom_commands = load_custom_command_names();

        for root in &self.roots {
            let mut models = Vec::new();
            load_models_from_dir(root, &mut models);
            for (path, model) in models {
                let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                let changed = match state.files.get(&path) {
                    Some(cached) => cached.mtime != mtime,
                    None => true,
                };
                if changed {
                    state.files.insert(path, CachedFileModel { mtime, model });
                }
            }
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
    let mut default = ExportValue::Items(Vec::new());
    for (key, rhs_text) in extract_match_arms(inner) {
        let val = parse_export_value(&rhs_text).unwrap_or(ExportValue::Items(Vec::new()));
        if key == "_" {
            default = val;
        } else {
            map.insert(key, val);
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
        if i >= chars.len() {
            continue;
        }
        let opener = chars[i];
        if opener != '[' && opener != '{' {
            continue;
        }
        let closer = if opener == '[' { ']' } else { '}' };
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
                _ if ch == opener => depth += 1,
                _ if ch == closer => {
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

        let rhs = chars[start..i].iter().collect::<String>();
        out.push((key, rhs));
    }

    out
}

fn parse_export_value(raw: &str) -> Option<ExportValue> {
    let t = raw.trim();
    if t.starts_with('[') && t.ends_with(']') {
        return parse_inline_list(t).map(ExportValue::Items);
    }
    if t.starts_with('{') && t.ends_with('}') && t.len() >= 2 {
        let inner = t[1..t.len() - 1].trim().to_string();
        return Some(ExportValue::Script(inner));
    }
    None
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
        let (body, end_idx) = collect_balanced_body(&lines, i, open_idx, opener, closer);
        i = end_idx;

        if let Some(rule) = parse_rule(header, &body, opener) {
            out.push(rule);
        }
        i += 1;
    }
    out
}

fn collect_balanced_body(
    lines: &[&str],
    start_line: usize,
    open_idx: usize,
    opener: char,
    closer: char,
) -> (String, usize) {
    let mut body = String::new();
    let mut depth = 0i32;
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut started = false;
    let mut line_idx = start_line;

    while line_idx < lines.len() {
        let src = lines[line_idx];
        let part = if line_idx == start_line {
            &src[open_idx..]
        } else {
            src
        };

        for ch in part.chars() {
            if let Some(q) = in_quote {
                if started {
                    body.push(ch);
                }
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
                    if started {
                        body.push(ch);
                    }
                }
                _ if ch == opener => {
                    depth += 1;
                    if started {
                        body.push(ch);
                    } else {
                        started = true;
                    }
                }
                _ if ch == closer => {
                    depth -= 1;
                    if depth == 0 {
                        return (body, line_idx);
                    }
                    if started {
                        body.push(ch);
                    }
                }
                _ => {
                    if started {
                        body.push(ch);
                    }
                }
            }
        }

        if started {
            body.push('\n');
        }
        line_idx += 1;
    }

    (body, line_idx.saturating_sub(1))
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
    let options = parse_rule_options(&tokens);

    let provider = if opener == '[' {
        RuleProvider::Static(parse_inline_list(body)?)
    } else {
        parse_block_provider(body)?
    };

    Some(CompletionRule {
        pattern_words,
        target,
        options,
        provider,
    })
}

fn parse_block_provider(body: &str) -> Option<RuleProvider> {
    let call = body.trim();
    if call.starts_with('[') && call.ends_with(']') {
        return Some(RuleProvider::Static(parse_inline_list(call)?));
    }
    if call.contains('|')
        || call.contains('\n')
        || call.contains(" if ")
        || call.contains(" match ")
        || call.contains("where ")
    {
        return Some(RuleProvider::Script(ProviderScript {
            source: call.to_string(),
        }));
    }
    parse_call_provider(call)
}

fn parse_call_provider(body: &str) -> Option<RuleProvider> {
    let call = body.trim();
    let tokens = shell_words::split(call).ok()?;
    let fn_name = tokens.first()?.clone();
    let mut args = Vec::new();
    for tok in tokens.iter().skip(1) {
        if tok == "$ctx" {
            args.push(ProviderArg::Ctx);
            continue;
        }
        if let Some(rest) = tok.strip_prefix("$ctx.") {
            args.push(ProviderArg::CtxPath(
                rest.split('.')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string())
                    .collect(),
            ));
            continue;
        }
        args.push(ProviderArg::Literal(tok.clone()));
    }
    Some(RuleProvider::Call { fn_name, args })
}

fn parse_rule_options(tokens: &[String]) -> RuleOptions {
    let mut opts = RuleOptions::default();
    let mut i = 0usize;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "cache" => {
                if let Some(next) = tokens.get(i + 1)
                    && let Some(d) = parse_duration_token(next)
                {
                    opts.cache_ttl = Some(d);
                    i += 1;
                }
            }
            "timeout" => {
                if let Some(next) = tokens.get(i + 1)
                    && let Some(d) = parse_duration_token(next)
                {
                    opts.timeout = d;
                    i += 1;
                }
            }
            "priority" => {
                if let Some(next) = tokens.get(i + 1)
                    && let Ok(v) = next.parse::<i64>()
                {
                    opts.priority = v;
                    i += 1;
                }
            }
            "no-filter" => opts.no_filter = true,
            _ => {}
        }
        i += 1;
    }
    opts
}

fn parse_duration_token(raw: &str) -> Option<Duration> {
    let s = raw.trim().to_ascii_lowercase();
    for (unit, mul) in [
        ("ms", 1u64),
        ("msec", 1),
        ("sec", 1_000),
        ("s", 1_000),
        ("min", 60_000),
    ] {
        if let Some(num) = s.strip_suffix(unit)
            && let Ok(v) = num.trim().parse::<u64>()
        {
            return Some(Duration::from_millis(v.saturating_mul(mul)));
        }
    }
    None
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
        if let Some(item) = parse_list_item(line)
            && set.insert(item.value.clone())
        {
            out.push(item);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn parse_list_item(line: &str) -> Option<CompletionItem> {
    if let Some(v) = quoted_value(line) {
        return Some(CompletionItem::new(v, None));
    }
    parse_record_item(line)
}

fn split_list_items(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut brace_depth = 0i32;
    let mut bracket_depth = 0i32;
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
            '{' => {
                brace_depth += 1;
                buf.push(ch);
            }
            '}' => {
                brace_depth -= 1;
                buf.push(ch);
            }
            '[' => {
                bracket_depth += 1;
                buf.push(ch);
            }
            ']' => {
                bracket_depth -= 1;
                buf.push(ch);
            }
            ',' | '\n' | '\r' => {
                if brace_depth == 0 && bracket_depth == 0 {
                    let t = buf.trim();
                    if !t.is_empty() {
                        parts.push(t.to_string());
                    }
                    buf.clear();
                } else {
                    buf.push(ch);
                }
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

fn parse_record_item(line: &str) -> Option<CompletionItem> {
    let t = line.trim();
    if !t.starts_with('{') || !t.ends_with('}') {
        return None;
    }
    let mut item = CompletionItem::new(String::new(), None);
    let inner = &t[1..t.len().saturating_sub(1)];
    for field in split_list_items(inner) {
        let mut it = field.splitn(2, ':');
        let key = it.next()?.trim().trim_matches('"').trim_matches('\'');
        let raw = it.next()?.trim();
        let value = quoted_value(raw).unwrap_or_else(|| raw.to_string());
        match key {
            "value" => item.value = value,
            "description" => item.description = Some(value),
            "kind" => item.kind = Some(value),
            "icon" => item.icon = Some(value),
            "insert" | "insert_text" => item.insert_text = Some(value),
            "priority" => item.priority = value.parse::<i64>().ok(),
            _ => {}
        }
    }
    if item.value.is_empty() {
        None
    } else {
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
    fn parse_dynamic_call_rule_with_ctx_arg() {
        let src = r#"
complete "search-car" arg 2 {
  models $ctx.args.0
}
"#;
        let rules = parse_completion_rules(src);
        assert_eq!(rules.len(), 1);
        match &rules[0].provider {
            RuleProvider::Call { fn_name, args } => {
                assert_eq!(fn_name, "models");
                assert!(!args.is_empty());
            }
            _ => panic!("expected call provider"),
        }
    }

    #[test]
    fn parse_dynamic_script_rule() {
        let src = r#"
complete "theme use" {
  theme list | get name
}
"#;
        let rules = parse_completion_rules(src);
        assert_eq!(rules.len(), 1);
        match &rules[0].provider {
            RuleProvider::Script(script) => {
                assert!(script.source.contains("theme list"));
            }
            _ => panic!("expected script provider"),
        }
    }

    #[test]
    fn parse_multiline_list_block_as_static_provider() {
        let src = r#"
complete "devflow" arg 1 {
  [
    { value: "build", description: "Build" }
    { value: "test", description: "Test" }
  ]
}
"#;
        let rules = parse_completion_rules(src);
        assert_eq!(rules.len(), 1);
        match &rules[0].provider {
            RuleProvider::Static(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].value, "build");
            }
            _ => panic!("expected static provider"),
        }
    }

    #[test]
    fn parse_static_record_list_items() {
        let items = parse_inline_list(
            r#"[
{ value: "install", description: "Install plugin", kind: "subcommand", icon: "📦", priority: 10 }
{ value: "list", description: "List plugins" }
]"#,
        )
        .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].value, "install");
        assert_eq!(items[0].kind.as_deref(), Some("subcommand"));
        assert_eq!(items[1].description.as_deref(), Some("List plugins"));
    }

    #[test]
    fn parse_rule_options_cache_timeout_priority() {
        let src = r#"
complete "git checkout" arg 1 cache 5sec timeout 1sec priority 100 no-filter {
  branches
}
"#;
        let rules = parse_completion_rules(src);
        assert_eq!(rules.len(), 1);
        let opts = &rules[0].options;
        assert_eq!(opts.cache_ttl, Some(Duration::from_secs(5)));
        assert_eq!(opts.timeout, Duration::from_secs(1));
        assert_eq!(opts.priority, 100);
        assert!(opts.no_filter);
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
            if let ExportValue::Items(items) = bmw {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].value, "X3");
                assert_eq!(items[1].value, "X5");
            } else {
                panic!("expected items");
            }
        } else {
            panic!("expected match function");
        }
    }

    #[test]
    fn parse_export_match_ctx_block_arm_script() {
        let src = r#"
export fn models($ctx) {
  match $ctx.args.0 {
    "build" => { ls | where type == "dir" | select name | get name }
    _ => []
  }
}
"#;
        let m = parse_export_functions(src);
        let fun = m.get("models").expect("models");
        if let ExportFn::MatchArg0 { map, .. } = fun {
            let build = map.get("build").expect("build arm");
            if let ExportValue::Script(s) = build {
                assert!(s.contains("ls | where"));
            } else {
                panic!("expected script export value");
            }
        } else {
            panic!("expected match function");
        }
    }
}
