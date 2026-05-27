use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;

use crate::api::Plugin;
use crate::command::CommandMetadata;
use crate::manifest::PluginManifest;
use crate::permission::PermissionPolicy;

#[derive(Default)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, Arc<dyn Plugin>>,
    manifests: BTreeMap<String, PluginManifest>,
    command_to_plugin: BTreeMap<String, String>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        plugin: Arc<dyn Plugin>,
        manifest: PluginManifest,
        policy: &PermissionPolicy,
    ) -> Result<()> {
        manifest.validate()?;
        policy.enforce_all(&manifest.permissions)?;

        let name = manifest.name.clone();
        for cmd in &manifest.command_names {
            self.command_to_plugin.insert(cmd.clone(), name.clone());
        }
        self.plugins.insert(name.clone(), plugin);
        self.manifests.insert(name, manifest);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Plugin>> {
        self.plugins.get(name).cloned()
    }

    pub fn manifest(&self, name: &str) -> Option<&PluginManifest> {
        self.manifests.get(name)
    }

    pub fn list_names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn find_plugin_by_command(&self, command: &str) -> Option<Arc<dyn Plugin>> {
        let plugin_name = self.command_to_plugin.get(command)?;
        self.get(plugin_name)
    }

    pub fn command_metadata(&self, command: &str) -> Option<CommandMetadata> {
        let plugin_name = self.command_to_plugin.get(command)?;
        let manifest = self.manifest(plugin_name)?;
        manifest
            .command_metadata
            .iter()
            .find(|m| m.name == command)
            .cloned()
    }
}
