mod app;
mod buffer;
mod completion;
mod diagnostics;
mod highlight;
mod ui;

use anyhow::Result;
use std::path::Path;

pub fn run(path: &Path) -> Result<()> {
    app::run_editor(path)
}
