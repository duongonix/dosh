use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    Macos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformTarget {
    pub os: Os,
    pub arch: Arch,
}

pub fn current_target() -> Result<PlatformTarget> {
    let os = if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else if cfg!(target_os = "macos") {
        Os::Macos
    } else {
        bail!("unsupported OS")
    };
    let arch = if cfg!(target_arch = "x86_64") {
        Arch::X86_64
    } else if cfg!(target_arch = "aarch64") {
        Arch::Aarch64
    } else {
        bail!("unsupported architecture")
    };
    Ok(PlatformTarget { os, arch })
}

impl PlatformTarget {
    pub fn asset_name(&self, version: &str) -> String {
        let os = match self.os {
            Os::Windows => "windows",
            Os::Linux => "linux",
            Os::Macos => "macos",
        };
        let arch = match self.arch {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        };
        let ext = if self.os == Os::Windows {
            "zip"
        } else {
            "tar.gz"
        };
        format!("dosh-v{version}-{os}-{arch}.{ext}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_are_correct() {
        assert_eq!(
            PlatformTarget {
                os: Os::Windows,
                arch: Arch::X86_64
            }
            .asset_name("1.0.4"),
            "dosh-v1.0.4-windows-x86_64.zip"
        );
        assert_eq!(
            PlatformTarget {
                os: Os::Linux,
                arch: Arch::Aarch64
            }
            .asset_name("1.0.4"),
            "dosh-v1.0.4-linux-aarch64.tar.gz"
        );
    }
}
