use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::simple_builtin;
use anyhow::{anyhow, bail};
use dosh_value::{DurationValue, FilesizeValue};

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(AddBuiltin),
        factory!(SubBuiltin),
        factory!(MulBuiltin),
        factory!(DivBuiltin),
        factory!(ModBuiltin),
        factory!(AbsBuiltin),
        factory!(PowBuiltin),
        factory!(SqrtBuiltin),
        factory!(ClampBuiltin),
        factory!(GtBuiltin),
        factory!(GteBuiltin),
        factory!(LtBuiltin),
        factory!(LteBuiltin),
        factory!(EqBuiltin),
        factory!(NeqBuiltin),
        factory!(IsEvenBuiltin),
        factory!(IsOddBuiltin),
    ]
}

simple_builtin!(
    AddBuiltin,
    "add",
    "add <n>",
    "Add number",
    &["10 | add 5"],
    |args, input, _ctx| binary_num(args, input, |a, b| a + b)
);
simple_builtin!(
    SubBuiltin,
    "sub",
    "sub <n>",
    "Subtract number",
    &["10 | sub 3"],
    |args, input, _ctx| binary_num(args, input, |a, b| a - b)
);
simple_builtin!(
    MulBuiltin,
    "mul",
    "mul <n>",
    "Multiply number",
    &["10 | mul 2"],
    |args, input, _ctx| binary_num(args, input, |a, b| a * b)
);
simple_builtin!(
    DivBuiltin,
    "div",
    "div <n>",
    "Divide number",
    &["10 | div 2"],
    |args, input, _ctx| {
        let b = parse_num_arg(args, 0)?;
        if b == 0.0 {
            bail!("division by zero")
        }
        unary_num(input, |a| a / b)
    }
);
simple_builtin!(
    ModBuiltin,
    "mod",
    "mod <n>",
    "Modulo",
    &["10 | mod 3"],
    |args, input, _ctx| {
        let b = parse_num_arg(args, 0)?;
        if b == 0.0 {
            bail!("mod by zero")
        }
        unary_num(input, |a| a % b)
    }
);
simple_builtin!(
    AbsBuiltin,
    "abs",
    "abs",
    "Absolute value",
    &["-10 | abs"],
    |_args, input, _ctx| unary_num(input, |a| a.abs())
);
simple_builtin!(
    PowBuiltin,
    "pow",
    "pow <n>",
    "Power",
    &["2 | pow 8"],
    |args, input, _ctx| binary_num(args, input, |a, b| a.powf(b))
);
simple_builtin!(
    SqrtBuiltin,
    "sqrt",
    "sqrt",
    "Square root",
    &["16 | sqrt"],
    |_args, input, _ctx| unary_num(input, |a| a.sqrt())
);
simple_builtin!(
    ClampBuiltin,
    "clamp",
    "clamp <min> <max>",
    "Clamp number in range",
    &["120 | clamp 0 100"],
    |args, input, _ctx| {
        let min = parse_num_arg(args, 0)?;
        let max = parse_num_arg(args, 1)?;
        unary_num(input, |a| a.clamp(min, max))
    }
);

simple_builtin!(
    GtBuiltin,
    "gt",
    "gt <n>",
    "Greater than",
    &["10 | gt 5"],
    |args, input, _ctx| cmp_num(args, input, |a, b| a > b)
);
simple_builtin!(
    GteBuiltin,
    "gte",
    "gte <n>",
    "Greater than or equal",
    &["10 | gte 10"],
    |args, input, _ctx| cmp_num(args, input, |a, b| a >= b)
);
simple_builtin!(
    LtBuiltin,
    "lt",
    "lt <n>",
    "Less than",
    &["10 | lt 20"],
    |args, input, _ctx| cmp_num(args, input, |a, b| a < b)
);
simple_builtin!(
    LteBuiltin,
    "lte",
    "lte <n>",
    "Less than or equal",
    &["10 | lte 10"],
    |args, input, _ctx| cmp_num(args, input, |a, b| a <= b)
);
simple_builtin!(
    EqBuiltin,
    "eq",
    "eq <n>",
    "Numeric equality",
    &["10 | eq 10"],
    |args, input, _ctx| cmp_num(args, input, |a, b| (a - b).abs() < f64::EPSILON)
);
simple_builtin!(
    NeqBuiltin,
    "neq",
    "neq <n>",
    "Numeric inequality",
    &["10 | neq 5"],
    |args, input, _ctx| cmp_num(args, input, |a, b| (a - b).abs() >= f64::EPSILON)
);

