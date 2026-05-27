use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("incompatible plugin: {0}")]
    Incompatible(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("untrusted plugin: {0}")]
    Untrusted(String),
    #[error("checksum mismatch for {0}")]
    ChecksumMismatch(String),
    #[error("signature verify failed: {0}")]
    SignatureVerifyFailed(String),
    #[error("plugin not found: {0}")]
    NotFound(String),
}
