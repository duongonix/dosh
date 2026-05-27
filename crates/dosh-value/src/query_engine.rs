use crate::value::{Record, Value};
use anyhow::{Result, anyhow, bail};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Compare {
        left: Path,
        op: CompareOp,
        right: Literal,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path(pub Vec<String>);

#[derive(Debug, Clone, PartialEq)]
pub enum CompareOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    StartsWith,
    EndsWith,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Filesize(u64),
    Duration(i64),
}

pub fn parse_filter_expr(input: &str) -> Result<Expr> {
    let mut p = Parser::new(tokenize(input));
    let expr = p.parse_or()?;
    if p.peek().is_some() {
        bail!("unexpected trailing tokens")
    }
    Ok(expr)
}

pub fn eval_filter(expr: &Expr, row: &Record) -> bool {
    match expr {
        Expr::Compare { left, op, right } => {
            let lhs = get_path(row, left);
            let rhs = literal_to_value(right);
            compare(lhs, op, &rhs)
        }
        Expr::And(a, b) => eval_filter(a, row) && eval_filter(b, row),
        Expr::Or(a, b) => eval_filter(a, row) || eval_filter(b, row),
    }
}

fn compare(lhs: Option<&Value>, op: &CompareOp, rhs: &Value) -> bool {
    match op {
        CompareOp::Eq => lhs == Some(rhs),
        CompareOp::Ne => lhs != Some(rhs),
        CompareOp::Gt => ord_cmp(lhs, rhs).is_some_and(|o| o.is_gt()),
        CompareOp::Ge => ord_cmp(lhs, rhs).is_some_and(|o| o.is_gt() || o.is_eq()),
        CompareOp::Lt => ord_cmp(lhs, rhs).is_some_and(|o| o.is_lt()),
        CompareOp::Le => ord_cmp(lhs, rhs).is_some_and(|o| o.is_lt() || o.is_eq()),
        CompareOp::Contains => lhs
            .map(|v| v.to_string().contains(&rhs.to_string()))
            .unwrap_or(false),
        CompareOp::StartsWith => lhs
            .map(|v| v.to_string().starts_with(&rhs.to_string()))
            .unwrap_or(false),
        CompareOp::EndsWith => lhs
            .map(|v| v.to_string().ends_with(&rhs.to_string()))
            .unwrap_or(false),
    }
}

fn ord_cmp(lhs: Option<&Value>, rhs: &Value) -> Option<std::cmp::Ordering> {
    let lhs = lhs?;
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
        (Value::Int(a), Value::Float(b)) => (*a as f64).partial_cmp(b),
        (Value::Float(a), Value::Int(b)) => a.partial_cmp(&(*b as f64)),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::Filesize(a), Value::Filesize(b)) => Some(a.bytes.cmp(&b.bytes)),
        (Value::Duration(a), Value::Duration(b)) => Some(a.nanos.cmp(&b.nanos)),
        _ => None,
    }
}

fn get_path<'a>(row: &'a Record, path: &Path) -> Option<&'a Value> {
    let mut cur = row.get(path.0.first()?)?;
    for part in path.0.iter().skip(1) {
        match cur {
            Value::Record(map) => cur = map.get(part)?,
            _ => return None,
        }
    }
    Some(cur)
}

fn literal_to_value(lit: &Literal) -> Value {
    match lit {
        Literal::Null => Value::Null,
        Literal::Bool(v) => Value::Bool(*v),
        Literal::Int(v) => Value::Int(*v),
        Literal::Float(v) => Value::Float(*v),
        Literal::String(v) => Value::String(v.clone()),
        Literal::Filesize(v) => Value::Filesize(crate::value::FilesizeValue { bytes: *v }),
        Literal::Duration(v) => Value::Duration(crate::value::DurationValue { nanos: *v }),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    String(String),
    Number(String),
    Op(String),
    And,
    Or,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.peek().copied() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '(' {
            out.push(Token::LParen);
            chars.next();
            continue;
        }
        if c == ')' {
            out.push(Token::RParen);
            chars.next();
            continue;
        }
        if c == '"' {
            chars.next();
            let mut s = String::new();
            for ch in chars.by_ref() {
                if ch == '"' {
                    break;
                }
                s.push(ch);
            }
            out.push(Token::String(s));
            continue;
        }
        if "=!<>".contains(c) {
            let mut op = String::new();
            op.push(c);
            chars.next();
            if let Some('=') = chars.peek().copied() {
                op.push('=');
                chars.next();
            }
            out.push(Token::Op(op));
            continue;
        }
        let mut word = String::new();
        while let Some(ch) = chars.peek().copied() {
            if ch.is_whitespace() || ch == '(' || ch == ')' || "=!<>\"".contains(ch) {
                break;
            }
            word.push(ch);
            chars.next();
        }
        match word.as_str() {
            "and" => out.push(Token::And),
            "or" => out.push(Token::Or),
            _ if word.chars().any(|ch| ch.is_ascii_digit())
                && word
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '.') =>
            {
                out.push(Token::Number(word))
            }
            _ => out.push(Token::Ident(word)),
        }
    }
    out
}

