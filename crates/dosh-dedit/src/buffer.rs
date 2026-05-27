use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TextBuffer {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub dirty: bool,
}

impl TextBuffer {
    pub fn open(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).unwrap_or_default();
        let mut lines = text.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Ok(Self {
            path: path.to_path_buf(),
            lines,
            dirty: false,
        })
    }

    pub fn save(&mut self) -> Result<()> {
        let text = self.lines.join("\n");
        fs::write(&self.path, text)?;
        self.dirty = false;
        Ok(())
    }

    pub fn line(&self, row: usize) -> &str {
        self.lines.get(row).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn line_mut(&mut self, row: usize) -> &mut String {
        if row >= self.lines.len() {
            self.lines.resize_with(row + 1, String::new);
        }
        &mut self.lines[row]
    }
}
