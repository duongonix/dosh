use dosh_ast::{Expression, Param, Statement, TypeExpr};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub(crate) struct FunctionDef {
    pub(crate) params: Vec<Param>,
    pub(crate) return_type: Option<TypeExpr>,
    pub(crate) body: Vec<Statement>,
    pub(crate) captures: BTreeMap<String, Expression>,
}

#[derive(Debug, Default)]
pub struct RuntimeState {
    pub(crate) scopes: Vec<BTreeMap<String, Expression>>,
    pub(crate) functions: BTreeMap<String, FunctionDef>,
    pub(crate) modules: BTreeMap<String, Vec<Statement>>,
    pub(crate) module_exports: BTreeMap<String, BTreeMap<String, Expression>>,
    pub(crate) imported_modules: BTreeSet<String>,
    pub(crate) import_stack: Vec<String>,
    pub(crate) constants: BTreeSet<String>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
            functions: BTreeMap::new(),
            modules: BTreeMap::new(),
            module_exports: BTreeMap::new(),
            imported_modules: BTreeSet::new(),
            import_stack: Vec::new(),
            constants: BTreeSet::new(),
        }
    }

    pub(crate) fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub(crate) fn set_var(&mut self, name: String, value: Expression) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    pub(crate) fn assign_var(
        &mut self,
        name: String,
        cell_path: &[dosh_ast::CellPathSegment],
        value: Expression,
        is_constant: bool,
    ) -> anyhow::Result<()> {
        if self.constants.contains(&name) && !cell_path.is_empty() {
            anyhow::bail!("Cannot reassign constant variable: ${name}");
        }
        if self.constants.contains(&name) && cell_path.is_empty() {
            anyhow::bail!("Cannot reassign constant variable: ${name}");
        }

        for scope in self.scopes.iter_mut().rev() {
            if let std::collections::btree_map::Entry::Occupied(mut entry) =
                scope.entry(name.clone())
            {
                if cell_path.is_empty() {
                    entry.insert(value);
                } else {
                    let mut root = entry.get().clone();
                    assign_cell_path(&mut root, cell_path, value)?;
                    entry.insert(root);
                }
                return Ok(());
            }
        }

        if cell_path.is_empty() {
            self.set_var(name.clone(), value);
        } else {
            anyhow::bail!("Variable not found: ${name}");
        }
        if is_constant {
            self.constants.insert(name);
        }
        Ok(())
    }

    pub(crate) fn get_var(&self, name: &str) -> Option<Expression> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    pub(crate) fn visible_vars(&self) -> BTreeMap<String, Expression> {
        let mut out = BTreeMap::new();
        for scope in &self.scopes {
            for (k, v) in scope {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }
}

fn assign_cell_path(
    root: &mut Expression,
    path: &[dosh_ast::CellPathSegment],
    value: Expression,
) -> anyhow::Result<()> {
    if path.is_empty() {
        *root = value;
        return Ok(());
    }
    match (&mut *root, &path[0]) {
        (Expression::Record(fields), dosh_ast::CellPathSegment::Field(name)) => {
            if path.len() == 1 {
                if let Some((_, v)) = fields.iter_mut().find(|(k, _)| k == name) {
                    *v = value;
                    return Ok(());
                }
                fields.push((name.clone(), value));
                return Ok(());
            }
            if let Some((_, v)) = fields.iter_mut().find(|(k, _)| k == name) {
                return assign_cell_path(v, &path[1..], value);
            }
            anyhow::bail!("Invalid cell path")
        }
        (Expression::List(items), dosh_ast::CellPathSegment::Index(i)) => {
            if *i >= items.len() {
                anyhow::bail!("Invalid cell path: index out of bounds");
            }
            if path.len() == 1 {
                items[*i] = value;
                return Ok(());
            }
            assign_cell_path(&mut items[*i], &path[1..], value)
        }
        _ => anyhow::bail!("Invalid cell path"),
    }
}

pub(crate) fn evaluate_truthy(expr: &Expression, state: &RuntimeState) -> bool {
    let resolved = match expr {
        Expression::Identifier(name) => state.get_var(name).unwrap_or_else(|| expr.clone()),
        _ => expr.clone(),
    };

    match resolved {
        Expression::Null => false,
        Expression::Bool(v) => v,
        Expression::Integer(v) => v != 0,
        Expression::Float(v) => v.parse::<f64>().ok().is_some_and(|x| x != 0.0),
        Expression::StringLiteral(v) => !v.is_empty(),
        Expression::Identifier(_) => false,
        Expression::Variable { .. } => false,
        Expression::Call { .. } => false,
        Expression::List(v) => !v.is_empty(),
        Expression::Record(v) => !v.is_empty(),
        Expression::Range { start, end } => start < end,
        Expression::Binary { .. } => false,
        Expression::Unary { .. } => false,
        Expression::Lambda { .. } => true,
        Expression::Closure { .. } => true,
        Expression::Pipeline(_) => false,
        Expression::MemberAccess { .. } => false,
    }
}

pub(crate) fn pattern_matches(
    pattern: &Expression,
    value: &Expression,
    state: &RuntimeState,
) -> bool {
    match pattern {
        Expression::Identifier(name) if name == "_" => true,
        Expression::Identifier(name) => {
            let resolved = state
                .get_var(name)
                .unwrap_or_else(|| Expression::Identifier(name.clone()));
            resolved == *value
        }
        _ => {
            let resolved_pattern = match pattern {
                Expression::Identifier(name) => state
                    .get_var(name)
                    .unwrap_or_else(|| Expression::Identifier(name.clone())),
                _ => pattern.clone(),
            };
            resolved_pattern == *value
        }
    }
}
