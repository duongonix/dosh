use dosh_value::{Expr, Record, Value, eval_filter};

#[derive(Debug, Clone, PartialEq)]
pub enum RowOp {
    Select(Vec<String>),
    Reject(Vec<String>),
    Filter(Expr),
    Skip(usize),
    Take(usize),
    Reverse,
    SortBy(String),
    MapField(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowStream {
    base: Vec<Record>,
    ops: Vec<RowOp>,
}

impl RowStream {
    pub fn new(base: Vec<Record>) -> Self {
        Self {
            base,
            ops: Vec::new(),
        }
    }

    pub fn push_op(&mut self, op: RowOp) {
        self.ops.push(op);
    }

    pub fn materialize_rows(&self) -> Vec<Record> {
        let mut rows = self.base.clone();
        for op in &self.ops {
            match op {
                RowOp::Select(cols) => {
                    rows = rows
                        .into_iter()
                        .map(|mut r| {
                            r.retain(|k, _| cols.contains(k));
                            r
                        })
                        .collect();
                }
                RowOp::Reject(cols) => {
                    rows = rows
                        .into_iter()
                        .map(|mut r| {
                            for c in cols {
                                r.shift_remove(c);
                            }
                            r
                        })
                        .collect();
                }
                RowOp::Filter(expr) => {
                    rows.retain(|r| eval_filter(expr, r));
                }
                RowOp::Skip(n) => {
                    rows = rows.into_iter().skip(*n).collect();
                }
                RowOp::Take(n) => {
                    rows = rows.into_iter().take(*n).collect();
                }
                RowOp::Reverse => rows.reverse(),
                RowOp::SortBy(field) => {
                    rows.sort_by(|a, b| {
                        let av = Value::Record(a.clone()).get_path(field).cloned();
                        let bv = Value::Record(b.clone()).get_path(field).cloned();
                        cmp_value_opt(av.as_ref(), bv.as_ref())
                    });
                }
                RowOp::MapField(_) => {}
            }
        }
        rows
    }

    pub fn materialize_value(&self) -> Value {
        Value::Table(dosh_value::Table::new(self.materialize_rows()))
    }

    pub fn materialize_mapped_values(&self) -> Option<Vec<Value>> {
        let mut field = None;
        for op in &self.ops {
            if let RowOp::MapField(f) = op {
                field = Some(f.clone());
            }
        }
        let field = field?;
        let rows = self.materialize_rows();
        Some(
            rows.into_iter()
                .map(|r| {
                    Value::Record(r)
                        .get_path(&field)
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .collect(),
        )
    }
}

fn cmp_value_opt(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(Value::Int(x)), Some(Value::Int(y))) => x.cmp(y),
        (Some(Value::Float(x)), Some(Value::Float(y))) => {
            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
        }
        (Some(Value::String(x)), Some(Value::String(y))) => x.cmp(y),
        (Some(Value::Filesize(x)), Some(Value::Filesize(y))) => x.bytes.cmp(&y.bytes),
        (Some(Value::Duration(x)), Some(Value::Duration(y))) => x.nanos.cmp(&y.nanos),
        _ => std::cmp::Ordering::Equal,
    }
}
