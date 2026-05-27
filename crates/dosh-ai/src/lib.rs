use anyhow::Result;

pub trait AiAssistant {
    fn explain_command(&self, command: &str) -> Result<String>;
}

#[derive(Debug, Default)]
pub struct NoopAiAssistant;

impl AiAssistant for NoopAiAssistant {
    fn explain_command(&self, command: &str) -> Result<String> {
        Ok(format!("AI helper is disabled. Command: {command}"))
    }
}
