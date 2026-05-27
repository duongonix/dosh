use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct GitContext {
    pub in_repo: bool,
    pub repo_root: Option<PathBuf>,
    pub branch: Option<String>,
    pub detached: bool,
    pub dirty: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectContext {
    pub node: bool,
    pub rust: bool,
    pub python: bool,
    pub deno: bool,
    pub bun: bool,
    pub go: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeVersions {
    pub node: Option<String>,
    pub rust: Option<String>,
    pub python: Option<String>,
    pub deno: Option<String>,
    pub bun: Option<String>,
    pub go: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub cwd: PathBuf,
    pub cwd_display: String,
    pub home_dir: Option<PathBuf>,
    pub username: String,
    pub hostname: String,
    pub os: String,
    pub last_exit_code: i32,
    pub last_duration_ms: u128,
    pub git: GitContext,
    pub project: ProjectContext,
    pub runtimes: RuntimeVersions,
}

impl PromptContext {
    pub fn from_env(cwd: &Path, last_exit_code: i32, last_duration_ms: u128) -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .map(PathBuf::from);
        let cwd_display = render_cwd(cwd, home.as_deref());
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "user".to_string());
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "host".to_string());
        Self {
            cwd: cwd.to_path_buf(),
            cwd_display,
            home_dir: home,
            username,
            hostname,
            os: std::env::consts::OS.to_string(),
            last_exit_code,
            last_duration_ms,
            git: GitContext::default(),
            project: ProjectContext::default(),
            runtimes: RuntimeVersions::default(),
        }
    }
}

pub(crate) fn render_cwd(cwd: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home
        && let Ok(rel) = cwd.strip_prefix(home)
    {
        let suffix = rel.display().to_string();
        return if suffix.is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", suffix.replace('\\', "/"))
        };
    }
    cwd.display().to_string().replace('\\', "/")
}

pub(crate) fn detect_project(cwd: &Path) -> ProjectContext {
    let has = |name: &str| cwd.join(name).exists();
    ProjectContext {
        node: has("package.json"),
        rust: has("Cargo.toml"),
        python: has("pyproject.toml") || has("requirements.txt"),
        deno: has("deno.json"),
        bun: has("bun.lockb"),
        go: has("go.mod"),
    }
}

pub(crate) fn detect_git(cwd: &Path) -> GitContext {
    let mut cur = Some(cwd.to_path_buf());
    while let Some(dir) = cur {
        let git_dir = dir.join(".git");
        if git_dir.exists() {
            let mut ctx = GitContext {
                in_repo: true,
                repo_root: Some(dir.clone()),
                ..GitContext::default()
            };
            let head = git_dir.join("HEAD");
            if let Ok(head_text) = std::fs::read_to_string(head) {
                let t = head_text.trim();
                if let Some(rest) = t.strip_prefix("ref: refs/heads/") {
                    ctx.branch = Some(rest.to_string());
                } else {
                    ctx.detached = true;
                    ctx.branch = Some(t.chars().take(7).collect::<String>());
                }
            }
            ctx.dirty = repo_dirty_by_mtime(&dir, Duration::from_secs(2));
            return ctx;
        }
        cur = dir.parent().map(|p| p.to_path_buf());
    }
    GitContext::default()
}

fn repo_dirty_by_mtime(root: &Path, window: Duration) -> bool {
    let now = Instant::now();
    let cutoff = std::time::SystemTime::now() - window;
    let rd = match std::fs::read_dir(root) {
        Ok(v) => v,
        Err(_) => return false,
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.file_name().and_then(|s| s.to_str()) == Some(".git") {
            continue;
        }
        if let Ok(md) = std::fs::metadata(&p)
            && let Ok(m) = md.modified()
            && m > cutoff
        {
            return true;
        }
        if now.elapsed() > Duration::from_millis(20) {
            break;
        }
    }
    false
}

pub(crate) fn detect_runtime_versions(project: &ProjectContext) -> RuntimeVersions {
    RuntimeVersions {
        node: if project.node {
            cmd_version("node", &["-v"])
        } else {
            None
        },
        rust: if project.rust {
            cmd_version("rustc", &["--version"])
        } else {
            None
        },
        python: if project.python {
            cmd_version("python", &["--version"]).or_else(|| cmd_version("python3", &["--version"]))
        } else {
            None
        },
        deno: if project.deno {
            cmd_version("deno", &["--version"])
        } else {
            None
        },
        bun: if project.bun {
            cmd_version("bun", &["--version"])
        } else {
            None
        },
        go: if project.go {
            cmd_version("go", &["version"])
        } else {
            None
        },
    }
}

fn cmd_version(bin: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(bin).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}
