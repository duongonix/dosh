use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use dosh_plugin::{
    Permission, PermissionGrants, PermissionPolicy, PluginManifest, PluginPackage, PluginSource,
    TrustPolicy, discover_plugins, ensure_compatible,
};
use serde::{Deserialize, Serialize};
use wasmtime::{Caller, Engine, Extern, Linker, Memory, Module, Store};

const HOST_WIT_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunRequest {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResponse {
    pub exit_code: i32,
    pub output: Option<String>,
}

#[derive(Default)]
struct HostState {
    granted: HashSet<Permission>,
    output: Option<String>,
}

#[derive(Debug, Clone)]
struct LoadedPlugin {
    package: PluginPackage,
    modified_at: SystemTime,
}

pub struct WasmPluginRuntime {
    engine: Engine,
    policy: PermissionPolicy,
    trust_policy: TrustPolicy,
    shell_version: String,
    plugin_root: Option<PathBuf>,
    plugins: BTreeMap<String, LoadedPlugin>,
    command_map: HashMap<String, String>,
    permission_grants_path: Option<PathBuf>,
    permission_grants: PermissionGrants,
}

impl Default for WasmPluginRuntime {
    fn default() -> Self {
        Self::new(PermissionPolicy::allow_all())
    }
}

impl WasmPluginRuntime {
    pub fn new(policy: PermissionPolicy) -> Self {
        Self {
            engine: Engine::default(),
            policy,
            trust_policy: TrustPolicy::default(),
            shell_version: env!("CARGO_PKG_VERSION").to_string(),
            plugin_root: None,
            plugins: BTreeMap::new(),
            command_map: HashMap::new(),
            permission_grants_path: None,
            permission_grants: PermissionGrants::default(),
        }
    }

    pub fn with_plugin_root(mut self, plugin_root: PathBuf) -> Self {
        self.plugin_root = Some(plugin_root);
        self
    }

    pub fn with_trust_policy(mut self, trust_policy: TrustPolicy) -> Self {
        self.trust_policy = trust_policy;
        self
    }

    pub fn with_permission_grants_file(mut self, path: PathBuf) -> Self {
        self.permission_grants = PermissionGrants::from_toml_file(&path).unwrap_or_default();
        self.permission_grants_path = Some(path);
        self
    }

    pub fn validate_module_bytes(&self, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            anyhow::bail!("empty wasm module")
        }
        Module::validate(&self.engine, bytes)?;
        Ok(())
    }

    pub fn validate_manifest(&self, manifest: &PluginManifest) -> Result<()> {
        manifest.validate()?;
        if manifest.source != PluginSource::Wasm {
            anyhow::bail!("manifest source is not wasm")
        }
        self.policy.enforce_all(&manifest.permissions)?;
        ensure_compatible(manifest, &self.shell_version, HOST_WIT_VERSION)?;
        Ok(())
    }

    pub fn load_from_filesystem(&mut self) -> Result<()> {
        let Some(root) = self.plugin_root.clone() else {
            return Ok(());
        };
        let discovered = discover_plugins(&root)?;
        self.plugins.clear();
        self.command_map.clear();
        for package in discovered {
            if let Err(e) = self.load_package(package.clone()) {
                eprintln!("warning: skip plugin `{}`: {e}", package.manifest.name);
            }
        }
        Ok(())
    }

    pub fn reload_changed_plugins(&mut self) -> Result<()> {
        let Some(root) = self.plugin_root.clone() else {
            return Ok(());
        };
        let discovered = discover_plugins(&root)?;
        let mut by_name = BTreeMap::new();
        for package in discovered {
            by_name.insert(package.manifest.name.clone(), package);
        }

        let mut new_plugins = BTreeMap::new();
        self.command_map.clear();

        for (name, package) in by_name {
            let modified = file_modified_time(&package.entry_path)?;
            let keep = self
                .plugins
                .get(&name)
                .map(|p| p.modified_at == modified)
                .unwrap_or(false);
            if keep {
                let existing = self.plugins.get(&name).cloned().expect("exists");
                for cmd in &existing.package.manifest.command_names {
                    self.command_map.insert(cmd.clone(), name.clone());
                }
                new_plugins.insert(name, existing);
                continue;
            }
            if let Err(e) = self.load_package_into(package.clone(), modified, &mut new_plugins) {
                eprintln!(
                    "warning: skip plugin `{}` reload: {e}",
                    package.manifest.name
                );
            }
        }

        self.plugins = new_plugins;
        Ok(())
    }

    pub fn run_command(
        &mut self,
        command: &str,
        args: &[String],
        cwd: Option<String>,
    ) -> Result<Option<RunResponse>> {
        self.reload_changed_plugins()?;
        let Some(plugin_name) = self.command_map.get(command).cloned() else {
            return Ok(None);
        };
        let plugin = self
            .plugins
            .get(&plugin_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("plugin not loaded: {plugin_name}"))?;
        let res = self.execute_plugin(&plugin, command, args, cwd)?;
        Ok(Some(res))
    }

    fn load_package(&mut self, package: PluginPackage) -> Result<()> {
        let modified = file_modified_time(&package.entry_path)?;
        let mut target = BTreeMap::new();
        self.load_package_into(package, modified, &mut target)?;
        self.plugins.extend(target);
        Ok(())
    }

    fn load_package_into(
        &mut self,
        package: PluginPackage,
        modified: SystemTime,
        target: &mut BTreeMap<String, LoadedPlugin>,
    ) -> Result<()> {
        self.validate_manifest(&package.manifest)?;
        self.ensure_permissions_granted(&package.manifest)?;
        self.validate_module_bytes(&fs::read(&package.entry_path)?)?;
        self.trust_policy
            .verify_plugin_file(&package.manifest, &package.entry_path)?;
        for cmd in &package.manifest.command_names {
            self.command_map
                .insert(cmd.clone(), package.manifest.name.clone());
        }
        target.insert(
            package.manifest.name.clone(),
            LoadedPlugin {
                package,
                modified_at: modified,
            },
        );
        Ok(())
    }

    fn ensure_permissions_granted(&mut self, manifest: &PluginManifest) -> Result<()> {
        let missing = self
            .permission_grants
            .missing_permissions(&manifest.name, &manifest.permissions);
        if missing.is_empty() {
            return Ok(());
        }
        println!(
            "Plugin `{}` requests new permissions: {}",
            manifest.name,
            missing
                .iter()
                .map(|p| format!("{p:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        print!("Allow and remember? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let accepted = matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes");
        if !accepted {
            anyhow::bail!("permission denied for plugin `{}`", manifest.name);
        }

        self.permission_grants
            .grant_permissions(&manifest.name, &missing);
        if let Some(path) = &self.permission_grants_path {
            self.permission_grants.save_to_toml_file(path)?;
        }
        Ok(())
    }

    fn execute_plugin(
        &self,
        plugin: &LoadedPlugin,
        command: &str,
        args: &[String],
        cwd: Option<String>,
    ) -> Result<RunResponse> {
        let bytes = fs::read(&plugin.package.entry_path).with_context(|| {
            format!(
                "failed to read plugin module {}",
                plugin.package.entry_path.display()
            )
        })?;
        let module = Module::new(&self.engine, bytes)?;
        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        linker.func_wrap(
            "dosh_host",
            "emit",
            |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> i32 {
                match read_guest_string(&mut caller, ptr, len) {
                    Ok(text) => {
                        caller.data_mut().output = Some(text);
                        0
                    }
                    Err(_) => 1,
                }
            },
        )?;
        linker.func_wrap(
            "dosh_host",
            "has_permission",
            |caller: Caller<'_, HostState>, code: i32| -> i32 {
                let perm = decode_permission(code);
                if caller.data().granted.contains(&perm) {
                    1
                } else {
                    0
                }
            },
        )?;

        let granted: HashSet<Permission> = plugin
            .package
            .manifest
            .permissions
            .iter()
            .cloned()
            .collect();
        let mut store = Store::new(
            &self.engine,
            HostState {
                granted,
                output: None,
            },
        );
        let instance = linker.instantiate(&mut store, &module)?;
        let memory = extract_memory(&mut store, &instance)?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .context("plugin must export alloc(i32)->i32")?;
        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "dealloc")
            .context("plugin must export dealloc(i32,i32)")?;
        let run = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, "dosh_run")
            .context("plugin must export dosh_run(ptr,len)->i64")?;

        let req = RunRequest {
            command: command.to_string(),
            args: args.to_vec(),
            cwd,
        };
        let req_bytes = serde_json::to_vec(&req)?;
        let input_ptr = alloc.call(&mut store, req_bytes.len() as i32)?;
        memory.write(&mut store, input_ptr as usize, &req_bytes)?;
        let packed = run.call(&mut store, (input_ptr, req_bytes.len() as i32))?;
        dealloc.call(&mut store, (input_ptr, req_bytes.len() as i32))?;

        let out_ptr = (packed >> 32) as i32;
        let out_len = (packed & 0xffff_ffff) as i32;
        let out = read_memory(&memory, &mut store, out_ptr, out_len)?;
        dealloc.call(&mut store, (out_ptr, out_len))?;

        let mut response: RunResponse = serde_json::from_slice(&out)?;
        if response.output.is_none() {
            response.output = store.data().output.clone();
        }
        Ok(response)
    }
}

