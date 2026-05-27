#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRisk {
    Safe,
    NeedsConfirmation,
}

pub fn classify_command(input: &str) -> CommandRisk {
    if input.contains("rm ") || input.contains("Remove-Item") {
        CommandRisk::NeedsConfirmation
    } else {
        CommandRisk::Safe
    }
}
