use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub args: Vec<String>,
    pub redirects: Vec<Redirect>,
    pub background: bool,
    pub force_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Pipeline {
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Redirect {
    Stdout(String),
    StdoutAppend(String),
    Stderr(String),
    Stdin(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TypeExpr {
    Any,
    Int,
    Bool,
    String,
    List(Box<TypeExpr>),
    Iterator(Box<TypeExpr>),
    Function {
        params: Vec<TypeExpr>,
        return_type: Box<TypeExpr>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CellPathSegment {
    Field(String),
    Index(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assignment {
    pub name: String,
    pub cell_path: Vec<CellPathSegment>,
    pub is_constant: bool,
    pub is_exported: bool,
    pub value: Expression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeExpr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Expression {
    Null,
    StringLiteral(String),
    Integer(i64),
    Float(String),
    Bool(bool),
    Identifier(String),
    Variable {
        name: String,
        cell_path: Vec<CellPathSegment>,
    },
    List(Vec<Expression>),
    Record(Vec<(String, Expression)>),
    Range {
        start: i64,
        end: i64,
    },
    Binary {
        left: Box<Expression>,
        op: BinaryOp,
        right: Box<Expression>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expression>,
    },
    Lambda {
        params: Vec<Param>,
        body: Vec<Statement>,
    },
    Closure {
        params: Vec<Param>,
        body: Vec<Statement>,
        captures: BTreeMap<String, Expression>,
    },
    MemberAccess {
        object: Box<Expression>,
        field: String,
    },
    Pipeline(Pipeline),
    Call {
        name: String,
        args: Vec<Expression>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Statement {
    Command(Command),
    Pipeline(Pipeline),
    Assignment(Assignment),
    Let {
        name: String,
        ty: Option<TypeExpr>,
        value: Expression,
    },
    If {
        condition: Expression,
        then_branch: Vec<Statement>,
        else_branch: Vec<Statement>,
    },
    For {
        variable: String,
        iterable: Expression,
        body: Vec<Statement>,
    },
    Match {
        expression: Expression,
        arms: Vec<(Expression, Vec<Statement>)>,
    },
    Function {
        name: String,
        params: Vec<Param>,
        return_type: Option<TypeExpr>,
        is_exported: bool,
        body: Vec<Statement>,
    },
    Module {
        name: String,
        body: Vec<Statement>,
    },
    Import {
        module: String,
        alias: Option<String>,
    },
    Test {
        name: String,
        body: Vec<Statement>,
    },
    Return(Option<Expression>),
    Break,
    Continue,
    Expr(Expression),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Script {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Scope {
    pub vars: BTreeMap<String, Expression>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_script_is_valid() {
        let script = Script::default();
        assert!(script.statements.is_empty());
    }
}
