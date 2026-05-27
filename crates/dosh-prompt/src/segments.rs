use crate::context::PromptContext;
use std::collections::BTreeMap;

pub trait PromptSegment: Send + Sync {
    fn name(&self) -> &'static str;
    fn render(&self, ctx: &PromptContext) -> Option<String>;
}

#[derive(Default)]
pub struct SegmentRegistry {
    segments: BTreeMap<String, Box<dyn PromptSegment>>,
}

impl SegmentRegistry {
    pub fn with_builtins() -> Self {
        let mut s = Self::default();
        s.register(Box::new(CwdSegment));
        s.register(Box::new(GitSegment));
        s.register(Box::new(StatusSegment));
        s.register(Box::new(TimeSegment));
        s.register(Box::new(DurationSegment));
        s.register(Box::new(OsSegment));
        s.register(Box::new(ProjectSegment));
        s.register(Box::new(RuntimeSegment));
        s
    }

    pub fn register(&mut self, segment: Box<dyn PromptSegment>) {
        self.segments.insert(segment.name().to_string(), segment);
    }

    pub fn render(&self, name: &str, ctx: &PromptContext) -> Option<String> {
        self.segments.get(name).and_then(|s| s.render(ctx))
    }

    pub fn names(&self) -> Vec<String> {
        self.segments.keys().cloned().collect()
    }
}

struct CwdSegment;
impl PromptSegment for CwdSegment {
    fn name(&self) -> &'static str {
        "cwd"
    }
    fn render(&self, ctx: &PromptContext) -> Option<String> {
        Some(ctx.cwd_display.clone())
    }
}

struct GitSegment;
impl PromptSegment for GitSegment {
    fn name(&self) -> &'static str {
        "git"
    }
    fn render(&self, ctx: &PromptContext) -> Option<String> {
        if !ctx.git.in_repo {
            return None;
        }
        let mut s = String::from("git:");
        s.push_str(ctx.git.branch.as_deref().unwrap_or("?"));
        if ctx.git.detached {
            s.push_str(" detached");
        }
        if ctx.git.dirty {
            s.push_str(" *");
        }
        Some(s)
    }
}

struct StatusSegment;
impl PromptSegment for StatusSegment {
    fn name(&self) -> &'static str {
        "status"
    }
    fn render(&self, ctx: &PromptContext) -> Option<String> {
        if ctx.last_exit_code == 0 {
            None
        } else {
            Some(format!("?{}", ctx.last_exit_code))
        }
    }
}

struct TimeSegment;
impl PromptSegment for TimeSegment {
    fn name(&self) -> &'static str {
        "time"
    }
    fn render(&self, _ctx: &PromptContext) -> Option<String> {
        let now = chrono::Local::now();
        Some(now.format("%H:%M:%S").to_string())
    }
}

struct DurationSegment;
impl PromptSegment for DurationSegment {
    fn name(&self) -> &'static str {
        "duration"
    }
    fn render(&self, ctx: &PromptContext) -> Option<String> {
        if ctx.last_duration_ms > 0 {
            Some(format!("{}ms", ctx.last_duration_ms))
        } else {
            None
        }
    }
}

struct OsSegment;
impl PromptSegment for OsSegment {
    fn name(&self) -> &'static str {
        "os"
    }
    fn render(&self, ctx: &PromptContext) -> Option<String> {
        Some(ctx.os.clone())
    }
}

struct ProjectSegment;
impl PromptSegment for ProjectSegment {
    fn name(&self) -> &'static str {
        "project"
    }
    fn render(&self, ctx: &PromptContext) -> Option<String> {
        let mut tags = Vec::new();
        if ctx.project.rust {
            tags.push("rust");
        }
        if ctx.project.node {
            tags.push("node");
        }
        if ctx.project.python {
            tags.push("python");
        }
        if ctx.project.go {
            tags.push("go");
        }
        if ctx.project.deno {
            tags.push("deno");
        }
        if ctx.project.bun {
            tags.push("bun");
        }
        if tags.is_empty() {
            None
        } else {
            Some(tags.join(","))
        }
    }
}

struct RuntimeSegment;
impl PromptSegment for RuntimeSegment {
    fn name(&self) -> &'static str {
        "runtime"
    }
    fn render(&self, ctx: &PromptContext) -> Option<String> {
        let mut out = Vec::new();
        if let Some(v) = &ctx.runtimes.node {
            out.push(format!("node@{}", short_version(v)));
        }
        if let Some(v) = &ctx.runtimes.rust {
            out.push(format!("rust@{}", short_version(v)));
        }
        if let Some(v) = &ctx.runtimes.python {
            out.push(format!("py@{}", short_version(v)));
        }
        if out.is_empty() {
            None
        } else {
            Some(out.join(" "))
        }
    }
}

fn short_version(v: &str) -> String {
    v.split_whitespace().last().unwrap_or(v).to_string()
}
