use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use dosh_config::DoshPaths;
use dosh_plugin::{
    PluginManager, TrustedKeyRing, init_plugin_scaffold, publish_plugin, sign_plugin_manifest,
    verify_plugin_signature,
};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "dosh", about = "Dosh modern shell", version)]
struct Cli {
    #[arg(short = 'c', long = "command")]
    command: Option<String>,

    #[arg(short = 'p', long = "path")]
    path: Option<PathBuf>,

    #[arg(long = "login", default_value_t = false)]
    login: bool,

    #[arg(long = "interactive", default_value_t = false)]
    interactive: bool,

    #[arg(long = "no-config", default_value_t = false)]
    no_config: bool,

    #[arg(long = "safe-mode", default_value_t = false)]
    safe_mode: bool,

    #[command(subcommand)]
    subcommand: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Run {
        script: PathBuf,
    },
    Check {
        script: PathBuf,
    },
    Test {
        path: Option<PathBuf>,
    },
    Fmt {
        script: PathBuf,
        #[arg(long = "check", default_value_t = false)]
        check: bool,
    },
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
    List,
    Enable {
        name: String,
    },
    Disable {
        name: String,
    },
    Remove {
        name: String,
    },
    Init {
        #[arg(long = "name")]
        name: String,
        #[arg(long = "dir")]
        dir: Option<PathBuf>,
    },
    Install {
        #[arg(long = "from")]
        from: PathBuf,
    },
    Publish {
        #[arg(long = "from")]
        from: PathBuf,
        #[arg(long = "registry")]
        registry: PathBuf,
    },
    Sign {
        #[arg(long = "dir")]
        dir: PathBuf,
        #[arg(long = "key-id")]
        key_id: String,
        #[arg(long = "private-key")]
        private_key: String,
    },
    Verify {
        #[arg(long = "dir")]
        dir: PathBuf,
        #[arg(long = "public-key")]
        public_key: String,
    },
    Trust {
        #[command(subcommand)]
        command: TrustCommand,
    },
}

