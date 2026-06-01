use anyhow::{Result, anyhow};
use flate2::read::GzDecoder;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;
use zip::ZipArchive;

pub fn extract_archive(archive_path: &Path, out_dir: &Path) -> Result<()> {
    let name = archive_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.ends_with(".zip") {
        let file = fs::File::open(archive_path)?;
        let mut zip = ZipArchive::new(file)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            let out = out_dir.join(f.name());
            if f.is_dir() {
                fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = fs::File::create(&out)?;
            std::io::copy(&mut f, &mut out_file)?;
        }
        return Ok(());
    }
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let file = fs::File::open(archive_path)?;
        let dec = GzDecoder::new(file);
        let mut tar = Archive::new(dec);
        tar.unpack(out_dir)?;
        return Ok(());
    }
    Err(anyhow!(
        "unsupported archive format: {}",
        archive_path.display()
    ))
}

pub fn find_binary(root: &Path, bin_name: &str) -> Option<PathBuf> {
    let flat = root.join(bin_name);
    if flat.is_file() {
        return Some(flat);
    }
    for e in walkdir::WalkDir::new(root).into_iter().flatten() {
        if !e.file_type().is_file() {
            continue;
        }
        if e.file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case(bin_name)
        {
            return Some(e.path().to_path_buf());
        }
    }
    None
}
