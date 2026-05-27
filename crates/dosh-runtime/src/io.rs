use anyhow::Result;
use dosh_ast::Redirect;
use dosh_builtins::PipelineData;
use std::fs::OpenOptions;
use std::process::{Command as ProcessCommand, Stdio};

pub(crate) fn pipeline_data_to_text(data: PipelineData) -> Option<String> {
    match data {
        PipelineData::Empty => None,
        other => Some(other.into_text()),
    }
}

pub(crate) fn apply_redirects(process: &mut ProcessCommand, redirects: &[Redirect]) -> Result<()> {
    for redirect in redirects {
        match redirect {
            Redirect::Stdout(path) => {
                let file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(path)?;
                process.stdout(Stdio::from(file));
            }
            Redirect::StdoutAppend(path) => {
                let file = OpenOptions::new().create(true).append(true).open(path)?;
                process.stdout(Stdio::from(file));
            }
            Redirect::Stdin(path) => {
                let file = OpenOptions::new().read(true).open(path)?;
                process.stdin(Stdio::from(file));
            }
            Redirect::Stderr(path) => {
                let file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(path)?;
                process.stderr(Stdio::from(file));
            }
        }
    }
    Ok(())
}
