use crate::model::{ExitStatus, PipelineStream, RuntimeContext};
use crate::state::RuntimeState;
use crate::{ControlFlow, Runtime, RuntimeOutcome, ok_outcome};
use anyhow::Result;
use dosh_ast::{Expression, Pipeline};
use dosh_builtins::PipelineData;
use dosh_env::EnvContext;
use dosh_parser::parse_expression_result;
use dosh_value::{Record as DRecord, Value as DValue};
use std::process::{Command as ProcessCommand, Stdio};

impl Runtime {
    pub(crate) fn execute_pipeline(
        &self,
        pipeline: &Pipeline,
        env: &mut EnvContext,
        state: &RuntimeState,
    ) -> Result<RuntimeOutcome> {
        if pipeline.commands.is_empty() {
            return Ok(ok_outcome());
        }
        let _ctx = RuntimeContext::from_env(env);

        if pipeline.commands.iter().any(|c| c.background) {
            return Ok(RuntimeOutcome {
                exit_code: 2,
                should_exit: false,
                output: Some("background pipelines are not supported yet".to_string()),
                flow: ControlFlow::None,
            });
        }

        let mut stream = PipelineStream {
            data: PipelineData::Empty,
        };
        let mut status = ExitStatus { code: 0 };

        for cmd in &pipeline.commands {
            if cmd.name == "__literal__" {
                let raw = cmd.args.first().cloned().unwrap_or_default();
                let expr = parse_expression_result(&raw)?;
                stream.data = PipelineData::Value(expression_to_value(expr));
                continue;
            }

            if !cmd.force_external
                && let Some(var_name) = cmd.name.strip_prefix('$')
            {
                if let Some(v) = state.get_var(var_name) {
                    stream.data = PipelineData::Value(expression_to_value(v));
                    continue;
                }
                return Ok(RuntimeOutcome {
                    exit_code: 2,
                    should_exit: false,
                    output: Some(format!("Variable not found: ${var_name}")),
                    flow: ControlFlow::None,
                });
            }

            let (resolved_name, resolved_args) = self
                .builtins
                .expand_alias(&cmd.name, &cmd.args)
                .unwrap_or_else(|| (cmd.name.clone(), cmd.args.clone()));

            if let Ok(mut plugins) = self.wasm_plugins.lock()
                && let Some(resp) = plugins.run_command(
                    &resolved_name,
                    &resolved_args,
                    Some(env.cwd().display().to_string()),
                )?
            {
                status.code = resp.exit_code;
                stream.data = PipelineData::Text(resp.output.unwrap_or_default());
                continue;
            }

            if !cmd.force_external
                && let Some(out) =
                    self.builtins
                        .run(&resolved_name, &resolved_args, stream.data.clone(), env)?
            {
                status.code = out.exit_code;
                stream.data = out.output.clone();
                if out.should_exit {
                    return Ok(RuntimeOutcome::from_builtin(out));
                }
                continue;
            }

            let exec_name = if cmd.force_external {
                if let Some(var_name) = resolved_name.strip_prefix('$') {
                    if let Some(v) = state.get_var(var_name) {
                        match v {
                            dosh_ast::Expression::StringLiteral(s) => s,
                            other => format!("{other:?}"),
                        }
                    } else {
                        return Ok(RuntimeOutcome {
                            exit_code: 2,
                            should_exit: false,
                            output: Some(format!("Variable not found: ${var_name}")),
                            flow: ControlFlow::None,
                        });
                    }
                } else {
                    resolved_name.clone()
                }
            } else {
                resolved_name.clone()
            };
            let spawn_name = crate::external::resolve_program_for_spawn(&exec_name);
            let mut process = ProcessCommand::new(&spawn_name);
            process.args(&resolved_args).current_dir(env.cwd());
            process.stdin(Stdio::piped()).stdout(Stdio::piped());
            crate::io::apply_redirects(&mut process, &cmd.redirects)?;

            let mut child = process.spawn()?;
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                let input_text = stream.data.clone();
                let input_text = input_text.into_text();
                if !input_text.is_empty() {
                    stdin.write_all(input_text.as_bytes())?;
                }
            }
            let output = child.wait_with_output()?;
            status.code = output.status.code().unwrap_or(1);
            stream.data = PipelineData::Text(String::from_utf8_lossy(&output.stdout).to_string());
        }

        Ok(RuntimeOutcome {
            exit_code: status.code,
            should_exit: false,
            output: crate::io::pipeline_data_to_text(stream.data),
            flow: ControlFlow::None,
        })
    }
}

fn expression_to_value(expr: Expression) -> DValue {
    match expr {
        Expression::Null => DValue::Null,
        Expression::StringLiteral(s) => DValue::String(s),
        Expression::Integer(i) => DValue::Int(i),
        Expression::Float(f) => DValue::Float(f.parse::<f64>().unwrap_or(0.0)),
        Expression::Bool(b) => DValue::Bool(b),
        Expression::List(items) => DValue::List(
            items
                .into_iter()
                .map(expression_to_value)
                .collect::<Vec<_>>(),
        ),
        Expression::Record(fields) => {
            let mut rec = DRecord::new();
            for (k, v) in fields {
                rec.insert(k, expression_to_value(v));
            }
            DValue::Record(rec)
        }
        other => DValue::String(format!("{other:?}")),
    }
}