#[derive(Debug, Subcommand)]
enum TrustCommand {
    Add {
        #[arg(long = "id")]
        id: String,
        #[arg(long = "public-key")]
        public_key: String,
        #[arg(long = "store", value_enum, default_value = "both")]
        store: TrustStoreTarget,
    },
    List,
    Remove {
        #[arg(long = "id")]
        id: String,
        #[arg(long = "store", value_enum, default_value = "both")]
        store: TrustStoreTarget,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TrustStoreTarget {
    File,
    Os,
    Both,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(cmd) = cli.subcommand {
        return run_subcommand(cmd);
    }

    print_startup_banner();
    let mut shell = dosh_core::Shell::with_config(dosh_core::ShellConfig {
        interactive: cli.interactive || cli.command.is_none(),
        command: cli.command,
        start_path: cli.path,
        login: cli.login,
        no_config: cli.no_config,
        safe_mode: cli.safe_mode,
    });
    shell.run()
}

fn print_startup_banner() {
    const AUTHOR: &str = "DuongOnix";
    let version = env!("CARGO_PKG_VERSION");
    const YELLOW: &str = "\x1b[93m";
    const CYAN: &str = "\x1b[96m";
    const RESET: &str = "\x1b[0m";
    let meta_pad = " ".repeat(47);
    println!(
        r#"
________               .__      _________.__           .__  .__   
\______ \   ____  _____|  |__  /   _____/|  |__   ____ |  | |  |  
 |    |  \ /  _ \/  ___/  |  \ \_____  \ |  |  \_/ __ \|  | |  |  
 |    `   (  <_> )___ \|   Y  \/        \|   Y  \  ___/|  |_|  |__
/_______  /\____/____  >___|  /_______  /|___|  /\___  >____/____/
        \/           \/     \/        \/      \/     \/           

{meta_pad}Version: {YELLOW}{version}{RESET}
{meta_pad}Author:  {CYAN}{AUTHOR}{RESET}
"#
    );
}

fn run_subcommand(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Run { script } => {
            let content = std::fs::read_to_string(&script)?;
            let mut shell = dosh_core::Shell::with_config(dosh_core::ShellConfig {
                interactive: false,
                command: Some(content),
                start_path: None,
                login: false,
                no_config: false,
                safe_mode: false,
            });
            shell.run()
        }
        Commands::Check { script } => {
            let content = std::fs::read_to_string(&script)?;
            let _ = dosh_parser::Parser::new().parse_script(&content)?;
            println!("ok: {}", script.display());
            Ok(())
        }
        Commands::Test { path } => run_script_tests(path),
        Commands::Fmt { script, check } => run_script_fmt(script, check),
        Commands::Plugin { command } => match command {
            PluginCommand::List => {
                let paths = DoshPaths::detect()?;
                let manager = PluginManager::new(paths.plugins_dir(), paths.configs_dir());
                let items = manager.list()?;
                if items.is_empty() {
                    println!("no plugins installed");
                    return Ok(());
                }
                for item in items {
                    println!(
                        "{}\t{}\t{}\t{}",
                        item.name,
                        item.version,
                        if item.enabled { "enabled" } else { "disabled" },
                        item.install_dir
                    );
                }
                Ok(())
            }
            PluginCommand::Enable { name } => {
                let paths = DoshPaths::detect()?;
                let manager = PluginManager::new(paths.plugins_dir(), paths.configs_dir());
                manager.set_enabled(&name, true)?;
                println!("plugin enabled: {name}");
                Ok(())
            }
            PluginCommand::Disable { name } => {
                let paths = DoshPaths::detect()?;
                let manager = PluginManager::new(paths.plugins_dir(), paths.configs_dir());
                manager.set_enabled(&name, false)?;
                println!("plugin disabled: {name}");
                Ok(())
            }
            PluginCommand::Remove { name } => {
                let paths = DoshPaths::detect()?;
                let manager = PluginManager::new(paths.plugins_dir(), paths.configs_dir());
                manager.uninstall(&name)?;
                println!("plugin removed: {name}");
                Ok(())
            }
            PluginCommand::Init { name, dir } => {
                let base = if let Some(d) = dir {
                    d
                } else {
                    std::env::current_dir()?
                };
                let created = init_plugin_scaffold(&base, &name)?;
                println!("plugin scaffold created: {}", created.display());
                Ok(())
            }
            PluginCommand::Install { from } => {
                let paths = DoshPaths::detect()?;
                let manager = PluginManager::new(paths.plugins_dir(), paths.configs_dir());
                let entry = manager.install_from_path(&from)?;
                println!("plugin installed: {}", entry.install_dir);
                Ok(())
            }
            PluginCommand::Publish { from, registry } => {
                let target = publish_plugin(&from, &registry)?;
                println!("plugin published: {}", target.display());
                Ok(())
            }
            PluginCommand::Sign {
                dir,
                key_id,
                private_key,
            } => {
                let signature = sign_plugin_manifest(&dir, &key_id, &private_key)?;
                println!("plugin signed: {signature}");
                Ok(())
            }
            PluginCommand::Verify { dir, public_key } => {
                verify_plugin_signature(&dir, &public_key)?;
                println!("plugin signature verified");
                Ok(())
            }
            PluginCommand::Trust { command } => run_trust_command(command),
        },
    }
}

fn run_script_tests(path: Option<PathBuf>) -> Result<()> {
    let mut targets = Vec::new();
    if let Some(path) = path {
        targets.push(path);
    } else {
        for entry in walkdir::WalkDir::new(std::env::current_dir()?)
            .into_iter()
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            let is_dosh = p.extension().and_then(|e| e.to_str()) == Some("dosh");
            if !is_dosh {
                continue;
            }
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let under_tests = p.components().any(|c| c.as_os_str() == "tests");
            if name.ends_with("_test.dosh") || under_tests {
                targets.push(p.to_path_buf());
            }
        }
    }

    targets.sort();
    targets.dedup();
    if targets.is_empty() {
        println!("no test scripts found");
        return Ok(());
    }

    let mut failed = 0usize;
    let mut total = 0usize;
    for script in targets {
        let content = std::fs::read_to_string(&script)?;
        match dosh_parser::Parser::new().parse_script(&content) {
            Ok(ast) => {
                let runtime = dosh_runtime::Runtime::new();
                let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
                let mut env = dosh_env::EnvContext::new(cwd);
                match runtime.execute_tests(&ast, &mut env) {
                    Ok(report) => {
                        total += report.total;
                        if report.failed == 0 {
                            println!("PASS {} ({} tests)", script.display(), report.passed);
                        } else {
                            failed += report.failed;
                            for case in report.cases.into_iter().filter(|c| !c.passed) {
                                eprintln!(
                                    "FAIL {} :: {}: {}",
                                    script.display(),
                                    case.name,
                                    case.error.unwrap_or_else(|| "unknown error".to_string())
                                );
                            }
                        }
                    }
                    Err(err) => {
                        failed += 1;
                        eprintln!("FAIL {}: {err}", script.display());
                    }
                }
            }
            Err(err) => {
                failed += 1;
                eprintln!("FAIL {}: {err}", script.display());
            }
        }
    }
    if failed > 0 {
        anyhow::bail!("{failed} script test(s) failed");
    }
    if total == 0 {
        println!("no test blocks found");
    } else {
        println!("ok: {total} test(s) passed");
    }
    Ok(())
}

fn run_script_fmt(script: PathBuf, check: bool) -> Result<()> {
    let content = std::fs::read_to_string(&script)?;
    let _ = dosh_parser::Parser::new().parse_script(&content)?;
    let normalized = content.replace("\r\n", "\n");
    let mut out_lines = Vec::new();
    for line in normalized.lines() {
        out_lines.push(line.trim_end().to_string());
    }
    let mut formatted = out_lines.join("\n");
    if !formatted.ends_with('\n') {
        formatted.push('\n');
    }

    if check {
        if formatted != normalized {
            anyhow::bail!("format check failed: {}", script.display());
        }
        println!("ok: {}", script.display());
        return Ok(());
    }

    if formatted != normalized {
        std::fs::write(&script, formatted)?;
        println!("formatted: {}", script.display());
    } else {
        println!("already formatted: {}", script.display());
    }
    Ok(())
}

fn run_trust_command(cmd: TrustCommand) -> Result<()> {
    let paths = DoshPaths::detect()?;
    let keyring_path = paths.plugins_dir().join("trusted-keys.toml");
    let mut keyring = TrustedKeyRing::from_toml_file(&keyring_path)?;

    match cmd {
        TrustCommand::Add {
            id,
            public_key,
            store,
        } => {
            if matches!(store, TrustStoreTarget::File | TrustStoreTarget::Both) {
                keyring.upsert_key_from_base64(&id, &public_key)?;
                keyring.save_to_toml_file(&keyring_path)?;
            }
            if matches!(store, TrustStoreTarget::Os | TrustStoreTarget::Both) {
                TrustedKeyRing::set_os_key_from_base64(&id, &public_key)?;
            }
            println!("trusted key added: {id}");
        }
        TrustCommand::List => {
            let mut ids = keyring.list_key_ids();
            ids.sort();
            if ids.is_empty() {
                println!("no trusted keys in file store: {}", keyring_path.display());
            } else {
                for id in ids {
                    println!("{id}");
                }
            }
        }
        TrustCommand::Remove { id, store } => {
            let mut removed_any = false;
            if matches!(store, TrustStoreTarget::File | TrustStoreTarget::Both) {
                removed_any |= keyring.remove_key(&id);
                keyring.save_to_toml_file(&keyring_path)?;
            }
            if matches!(store, TrustStoreTarget::Os | TrustStoreTarget::Both) {
                let _ = TrustedKeyRing::remove_os_key(&id);
                removed_any = true;
            }
            if removed_any {
                println!("trusted key removed: {id}");
            } else {
                println!("trusted key not found: {id}");
            }
        }
    }
    Ok(())
}
