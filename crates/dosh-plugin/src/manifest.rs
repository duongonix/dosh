use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::command::{CommandDataType, CommandMetadata, CommandSideEffect};
use crate::error::PluginError;
use crate::permission::{EnvPermission, FilesystemPermission, Permission, PermissionSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginSource {
    Native,
    Wasm,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginDependency {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub source: PluginSource,
    pub target: Option<String>,
    pub minimum_dosh_version: Option<String>,
    pub permissions: Vec<Permission>,
    pub permission_set: PermissionSet,
    pub command_names: Vec<String>,
    pub command_metadata: Vec<CommandMetadata>,
    pub entry: Option<String>,
    pub checksum: Option<String>,
    pub dependencies: Vec<PluginDependency>,
    pub api_version: Option<String>,
    pub min_shell_version: Option<String>,
    pub max_shell_version: Option<String>,
    pub signature: Option<String>,
    pub hot_reload: bool,
}

impl PluginManifest {
    pub fn from_toml_str(input: &str) -> Result<Self> {
        let value: toml::Value =
            toml::from_str(input).context("failed to parse plugin manifest TOML")?;
        let parsed = if value.get("plugin").is_some() {
            parse_product_manifest(&value)?
        } else {
            parse_legacy_manifest(&value)?
        };
        parsed.validate()?;
        Ok(parsed)
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || !is_valid_name(&self.name) {
            return Err(PluginError::InvalidManifest("invalid plugin name".into()).into());
        }
        if self.version.trim().is_empty() || !looks_like_semver(&self.version) {
            return Err(PluginError::InvalidManifest("invalid plugin version".into()).into());
        }
        if matches!(self.source, PluginSource::Wasm)
            && self.entry.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(PluginError::InvalidManifest("missing wasm entry".into()).into());
        }
        if self.command_names.iter().any(|c| c.trim().is_empty()) {
            return Err(PluginError::InvalidManifest("invalid command name".into()).into());
        }
        Ok(())
    }
}

fn parse_product_manifest(v: &toml::Value) -> Result<PluginManifest> {
    #[derive(Debug, Deserialize)]
    struct ProductRoot {
        plugin: ProductPlugin,
        #[serde(default)]
        commands: Vec<ProductCommand>,
        permissions: Option<toml::Value>,
        #[serde(default)]
        dependencies: Vec<PluginDependency>,
    }

    #[derive(Debug, Deserialize)]
    struct ProductPlugin {
        name: String,
        version: String,
        description: Option<String>,
        author: Option<String>,
        license: Option<String>,
        homepage: Option<String>,
        repository: Option<String>,
        entry: Option<String>,
        #[serde(rename = "type")]
        kind: String,
        target: Option<String>,
        minimum_dosh_version: Option<String>,
        checksum: Option<String>,
        signature: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ProductCommand {
        name: String,
        usage: String,
        description: String,
        input: String,
        output: String,
        #[serde(default)]
        examples: Vec<String>,
    }

    let root: ProductRoot = v.clone().try_into()?;
    let source = parse_source(&root.plugin.kind)?;
    let command_metadata = root
        .commands
        .into_iter()
        .map(|c| CommandMetadata {
            name: c.name.clone(),
            usage: c.usage,
            description: c.description,
            examples: c.examples,
            input: parse_data_type(&c.input),
            output: parse_data_type(&c.output),
            permissions_needed: Vec::new(),
            side_effects: vec![CommandSideEffect::None],
            streaming_support_future: true,
        })
        .collect::<Vec<_>>();
    let command_names = command_metadata
        .iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>();

    let permission_set = parse_permission_set(root.permissions.as_ref())?;

    Ok(PluginManifest {
        name: root.plugin.name,
        version: root.plugin.version,
        description: root.plugin.description,
        author: root.plugin.author,
        license: root.plugin.license,
        homepage: root.plugin.homepage,
        repository: root.plugin.repository,
        source,
        target: root.plugin.target,
        minimum_dosh_version: root.plugin.minimum_dosh_version,
        permissions: Vec::new(),
        permission_set,
        command_names,
        command_metadata,
        entry: root.plugin.entry,
        checksum: root.plugin.checksum,
        dependencies: root.dependencies,
        api_version: Some("v1".into()),
        min_shell_version: None,
        max_shell_version: None,
        signature: root.plugin.signature,
        hot_reload: true,
    })
}

fn parse_permission_set(v: Option<&toml::Value>) -> Result<PermissionSet> {
    let Some(v) = v else {
        return Ok(PermissionSet::default());
    };
    let Some(tbl) = v.as_table() else {
        return Err(PluginError::InvalidManifest("permissions must be a table".into()).into());
    };
    let filesystem = tbl
        .get("filesystem")
        .and_then(|x| x.as_str())
        .map(|s| match s.to_ascii_lowercase().as_str() {
            "none" => Ok(FilesystemPermission::None),
            "read" => Ok(FilesystemPermission::Read),
            "write" => Ok(FilesystemPermission::Write),
            "read-write" | "read_write" => Ok(FilesystemPermission::ReadWrite),
            "scoped" => Ok(FilesystemPermission::Scoped),
            _ => Err(PluginError::InvalidManifest(format!(
                "unsupported filesystem permission `{s}`"
            ))),
        })
        .transpose()?
        .unwrap_or_default();
    let env = tbl
        .get("env")
        .and_then(|x| x.as_str())
        .map(|s| match s.to_ascii_lowercase().as_str() {
            "none" => Ok(EnvPermission::None),
            "read" => Ok(EnvPermission::Read),
            "write" => Ok(EnvPermission::Write),
            _ => Err(PluginError::InvalidManifest(format!(
                "unsupported env permission `{s}`"
            ))),
        })
        .transpose()?
        .unwrap_or_default();
    let network = tbl
        .get("network")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let process = tbl
        .get("process")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let secret = tbl.get("secret").and_then(|x| x.as_bool()).unwrap_or(false);
    Ok(PermissionSet {
        filesystem,
        network,
        process,
        env,
        secret,
    })
}

fn parse_legacy_manifest(v: &toml::Value) -> Result<PluginManifest> {
    #[derive(Debug, Deserialize)]
    struct Legacy {
        name: String,
        version: String,
        source: PluginSource,
        permissions: Vec<Permission>,
        #[serde(default)]
        commands: Vec<String>,
        entry: Option<String>,
        description: Option<String>,
        api_version: Option<String>,
        min_shell_version: Option<String>,
        max_shell_version: Option<String>,
        signature: Option<String>,
        #[serde(default = "default_hot_reload")]
        hot_reload: bool,
    }
    let l: Legacy = v.clone().try_into()?;
    let command_metadata = l
        .commands
        .iter()
        .map(|name| CommandMetadata {
            name: name.clone(),
            usage: name.clone(),
            description: String::new(),
            examples: Vec::new(),
            input: CommandDataType::Any,
            output: CommandDataType::Any,
            permissions_needed: Vec::new(),
            side_effects: vec![CommandSideEffect::None],
            streaming_support_future: false,
        })
        .collect::<Vec<_>>();

    Ok(PluginManifest {
        name: l.name,
        version: l.version,
        description: l.description,
        author: None,
        license: None,
        homepage: None,
        repository: None,
        source: l.source,
        target: None,
        minimum_dosh_version: None,
        permissions: l.permissions,
        permission_set: PermissionSet::default(),
        command_names: l.commands,
        command_metadata,
        entry: l.entry,
        checksum: None,
        dependencies: Vec::new(),
        api_version: l.api_version,
        min_shell_version: l.min_shell_version,
        max_shell_version: l.max_shell_version,
        signature: l.signature,
        hot_reload: l.hot_reload,
    })
}

fn default_hot_reload() -> bool {
    true
}

fn parse_source(v: &str) -> Result<PluginSource> {
    match v.to_ascii_lowercase().as_str() {
        "wasm" => Ok(PluginSource::Wasm),
        "native" => Ok(PluginSource::Native),
        _ => Err(PluginError::InvalidManifest(format!("unsupported plugin type `{v}`")).into()),
    }
}

fn parse_data_type(v: &str) -> CommandDataType {
    match v.to_ascii_lowercase().as_str() {
        "text" => CommandDataType::Text,
        "structured" => CommandDataType::Structured,
        "table" => CommandDataType::Table,
        "binary" => CommandDataType::Binary,
        _ => CommandDataType::Any,
    }
}

fn looks_like_semver(v: &str) -> bool {
    let parts = v.split('.').collect::<Vec<_>>();
    parts.len() >= 2 && parts.iter().all(|p| p.parse::<u64>().is_ok())
}

fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_product_manifest() {
        let raw = r#"
[plugin]
name = "hello"
version = "0.1.0"
entry = "hello.wasm"
type = "wasm"

[[commands]]
name = "hello"
usage = "hello <name>"
description = "Print hello"
input = "any"
output = "text"

[permissions]
filesystem = "none"
network = false
process = false
env = "read"
secret = false
"#;
        let m = PluginManifest::from_toml_str(raw).unwrap();
        assert_eq!(m.name, "hello");
        assert_eq!(m.command_names, vec!["hello"]);
    }
}