fn extract_memory<T>(store: &mut Store<T>, instance: &wasmtime::Instance) -> Result<Memory> {
    let Some(Extern::Memory(memory)) = instance.get_export(store, "memory") else {
        anyhow::bail!("plugin must export linear memory as `memory`");
    };
    Ok(memory)
}

fn read_memory<T>(memory: &Memory, store: &mut Store<T>, ptr: i32, len: i32) -> Result<Vec<u8>> {
    let mut out = vec![0_u8; len.max(0) as usize];
    memory.read(store, ptr as usize, &mut out)?;
    Ok(out)
}

fn read_guest_string(caller: &mut Caller<'_, HostState>, ptr: i32, len: i32) -> Result<String> {
    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
        anyhow::bail!("guest memory export not found");
    };
    let mut out = vec![0_u8; len.max(0) as usize];
    memory.read(&*caller, ptr as usize, &mut out)?;
    Ok(String::from_utf8(out)?)
}

fn file_modified_time(path: &Path) -> Result<SystemTime> {
    Ok(fs::metadata(path)?.modified()?)
}

fn decode_permission(code: i32) -> Permission {
    match code {
        0 => Permission::ReadConfig,
        1 => Permission::ReadTheme,
        2 => Permission::ReadFonts,
        3 => Permission::ReadPluginData,
        4 => Permission::WritePluginData,
        5 => Permission::NetworkAccess,
        6 => Permission::SpawnProcess,
        _ => Permission::ReadPluginData,
    }
}
