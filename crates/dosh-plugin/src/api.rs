use anyhow::Result;
use dosh_builtins::PipelineData;

use crate::command::CommandMetadata;

#[derive(Debug, Clone, Default)]
pub struct PluginContext {
    pub cwd: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginCommand {
    pub metadata: CommandMetadata,
}

pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn commands(&self) -> Vec<PluginCommand>;
    fn run(
        &self,
        command: &str,
        args: &[String],
        input: PipelineData,
        ctx: &PluginContext,
    ) -> Result<PipelineData>;
}
