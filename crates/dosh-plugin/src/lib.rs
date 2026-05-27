pub mod api;
pub mod command;
pub mod compat;
pub mod discovery;
pub mod distribution;
pub mod error;
pub mod grants;
pub mod manager;
pub mod manifest;
pub mod marketplace;
pub mod permission;
pub mod registry;
pub mod storage;
pub mod trust;

pub use api::{Plugin, PluginCommand, PluginContext};
pub use command::{CommandDataType, CommandMetadata, CommandSideEffect};
pub use compat::ensure_compatible;
pub use discovery::{PluginPackage, discover_plugins};
pub use distribution::{
    init_plugin_scaffold, install_plugin, publish_plugin, sign_plugin_manifest,
    verify_plugin_signature,
};
pub use grants::PermissionGrants;
pub use manager::PluginManager;
pub use manifest::{PluginManifest, PluginSource};
pub use marketplace::{InstallPlan, LocalRegistry, RegistryClient, RegistryPackageMetadata};
pub use permission::{
    EnvPermission, FilesystemPermission, Permission, PermissionDecision, PermissionError,
    PermissionPolicy, PermissionSet,
};
pub use registry::PluginRegistry;
pub use storage::{PluginInstallSource, PluginStateEntry, PluginsLockfile, PluginsState};
pub use trust::{TrustPolicy, TrustStore, TrustedKeyRing};
