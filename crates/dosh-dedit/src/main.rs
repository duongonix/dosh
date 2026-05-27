use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "dedit")]
#[command(about = "Dosh terminal editor", long_about = None)]
struct Cli {
    path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    dosh_dedit::run(&cli.path)
}
