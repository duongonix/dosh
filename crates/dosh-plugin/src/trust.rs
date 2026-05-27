use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::manifest::PluginManifest;

#[derive(Debug, Clone)]
pub struct TrustPolicy {
    pub allow_unsigned: bool,
    pub trusted_keys: TrustedKeyRing,
    pub store: Option<TrustStore>,
}

impl Default for TrustPolicy {
    fn default() -> Self {
        Self {
            allow_unsigned: true,
            trusted_keys: TrustedKeyRing::default(),
            store: None,
        }
    }
}

impl TrustPolicy {
    pub fn from_keyring_file(path: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            allow_unsigned: false,
            trusted_keys: TrustedKeyRing::from_toml_file(path)?,
            store: None,
        })
    }

    pub fn with_store(mut self, store: TrustStore) -> Self {
        self.store = Some(store);
        self
    }

    pub fn verify_plugin_file(&self, manifest: &PluginManifest, wasm_path: &Path) -> Result<()> {
        if let Some(store) = &self.store {
            if !store.is_plugin_trusted(&manifest.name) && manifest.signature.is_none() {
                anyhow::bail!("plugin is not trusted and unsigned: {}", manifest.name);
            }
        }
        let bytes = fs::read(wasm_path)?;
        match manifest.signature.as_deref() {
            None if self.allow_unsigned => Ok(()),
            None => anyhow::bail!("plugin signature is required"),
            Some(sig) if sig.starts_with("sha256:") => verify_sha256(sig, &bytes),
            Some(sig) if sig.starts_with("ed25519:") => self.verify_ed25519(sig, &bytes),
            Some(_) => anyhow::bail!("unsupported plugin signature scheme"),
        }
    }

    fn verify_ed25519(&self, sig_spec: &str, bytes: &[u8]) -> Result<()> {
        // Format: ed25519:<key_id>:<base64_signature>
        let mut parts = sig_spec.splitn(3, ':');
        let _scheme = parts.next();
        let key_id = parts
            .next()
            .filter(|s| !s.trim().is_empty())
            .context("missing key id in ed25519 signature")?;
        let sig_b64 = parts
            .next()
            .filter(|s| !s.trim().is_empty())
            .context("missing signature in ed25519 signature")?;

        let pubkey = self
            .trusted_keys
            .resolve_key(key_id)
            .with_context(|| format!("trusted key not found: {key_id}"))?;
        let pubkey_arr: [u8; 32] = pubkey
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("ed25519 public key for `{key_id}` must be 32 bytes"))?;
        let verify_key = VerifyingKey::from_bytes(&pubkey_arr)?;

        let sig_bytes = B64.decode(sig_b64)?;
        let signature = Signature::from_slice(&sig_bytes)?;
        verify_key
            .verify(bytes, &signature)
            .context("ed25519 signature verification failed")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrustStore {
    #[serde(default)]
    pub trusted_plugins: BTreeSet<String>,
    #[serde(default)]
    pub trusted_publishers: BTreeSet<String>,
}

