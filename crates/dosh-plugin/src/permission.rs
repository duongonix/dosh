use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::error::PluginError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
    ReadConfig,
    ReadTheme,
    ReadFonts,
    ReadPluginData,
    WritePluginData,
    NetworkAccess,
    SpawnProcess,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemPermission {
    #[default]
    #[serde(alias = "None")]
    None,
    #[serde(alias = "Read")]
    Read,
    #[serde(alias = "Write")]
    Write,
    #[serde(alias = "ReadWrite", alias = "read-write")]
    ReadWrite,
    #[serde(alias = "Scoped")]
    Scoped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EnvPermission {
    #[default]
    #[serde(alias = "None")]
    None,
    #[serde(alias = "Read")]
    Read,
    #[serde(alias = "Write")]
    Write,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PermissionSet {
    #[serde(default)]
    pub filesystem: FilesystemPermission,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub process: bool,
    #[serde(default)]
    pub env: EnvPermission,
    #[serde(default)]
    pub secret: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PermissionError {
    #[error("permission denied: {0}")]
    Denied(String),
}

#[derive(Debug, Clone, Default)]
pub struct PermissionPolicy {
    allowed_legacy: HashSet<Permission>,
    allowed_set: PermissionSet,
}

impl PermissionPolicy {
    pub fn allow_all() -> Self {
        Self {
            allowed_legacy: [
                Permission::ReadConfig,
                Permission::ReadTheme,
                Permission::ReadFonts,
                Permission::ReadPluginData,
                Permission::WritePluginData,
                Permission::NetworkAccess,
                Permission::SpawnProcess,
            ]
            .into_iter()
            .collect(),
            allowed_set: PermissionSet {
                filesystem: FilesystemPermission::ReadWrite,
                network: true,
                process: true,
                env: EnvPermission::Write,
                secret: true,
            },
        }
    }

    pub fn from_allowed(allowed: impl IntoIterator<Item = Permission>) -> Self {
        Self {
            allowed_legacy: allowed.into_iter().collect(),
            allowed_set: PermissionSet::default(),
        }
    }

    pub fn is_allowed(&self, permission: &Permission) -> bool {
        self.allowed_legacy.contains(permission)
    }

    pub fn enforce_all(&self, requested: &[Permission]) -> anyhow::Result<()> {
        for permission in requested {
            if !self.is_allowed(permission) {
                return Err(PluginError::PermissionDenied(format!("{permission:?}")).into());
            }
        }
        Ok(())
    }

    pub fn with_allowed_set(mut self, allowed_set: PermissionSet) -> Self {
        self.allowed_set = allowed_set;
        self
    }

    pub fn decide(&self, requested: &PermissionSet) -> PermissionDecision {
        if !fs_allowed(&self.allowed_set.filesystem, &requested.filesystem) {
            return PermissionDecision::Deny;
        }
        if requested.network && !self.allowed_set.network {
            return PermissionDecision::Deny;
        }
        if requested.process && !self.allowed_set.process {
            return PermissionDecision::Deny;
        }
        if !env_allowed(&self.allowed_set.env, &requested.env) {
            return PermissionDecision::Deny;
        }
        if requested.secret && !self.allowed_set.secret {
            return PermissionDecision::Deny;
        }
        PermissionDecision::Allow
    }

    pub fn enforce_set(&self, requested: &PermissionSet) -> Result<(), PermissionError> {
        if matches!(self.decide(requested), PermissionDecision::Allow) {
            Ok(())
        } else {
            Err(PermissionError::Denied(format!("{requested:?}")))
        }
    }
}

fn fs_allowed(allowed: &FilesystemPermission, requested: &FilesystemPermission) -> bool {
    use FilesystemPermission as F;
    matches!(
        (allowed, requested),
        (_, F::None)
            | (F::Read, F::Read)
            | (F::Write, F::Write)
            | (F::ReadWrite, F::Read)
            | (F::ReadWrite, F::Write)
            | (F::ReadWrite, F::ReadWrite)
            | (F::Scoped, F::Read)
            | (F::Scoped, F::Write)
            | (F::Scoped, F::Scoped)
    )
}

fn env_allowed(allowed: &EnvPermission, requested: &EnvPermission) -> bool {
    use EnvPermission as E;
    matches!(
        (allowed, requested),
        (_, E::None) | (E::Read, E::Read) | (E::Write, E::Read) | (E::Write, E::Write)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_unknown_permission() {
        let policy = PermissionPolicy::from_allowed([Permission::ReadConfig]);
        assert!(!policy.is_allowed(&Permission::NetworkAccess));
    }

    #[test]
    fn denies_permission_set_by_default() {
        let policy = PermissionPolicy::default();
        let requested = PermissionSet {
            network: true,
            ..PermissionSet::default()
        };
        assert!(policy.enforce_set(&requested).is_err());
    }
}
