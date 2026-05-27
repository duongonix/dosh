use anyhow::Result;
use dosh_ast::{Command, Redirect};

pub fn parse_pipeline_commands(tokens: &[String]) -> Result<Vec<Command>> {
    let mut segments: Vec<Vec<String>> = Vec::new();
    let mut current = Vec::new();
    for token in tokens {
        if token == "|" {
            if current.is_empty() {
                anyhow::bail!("invalid pipeline: empty segment");
            }
            segments.push(current);
            current = Vec::new();
        } else {
            current.push(token.clone());
        }
    }
    if current.is_empty() {
        anyhow::bail!("invalid pipeline: trailing pipe");
    }
    segments.push(current);

    let mut commands = Vec::with_capacity(segments.len());
    let segment_count = segments.len();
    for (idx, seg) in segments.into_iter().enumerate() {
        let cmd = parse_single_command_tokens(&seg)?;
        if idx + 1 < segment_count && cmd.background {
            anyhow::bail!("background '&' is only allowed at the end of a full command line");
        }
        commands.push(cmd);
    }

    Ok(commands)
}

pub fn parse_single_command_tokens(tokens: &[String]) -> Result<Command> {
    if tokens.is_empty() {
        anyhow::bail!("empty command");
    }

    let mut name = String::new();
    let mut args = Vec::new();
    let mut redirects = Vec::new();
    let mut background = false;
    let mut idx = 0usize;

    while idx < tokens.len() {
        let tok = &tokens[idx];
        match tok.as_str() {
            ">" | ">>" | "<" => {
                if is_expression_command(&name) {
                    if name.is_empty() {
                        name = tok.clone();
                    } else {
                        args.push(tok.clone());
                    }
                    idx += 1;
                    continue;
                }
                let Some(target) = tokens.get(idx + 1) else {
                    anyhow::bail!("missing redirect target after '{tok}'");
                };
                let redirect = match tok.as_str() {
                    ">" => Redirect::Stdout(target.clone()),
                    ">>" => Redirect::StdoutAppend(target.clone()),
                    "<" => Redirect::Stdin(target.clone()),
                    _ => unreachable!(),
                };
                redirects.push(redirect);
                idx += 2;
            }
            "&" => {
                if idx != tokens.len() - 1 {
                    anyhow::bail!("'&' must appear at the end of the command");
                }
                background = true;
                idx += 1;
            }
            _ => {
                if name.is_empty() {
                    name = tok.clone();
                } else {
                    args.push(tok.clone());
                }
                idx += 1;
            }
        }
    }

    if name.is_empty() {
        anyhow::bail!("command name is missing");
    }

    let mut force_external = false;
    if name == "^" {
        if args.is_empty() {
            anyhow::bail!("'^' expects command path or variable");
        }
        name = args.remove(0);
        force_external = true;
    } else if let Some(rest) = name.strip_prefix('^') {
        if rest.is_empty() {
            anyhow::bail!("'^' expects command path or variable");
        }
        name = rest.to_string();
        force_external = true;
    }

    Ok(Command {
        name,
        args,
        redirects,
        background,
        force_external,
    })
}

fn is_expression_command(name: &str) -> bool {
    matches!(name, "where" | "filter")
}
