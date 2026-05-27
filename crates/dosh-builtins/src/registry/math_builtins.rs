use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::simple_builtin;
use anyhow::bail;
use rand::Rng;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        || Box::new(MathBuiltin),
        || Box::new(SumBuiltin),
        || Box::new(AvgBuiltin),
        || Box::new(MinBuiltin),
        || Box::new(MaxBuiltin),
        || Box::new(MedianBuiltin),
        || Box::new(RoundBuiltin),
        || Box::new(FloorBuiltin),
        || Box::new(CeilBuiltin),
        || Box::new(RandomBuiltin),
    ]
}

simple_builtin!(
    MathBuiltin,
    "math",
    "math <op> <a> [b]",
    "Math operations: add|sub|mul|div|pow",
    &["math add 1 2"],
    |args, _input, _ctx| {
        if args.len() < 2 {
            bail!("math expects <op> <a> [b]")
        }
        let op = &args[0];
        let a = args[1].parse::<f64>()?;
        let b = args
            .get(2)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let out = match op.as_str() {
            "add" => a + b,
            "sub" => a - b,
            "mul" => a * b,
            "div" => {
                if b == 0.0 {
                    bail!("division by zero")
                } else {
                    a / b
                }
            }
            "pow" => a.powf(b),
            _ => bail!("unsupported math op"),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(out))))
    }
);
simple_builtin!(
    SumBuiltin,
    "sum",
    "sum",
    "Sum numeric list",
    &["echo [1,2,3] | from-json | sum"],
    |_args, input, _ctx| Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(
        number_list(&pipeline_to_value(input)?).iter().sum()
    ))))
);
simple_builtin!(
    AvgBuiltin,
    "avg",
    "avg",
    "Average numeric list",
    &["open nums.json | avg"],
    |_args, input, _ctx| {
        let nums = number_list(&pipeline_to_value(input)?);
        if nums.is_empty() {
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Null)));
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(
            nums.iter().sum::<f64>() / nums.len() as f64,
        ))))
    }
);
simple_builtin!(
    MinBuiltin,
    "min",
    "min",
    "Minimum numeric value",
    &["open nums.json | min"],
    |_args, input, _ctx| {
        let nums = number_list(&pipeline_to_value(input)?);
        let out = nums.into_iter().fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a| a.min(v)))
        });
        Ok(BuiltinOutcome::ok(PipelineData::Value(
            out.map(Value::Float).unwrap_or(Value::Null),
        )))
    }
);
simple_builtin!(
    MaxBuiltin,
    "max",
    "max",
    "Maximum numeric value",
    &["open nums.json | max"],
    |_args, input, _ctx| {
        let nums = number_list(&pipeline_to_value(input)?);
        let out = nums.into_iter().fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a| a.max(v)))
        });
        Ok(BuiltinOutcome::ok(PipelineData::Value(
            out.map(Value::Float).unwrap_or(Value::Null),
        )))
    }
);
simple_builtin!(
    MedianBuiltin,
    "median",
    "median",
    "Median numeric value",
    &["open nums.json | median"],
    |_args, input, _ctx| {
        let mut nums = number_list(&pipeline_to_value(input)?);
        if nums.is_empty() {
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Null)));
        }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = nums.len() / 2;
        let v = if nums.len() % 2 == 0 {
            (nums[mid - 1] + nums[mid]) / 2.0
        } else {
            nums[mid]
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(v))))
    }
);
simple_builtin!(
    RoundBuiltin,
    "round",
    "round [digits]",
    "Round numeric value with optional precision",
    &["echo 3.6 | parse float | round", "3.14159 | round 2"],
    |args, input, _ctx| {
        let n = first_number(&pipeline_to_value(input)?).unwrap_or(0.0);
        let digits = args
            .first()
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let factor = 10f64.powi(digits.max(0));
        let rounded = if digits <= 0 {
            n.round()
        } else {
            (n * factor).round() / factor
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(
            rounded,
        ))))
    }
);
simple_builtin!(
    FloorBuiltin,
    "floor",
    "floor",
    "Floor numeric value",
    &["echo 3.6 | parse float | floor"],
    |_args, input, _ctx| Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(
        first_number(&pipeline_to_value(input)?)
            .unwrap_or(0.0)
            .floor()
    ))))
);
simple_builtin!(
    CeilBuiltin,
    "ceil",
    "ceil",
    "Ceil numeric value",
    &["echo 3.1 | parse float | ceil"],
    |_args, input, _ctx| Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(
        first_number(&pipeline_to_value(input)?)
            .unwrap_or(0.0)
            .ceil()
    ))))
);
simple_builtin!(
    RandomBuiltin,
    "random",
    "random [min] [max]",
    "Generate random integer in range (default 0..100)",
    &["random", "random 1 6"],
    |args, _input, _ctx| {
        let min = args
            .first()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let max = args
            .get(1)
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(100);
        if max < min {
            bail!("random expects max >= min")
        }
        let n = rand::thread_rng().gen_range(min..=max);
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Int(n))))
    }
);

fn number_list(value: &Value) -> Vec<f64> {
    match value {
        Value::Int(v) => vec![*v as f64],
        Value::Float(v) => vec![*v],
        Value::List(vs) => vs
            .iter()
            .filter_map(|v| match v {
                Value::Int(i) => Some(*i as f64),
                Value::Float(f) => Some(*f),
                _ => None,
            })
            .collect(),
        Value::Table(t) => t
            .rows
            .iter()
            .flat_map(|r| r.values())
            .filter_map(|v| match v {
                Value::Int(i) => Some(*i as f64),
                Value::Float(f) => Some(*f),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn first_number(value: &Value) -> Option<f64> {
    number_list(value).into_iter().next()
}
