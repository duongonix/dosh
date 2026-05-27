use anyhow::Result;
use clap::Parser;
use dosh_durl::{DurlRunOptions, run};
use dosh_value::Value;

#[derive(Parser, Debug)]
#[command(name = "durl", version)]
#[command(about = "Structured HTTP client")]
struct Cli {
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let out = run(DurlRunOptions {
        args: cli.args,
        input: None,
    })?;
    match out {
        Value::String(s) => println!("{s}"),
        v => println!("{v}"),
    }
    Ok(())
}
