use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use walkdir::WalkDir;

use crate::{PluginManifest, PluginSource};

pub fn init_plugin_scaffold(dir: &Path, name: &str) -> Result<PathBuf> {
    let plugin_dir = dir.join(name);
    fs::create_dir_all(&plugin_dir)?;
    let manifest_path = plugin_dir.join("dosh.plugin.toml");
    let src_dir = plugin_dir.join("src");
    let cargo_toml = plugin_dir.join("Cargo.toml");
    let readme = plugin_dir.join("README.md");
    fs::create_dir_all(&src_dir)?;
    if !manifest_path.exists() {
        let content = format!(
            "[plugin]\nname = \"{name}\"\nversion = \"0.1.0\"\ndescription = \"{name} plugin\"\nauthor = \"unknown\"\nlicense = \"MIT\"\nentry = \"plugin.wasm\"\ntype = \"wasm\"\nminimum_dosh_version = \"0.1.0\"\n\n[[commands]]\nname = \"{name}.run\"\nusage = \"{name}.run\"\ndescription = \"Execute {name}\"\ninput = \"any\"\noutput = \"text\"\n\n[permissions]\nfilesystem = \"none\"\nnetwork = false\nprocess = false\nenv = \"read\"\nsecret = false\n"
        );
        fs::write(&manifest_path, content)?;
    }
    if !cargo_toml.exists() {
        fs::write(
            &cargo_toml,
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\nserde = {{ version = \"1\", features = [\"derive\"] }}\nserde_json = \"1\"\n"
            ),
        )?;
    }
    if !src_dir.join("lib.rs").exists() {
        fs::write(
            src_dir.join("lib.rs"),
            r#"use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct RunRequest {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    exit_code: i32,
    output: Option<String>,
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: i32) -> i32 {
    let mut buf = Vec::<u8>::with_capacity(len.max(0) as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: i32, len: i32) {
    if ptr == 0 || len < 0 {
        return;
    }
    unsafe {
        let _ = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dosh_run(ptr: i32, len: i32) -> i64 {
    let input = unsafe { std::slice::from_raw_parts(ptr as *const u8, len.max(0) as usize) };
    let req: RunRequest = serde_json::from_slice(input).unwrap_or(RunRequest {
        command: "unknown".into(),
        args: vec![],
        cwd: None,
    });
    let response = RunResponse {
        exit_code: 0,
        output: Some(format!("plugin command={} args={:?} cwd={:?}", req.command, req.args, req.cwd)),
    };
    let bytes = serde_json::to_vec(&response).unwrap_or_default();
    let out_len = bytes.len() as i32;
    let out_ptr = alloc(out_len);
    if out_len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_ptr as *mut u8, out_len as usize);
        }
    }
    ((out_ptr as i64) << 32) | (out_len as u32 as i64)
}
"#,
        )?;
    }
    if !readme.exists() {
        fs::write(
            &readme,
            "# Dosh Plugin\n\nBuild your WASM plugin and output `plugin.wasm` next to manifest.\n",
        )?;
    }
    Ok(plugin_dir)
}

pub fn install_plugin(plugin_dir: &Path, plugins_root: &Path) -> Result<PathBuf> {
    let manifest = read_manifest(plugin_dir)?;
    let target = plugins_root.join(&manifest.name);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    copy_dir_all(plugin_dir, &target)?;
    Ok(target)
}

pub fn publish_plugin(plugin_dir: &Path, registry_root: &Path) -> Result<PathBuf> {
    let manifest = read_manifest(plugin_dir)?;
    let target = registry_root.join(&manifest.name).join(&manifest.version);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    copy_dir_all(plugin_dir, &target)?;
    Ok(target)
}

pub fn sign_plugin_manifest(
    plugin_dir: &Path,
    key_id: &str,
    private_key_b64: &str,
) -> Result<String> {
    let mut manifest = read_manifest(plugin_dir)?;
    if manifest.source != PluginSource::Wasm {
        anyhow::bail!("only wasm plugins are supported for signing");
    }
    let entry = manifest.entry.clone().context("manifest entry missing")?;
    let wasm_path = plugin_dir.join(entry);
    let payload = fs::read(&wasm_path)
        .with_context(|| format!("failed to read wasm entry {}", wasm_path.display()))?;

    let signing = parse_signing_key(private_key_b64)?;
    let signature: Signature = signing.sign(&payload);
    let sig_b64 = B64.encode(signature.to_bytes());
    let sig = format!("ed25519:{key_id}:{sig_b64}");
    manifest.signature = Some(sig.clone());
    let manifest_path = plugin_dir.join("plugin.toml");
    fs::write(&manifest_path, toml::to_string_pretty(&manifest)?)?;
    Ok(sig)
}

pub fn verify_plugin_signature(plugin_dir: &Path, public_key_b64: &str) -> Result<()> {
    let manifest = read_manifest(plugin_dir)?;
    let entry = manifest.entry.clone().context("manifest entry missing")?;
    let sig = manifest
        .signature
        .clone()
        .context("manifest signature missing")?;
    let Some(sig_b64) = sig.split(':').nth(2) else {
        anyhow::bail!("invalid ed25519 signature format");
    };
    let payload = fs::read(plugin_dir.join(entry))?;
    let key_bytes = B64.decode(public_key_b64.as_bytes())?;
    let key_arr: [u8; 32] = key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("public key must be 32 bytes"))?;
    let verify_key = VerifyingKey::from_bytes(&key_arr)?;
    let sig_bytes = B64.decode(sig_b64.as_bytes())?;
    let signature = Signature::from_slice(&sig_bytes)?;
    verify_key.verify(&payload, &signature)?;
    Ok(())
}

fn read_manifest(plugin_dir: &Path) -> Result<PluginManifest> {
    let manifest_path = if plugin_dir.join("dosh.plugin.toml").exists() {
        plugin_dir.join("dosh.plugin.toml")
    } else {
        plugin_dir.join("plugin.toml")
    };
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read manifest {}", manifest_path.display()))?;
    PluginManifest::from_toml_str(&raw)
}

fn parse_signing_key(private_key_b64: &str) -> Result<SigningKey> {
    let bytes = B64.decode(private_key_b64.as_bytes())?;
    match bytes.len() {
        32 => {
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid private key"))?;
            Ok(SigningKey::from_bytes(&arr))
        }
        64 => {
            let arr: [u8; 64] = bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid private key"))?;
            SigningKey::from_keypair_bytes(&arr).map_err(Into::into)
        }
        _ => anyhow::bail!("private key must be base64-encoded 32-byte seed or 64-byte keypair"),
    }
}

fn copy_dir_all(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in WalkDir::new(from).min_depth(1) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(from)?;
        let target = to.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