simple_builtin!(
    IsEvenBuiltin,
    "is-even",
    "is-even",
    "Check even integer",
    &["10 | is-even"],
    |_args, input, _ctx| {
        let n = input_to_i64(input)?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(
            n % 2 == 0,
        ))))
    }
);
simple_builtin!(
    IsOddBuiltin,
    "is-odd",
    "is-odd",
    "Check odd integer",
    &["11 | is-odd"],
    |_args, input, _ctx| {
        let n = input_to_i64(input)?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(
            n % 2 != 0,
        ))))
    }
);

pub(crate) fn convert_to_unit(value: &Value, unit: &str) -> Option<Value> {
    let u = unit.to_ascii_lowercase();
    match value {
        Value::String(s) => {
            if let Some(v) = Value::parse_filesize(s) {
                return convert_to_unit(&Value::Filesize(v), &u);
            }
            if let Some(v) = Value::parse_duration(s) {
                return convert_to_unit(&Value::Duration(v), &u);
            }
            None
        }
        Value::Filesize(FilesizeValue { bytes }) => {
            let div = match u.as_str() {
                "b" => 1_f64,
                "kb" => 1024_f64,
                "mb" => 1024_f64 * 1024_f64,
                "gb" => 1024_f64 * 1024_f64 * 1024_f64,
                "tb" => 1024_f64.powf(4.0),
                _ => return None,
            };
            let mut rec = Record::new();
            rec.insert("kind".into(), Value::String("filesize".into()));
            rec.insert("unit".into(), Value::String(u));
            rec.insert("value".into(), Value::Float(*bytes as f64 / div));
            rec.insert("bytes".into(), Value::Int(*bytes as i64));
            Some(Value::Record(rec))
        }
        Value::Duration(DurationValue { nanos }) => {
            let div = match u.as_str() {
                "ns" => 1_f64,
                "ms" => 1_000_000_f64,
                "sec" | "s" => 1_000_000_000_f64,
                "min" => 60_f64 * 1_000_000_000_f64,
                "hr" => 3600_f64 * 1_000_000_000_f64,
                "day" => 86_400_f64 * 1_000_000_000_f64,
                _ => return None,
            };
            let mut rec = Record::new();
            rec.insert("kind".into(), Value::String("duration".into()));
            rec.insert("unit".into(), Value::String(u));
            rec.insert("value".into(), Value::Float(*nanos as f64 / div));
            rec.insert("nanos".into(), Value::Int(*nanos as i64));
            Some(Value::Record(rec))
        }
        _ => None,
    }
}

fn unary_num(input: PipelineData, f: impl Fn(f64) -> f64) -> anyhow::Result<BuiltinOutcome> {
    let n = input_to_f64(input)?;
    Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(f(n)))))
}

fn binary_num(
    args: &[String],
    input: PipelineData,
    f: impl Fn(f64, f64) -> f64,
) -> anyhow::Result<BuiltinOutcome> {
    let a = input_to_f64(input)?;
    let b = parse_num_arg(args, 0)?;
    Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(f(
        a, b,
    )))))
}

fn cmp_num(
    args: &[String],
    input: PipelineData,
    f: impl Fn(f64, f64) -> bool,
) -> anyhow::Result<BuiltinOutcome> {
    let a = input_to_f64(input)?;
    let b = parse_num_arg(args, 0)?;
    Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(f(
        a, b,
    )))))
}

fn input_to_f64(input: PipelineData) -> anyhow::Result<f64> {
    let v = pipeline_to_value(input)?;
    Ok(match v {
        Value::Int(i) => i as f64,
        Value::Float(x) => x,
        Value::String(s) => s.trim().parse::<f64>()?,
        _ => bail!("expected number input"),
    })
}

fn input_to_i64(input: PipelineData) -> anyhow::Result<i64> {
    let v = pipeline_to_value(input)?;
    Ok(match v {
        Value::Int(i) => i,
        Value::Float(x) => x as i64,
        Value::String(s) => s.trim().parse::<i64>()?,
        _ => bail!("expected integer input"),
    })
}

fn parse_num_arg(args: &[String], idx: usize) -> anyhow::Result<f64> {
    args.get(idx)
        .ok_or_else(|| anyhow!("missing numeric argument"))?
        .parse::<f64>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_string_units_works() {
        let kb = convert_to_unit(&Value::String("1mb".into()), "kb").expect("convert");
        assert!(matches!(kb, Value::Record(_)));

        let min = convert_to_unit(&Value::String("90sec".into()), "min").expect("convert");
        assert!(matches!(min, Value::Record(_)));
    }
}