struct Parser {
    tokens: Vec<Token>,
    idx: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, idx: 0 }
    }
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.idx)
    }
    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.idx).cloned();
        self.idx += usize::from(t.is_some());
        t
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.next();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_atom()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.next();
            let right = self.parse_atom()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_atom(&mut self) -> Result<Expr> {
        if matches!(self.peek(), Some(Token::LParen)) {
            self.next();
            let expr = self.parse_or()?;
            if !matches!(self.next(), Some(Token::RParen)) {
                bail!("missing ')' in filter expression");
            }
            return Ok(expr);
        }
        let path = match self.next() {
            Some(Token::Ident(v)) => Path(v.split('.').map(|s| s.to_string()).collect()),
            _ => bail!("expected field path"),
        };
        let op = match self.next() {
            Some(Token::Op(v)) => match v.as_str() {
                "==" => CompareOp::Eq,
                "!=" => CompareOp::Ne,
                ">" => CompareOp::Gt,
                ">=" => CompareOp::Ge,
                "<" => CompareOp::Lt,
                "<=" => CompareOp::Le,
                _ => bail!("unsupported operator"),
            },
            Some(Token::Ident(v)) => match v.as_str() {
                "contains" => CompareOp::Contains,
                "starts-with" => CompareOp::StartsWith,
                "ends-with" => CompareOp::EndsWith,
                _ => bail!("expected operator"),
            },
            _ => bail!("expected operator"),
        };
        let right = parse_literal(self.next().ok_or_else(|| anyhow!("expected right value"))?)?;
        Ok(Expr::Compare {
            left: path,
            op,
            right,
        })
    }
}

fn parse_literal(t: Token) -> Result<Literal> {
    match t {
        Token::String(v) => Ok(Literal::String(v)),
        Token::Ident(v) if v == "true" => Ok(Literal::Bool(true)),
        Token::Ident(v) if v == "false" => Ok(Literal::Bool(false)),
        Token::Ident(v) if v == "null" => Ok(Literal::Null),
        Token::Ident(v) => Ok(Literal::String(v)),
        Token::Number(v) => parse_typed_literal(&v),
        _ => bail!("expected literal"),
    }
}

fn parse_typed_literal(s: &str) -> Result<Literal> {
    let lower = s.to_ascii_lowercase();
    if let Some(v) = parse_filesize(&lower) {
        return Ok(Literal::Filesize(v));
    }
    if let Some(v) = parse_duration(&lower) {
        return Ok(Literal::Duration(v));
    }
    if let Ok(i) = s.parse::<i64>() {
        return Ok(Literal::Int(i));
    }
    if let Ok(f) = s.parse::<f64>() {
        return Ok(Literal::Float(f));
    }
    Ok(Literal::String(s.to_string()))
}

fn parse_filesize(s: &str) -> Option<u64> {
    for (unit, mult) in [
        ("tb", 1024_u64.pow(4)),
        ("kb", 1024_u64),
        ("mb", 1024_u64.pow(2)),
        ("gb", 1024_u64.pow(3)),
        ("b", 1),
    ] {
        if let Some(num) = s.strip_suffix(unit) {
            return num.parse::<u64>().ok().map(|n| n.saturating_mul(mult));
        }
    }
    None
}

fn parse_duration(s: &str) -> Option<i64> {
    for (unit, mult) in [
        ("day", 86_400_000_000_000_i64),
        ("ms", 1_000_000_i64),
        ("sec", 1_000_000_000_i64),
        ("s", 1_000_000_000),
        ("min", 60_000_000_000),
        ("hr", 3_600_000_000_000),
    ] {
        if let Some(num) = s.strip_suffix(unit) {
            return num.parse::<i64>().ok().map(|n| n.saturating_mul(mult));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{FilesizeValue, Value};

    #[test]
    fn parse_and_eval() {
        let expr = parse_filter_expr("size > 1mb and name == \"main.rs\"").unwrap();
        let mut row = Record::new();
        row.insert("name".into(), Value::String("main.rs".into()));
        row.insert(
            "size".into(),
            Value::Filesize(FilesizeValue {
                bytes: 2 * 1024 * 1024,
            }),
        );
        assert!(eval_filter(&expr, &row));
    }

    #[test]
    fn parse_string_ops() {
        let expr = parse_filter_expr("name ends-with \".rs\"").unwrap();
        let mut row = Record::new();
        row.insert("name".into(), Value::String("main.rs".into()));
        assert!(eval_filter(&expr, &row));
    }
}
