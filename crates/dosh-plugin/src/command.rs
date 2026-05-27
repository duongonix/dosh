use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandDataType {
    Any,
    Text,
    Structured,
    Table,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandSideEffect {
    None,
    ReadsFilesystem,
    WritesFilesystem,
    Network,
    ProcessSpawn,
    EnvRead,
    EnvWrite,
    SecretRead,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandMetadata {
    pub name: String,
    pub usage: String,
    pub description: String,
    #[serde(default)]
    pub examples: Vec<String>,
    pub input: CommandDataType,
    pub output: CommandDataType,
    #[serde(default)]
    pub permissions_needed: Vec<String>,
    #[serde(default)]
    pub side_effects: Vec<CommandSideEffect>,
    #[serde(default)]
    pub streaming_support_future: bool,
}
