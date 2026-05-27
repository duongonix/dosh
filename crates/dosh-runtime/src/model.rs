use dosh_builtins::PipelineData;
use dosh_env::EnvContext;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitStatus {
    pub code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeContext {
    pub cwd: PathBuf,
}

impl RuntimeContext {
    pub fn from_env(env: &EnvContext) -> Self {
        Self {
            cwd: env.cwd().to_path_buf(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipelineStream {
    pub data: PipelineData,
}
