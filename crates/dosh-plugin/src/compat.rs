use anyhow::Result;

use crate::manifest::PluginManifest;

pub fn ensure_compatible(
    manifest: &PluginManifest,
    shell_version: &str,
    wit_version: &str,
) -> Result<()> {
    let min_ref = manifest
        .min_shell_version
        .as_ref()
        .or(manifest.minimum_dosh_version.as_ref());
    if let Some(min) = min_ref {
        if cmp_versions(shell_version, min) < 0 {
            anyhow::bail!("plugin requires shell >= {min}, current is {shell_version}");
        }
    }
    if let Some(max) = &manifest.max_shell_version {
        if cmp_versions(shell_version, max) > 0 {
            anyhow::bail!("plugin requires shell <= {max}, current is {shell_version}");
        }
    }
    if let Some(api) = &manifest.api_version {
        if api.trim() != wit_version {
            anyhow::bail!("plugin api version mismatch: plugin={api}, host={wit_version}");
        }
    }
    Ok(())
}

fn cmp_versions(a: &str, b: &str) -> i8 {
    let pa = parse_version(a);
    let pb = parse_version(b);
    for i in 0..3 {
        match pa[i].cmp(&pb[i]) {
            std::cmp::Ordering::Less => return -1,
            std::cmp::Ordering::Equal => continue,
            std::cmp::Ordering::Greater => return 1,
        }
    }
    0
}

fn parse_version(s: &str) -> [u64; 3] {
    let mut out = [0_u64; 3];
    for (i, seg) in s.split('.').take(3).enumerate() {
        out[i] = seg.parse::<u64>().unwrap_or(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::{Permission, PluginManifest, PluginSource};

    use super::ensure_compatible;

    #[test]
    fn enforces_min_max() {
        let m = PluginManifest {
            name: "p".into(),
            version: "1.0.0".into(),
            source: PluginSource::Wasm,
            permissions: vec![Permission::ReadConfig],
            permission_set: Default::default(),
            command_names: vec!["p.run".into()],
            command_metadata: vec![],
            entry: Some("x.wasm".into()),
            description: None,
            author: None,
            license: None,
            homepage: None,
            repository: None,
            target: None,
            minimum_dosh_version: None,
            checksum: None,
            dependencies: vec![],
            api_version: Some("v1".into()),
            min_shell_version: Some("1.2.0".into()),
            max_shell_version: Some("1.5.0".into()),
            signature: None,
            hot_reload: true,
        };
        assert!(ensure_compatible(&m, "1.3.0", "v1").is_ok());
        assert!(ensure_compatible(&m, "1.1.0", "v1").is_err());
        assert!(ensure_compatible(&m, "1.6.0", "v1").is_err());
    }
}