impl TrustStore {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(toml::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn trust_plugin(&mut self, plugin: &str) {
        self.trusted_plugins.insert(plugin.to_string());
    }

    pub fn untrust_plugin(&mut self, plugin: &str) {
        self.trusted_plugins.remove(plugin);
    }

    pub fn is_plugin_trusted(&self, plugin: &str) -> bool {
        self.trusted_plugins.contains(plugin)
    }
}

fn verify_sha256(sig_spec: &str, bytes: &[u8]) -> Result<()> {
    let expected = sig_spec.strip_prefix("sha256:").unwrap_or(sig_spec);
    let digest = Sha256::digest(bytes);
    let actual = format!("{:x}", digest);
    if !actual.eq_ignore_ascii_case(expected) {
        anyhow::bail!("plugin signature mismatch");
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct TrustedKeyRing {
    keys: BTreeMap<String, Vec<u8>>,
}

impl TrustedKeyRing {
    pub fn from_toml_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read trusted keyring at {}", path.display()))?;
        let parsed: TrustedKeysFile = toml::from_str(&raw)
            .with_context(|| format!("invalid trusted keyring TOML at {}", path.display()))?;
        let mut keys = BTreeMap::new();
        for (id, b64) in parsed.keys {
            let bytes = B64
                .decode(b64.as_bytes())
                .with_context(|| format!("invalid base64 public key for key id `{id}`"))?;
            keys.insert(id, bytes);
        }
        Ok(Self { keys })
    }

    pub fn to_toml_string(&self) -> Result<String> {
        let mut out = String::from("[keys]\n");
        for (id, key) in &self.keys {
            out.push_str(&format!("{id} = \"{}\"\n", B64.encode(key)));
        }
        Ok(out)
    }

    pub fn save_to_toml_file(&self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_toml_string()?)?;
        Ok(())
    }

    pub fn list_key_ids(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }

    pub fn upsert_key_from_base64(&mut self, key_id: &str, pubkey_b64: &str) -> Result<()> {
        let bytes = B64
            .decode(pubkey_b64.as_bytes())
            .with_context(|| format!("invalid base64 public key for key id `{key_id}`"))?;
        if bytes.len() != 32 {
            anyhow::bail!("ed25519 public key for `{key_id}` must be 32 bytes");
        }
        self.keys.insert(key_id.to_string(), bytes);
        Ok(())
    }

    pub fn remove_key(&mut self, key_id: &str) -> bool {
        self.keys.remove(key_id).is_some()
    }

    pub fn set_os_key_from_base64(key_id: &str, pubkey_b64: &str) -> Result<()> {
        let bytes = B64.decode(pubkey_b64.as_bytes())?;
        if bytes.len() != 32 {
            anyhow::bail!("ed25519 public key for `{key_id}` must be 32 bytes");
        }
        let entry = keyring::Entry::new("dosh-plugin-trust", key_id)?;
        entry.set_password(pubkey_b64)?;
        Ok(())
    }

    pub fn remove_os_key(key_id: &str) -> Result<()> {
        let entry = keyring::Entry::new("dosh-plugin-trust", key_id)?;
        let _ = entry.delete_credential();
        Ok(())
    }

    pub fn resolve_key(&self, key_id: &str) -> Result<Vec<u8>> {
        if let Some(k) = self.keys.get(key_id) {
            return Ok(k.clone());
        }

        // Fallback to OS keychain entry:
        // service=dosh-plugin-trust, username=<key_id>, secret=base64(pubkey-32b)
        let entry = keyring::Entry::new("dosh-plugin-trust", key_id)?;
        let secret = entry.get_password()?;
        let bytes = B64.decode(secret.as_bytes())?;
        Ok(bytes)
    }
}

#[derive(Debug, Deserialize)]
struct TrustedKeysFile {
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use ed25519_dalek::{Signer, SigningKey};

    use crate::{Permission, PluginSource};

    use super::*;

    #[test]
    fn verifies_ed25519_signature_from_file_keyring() {
        let signing = SigningKey::from_bytes(&[7_u8; 32]);
        let verify = signing.verifying_key();
        let payload = b"wasm-payload";
        let signature = signing.sign(payload);

        let pubkey_b64 = B64.encode(verify.as_bytes());
        let sig_b64 = B64.encode(signature.to_bytes());
        let sig_spec = format!("ed25519:test-key:{sig_b64}");

        let mut manifest = crate::PluginManifest {
            name: "demo".into(),
            version: "1.0.0".into(),
            description: None,
            author: None,
            license: None,
            homepage: None,
            repository: None,
            source: PluginSource::Wasm,
            target: None,
            minimum_dosh_version: None,
            permissions: vec![Permission::ReadConfig],
            permission_set: Default::default(),
            command_names: vec!["demo.run".into()],
            command_metadata: vec![],
            entry: Some("plugin.wasm".into()),
            checksum: None,
            dependencies: vec![],
            api_version: None,
            min_shell_version: None,
            max_shell_version: None,
            signature: Some(sig_spec),
            hot_reload: true,
        };

        let tmp = std::env::temp_dir().join("dosh-trust-test");
        let _ = fs::create_dir_all(&tmp);
        let wasm = tmp.join("plugin.wasm");
        let mut f = fs::File::create(&wasm).unwrap();
        f.write_all(payload).unwrap();
        let keyring = tmp.join("trusted-keys.toml");
        fs::write(&keyring, format!("[keys]\ntest-key = \"{pubkey_b64}\"\n")).unwrap();

        let policy = TrustPolicy::from_keyring_file(&keyring).unwrap();
        assert!(policy.verify_plugin_file(&manifest, &wasm).is_ok());

        manifest.signature = Some("ed25519:test-key:Zm9v".into());
        assert!(policy.verify_plugin_file(&manifest, &wasm).is_err());
    }
}
