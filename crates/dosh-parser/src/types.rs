use anyhow::Result;
use dosh_ast::TypeExpr;

pub fn parse_type_expr(input: &str) -> Result<TypeExpr> {
    let text = input.trim();
    if text.is_empty() {
        anyhow::bail!("type annotation cannot be empty");
    }

    if let Some(inner) = text.strip_prefix("list<").and_then(|s| s.strip_suffix('>')) {
        return Ok(TypeExpr::List(Box::new(parse_type_expr(inner)?)));
    }
    if let Some(inner) = text.strip_prefix("iter<").and_then(|s| s.strip_suffix('>')) {
        return Ok(TypeExpr::Iterator(Box::new(parse_type_expr(inner)?)));
    }

    let ty = match text {
        "any" => TypeExpr::Any,
        "int" => TypeExpr::Int,
        "bool" => TypeExpr::Bool,
        "string" => TypeExpr::String,
        _ => anyhow::bail!("unknown type `{text}`"),
    };
    Ok(ty)
}
