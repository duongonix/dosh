use anyhow::{Result, bail};
use dosh_env::EnvContext;

use crate::registry::BuiltinOutcome;

pub fn run_shell_builtin(
    name: &str,
    args: &[String],
    env: &mut EnvContext,
) -> Result<Option<BuiltinOutcome>> {
    let outcome = match name {
        "cd" => Some(cd(args, env)?),
        "pwd" => Some(BuiltinOutcome::ok(Some(env.cwd().display().to_string()))),
        "echo" => Some(BuiltinOutcome::ok(Some(args.join(" ")))),
        "exit" => Some(BuiltinOutcome {
            exit_code: 0,
            should_exit: true,
            output: None,
        }),
        _ => None,
    };

    Ok(outcome)
}

fn cd(args: &[String], env: &mut EnvContext) -> Result<BuiltinOutcome> {
    if args.len() > 1 {
        bail!("cd expects zero or one argument")
    }

    let target = args
        .first()
        .cloned()
        .or_else(|| std::env::var("HOME").ok())
        .or_else(|| std::env::var("USERPROFILE").ok())
        .ok_or_else(|| anyhow::anyhow!("home directory is not available"))?;

    env.change_dir(&target)?;
    Ok(BuiltinOutcome::ok(None))
}
