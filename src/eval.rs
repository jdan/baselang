use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::ast::*;

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Array(Rc<RefCell<Vec<i64>>>),
}

impl Value {
    fn as_int(&self, span: Span) -> Result<i64, SpannedError> {
        match self {
            Value::Int(n) => Ok(*n),
            _ => Err(SpannedError {
                message: "expected integer, got array".to_string(),
                span,
            }),
        }
    }
}

struct FnDef {
    params: Vec<String>,
    body: Vec<Spanned<Stmt>>,
}

enum Flow {
    Next,
    Return(Value),
    Break,
}

type Env = HashMap<String, Value>;
type Fns = HashMap<String, FnDef>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionMetric {
    pub count: u64,
    pub total_nanos: u128,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionMetrics {
    pub by_offset: BTreeMap<usize, ExecutionMetric>,
}

impl ExecutionMetrics {
    fn record(&mut self, span: Span, elapsed: std::time::Duration) {
        let entry = self.by_offset.entry(span.start).or_default();
        entry.count += 1;
        entry.total_nanos += elapsed.as_nanos();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalArtifacts {
    pub output: Vec<String>,
    pub metrics: ExecutionMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalFailure {
    pub error: SpannedError,
    pub metrics: ExecutionMetrics,
}

pub fn eval(stmts: &[Spanned<Stmt>]) -> Result<Vec<String>, SpannedError> {
    eval_with_metrics(stmts)
        .map(|artifacts| artifacts.output)
        .map_err(|failure| failure.error)
}

pub fn eval_with_metrics(stmts: &[Spanned<Stmt>]) -> Result<EvalArtifacts, EvalFailure> {
    let mut env = Env::new();
    let mut fns = Fns::new();
    let mut output = Vec::new();
    let mut metrics = ExecutionMetrics::default();

    let flow = match eval_stmts(stmts, &mut env, &mut fns, &mut output, &mut metrics) {
        Ok(flow) => flow,
        Err(error) => {
            return Err(EvalFailure { error, metrics });
        }
    };

    match flow {
        Flow::Next => Ok(EvalArtifacts { output, metrics }),
        Flow::Return(_) => Err(EvalFailure {
            error: SpannedError {
                message: "return outside of function".to_string(),
                span: Span { start: 0, end: 0 },
            },
            metrics,
        }),
        Flow::Break => Err(EvalFailure {
            error: SpannedError {
                message: "break outside of loop".to_string(),
                span: Span { start: 0, end: 0 },
            },
            metrics,
        }),
    }
}

fn eval_stmts(
    stmts: &[Spanned<Stmt>],
    env: &mut Env,
    fns: &mut Fns,
    output: &mut Vec<String>,
    metrics: &mut ExecutionMetrics,
) -> Result<Flow, SpannedError> {
    for stmt in stmts {
        match eval_stmt(stmt, env, fns, output, metrics)? {
            Flow::Next => {}
            flow => return Ok(flow),
        }
    }
    Ok(Flow::Next)
}

fn eval_stmt(
    stmt: &Spanned<Stmt>,
    env: &mut Env,
    fns: &mut Fns,
    output: &mut Vec<String>,
    metrics: &mut ExecutionMetrics,
) -> Result<Flow, SpannedError> {
    let started = Instant::now();
    let result = eval_stmt_inner(stmt, env, fns, output, metrics);
    metrics.record(stmt.span, started.elapsed());
    result
}

fn eval_stmt_inner(
    stmt: &Spanned<Stmt>,
    env: &mut Env,
    fns: &mut Fns,
    output: &mut Vec<String>,
    metrics: &mut ExecutionMetrics,
) -> Result<Flow, SpannedError> {
    match &stmt.node {
        Stmt::Assign {
            name, op, value, ..
        } => {
            let val = eval_expr(value, env, fns, output, metrics)?;
            match op {
                AssignOp::Assign => {
                    env.insert(name.clone(), val);
                }
                AssignOp::AddAssign => {
                    let current = env
                        .get(name)
                        .map(|v| v.as_int(value.span))
                        .transpose()?
                        .unwrap_or(0);
                    env.insert(name.clone(), Value::Int(current + val.as_int(value.span)?));
                }
            }
            Ok(Flow::Next)
        }
        Stmt::For {
            var,
            from,
            to,
            body,
            ..
        } => {
            let from_val = eval_expr(from, env, fns, output, metrics)?.as_int(from.span)?;
            let to_val = eval_expr(to, env, fns, output, metrics)?.as_int(to.span)?;
            for i in from_val..to_val {
                env.insert(var.clone(), Value::Int(i));
                match eval_stmts(body, env, fns, output, metrics)? {
                    Flow::Next => {}
                    Flow::Break => break,
                    Flow::Return(v) => return Ok(Flow::Return(v)),
                }
            }
            Ok(Flow::Next)
        }
        Stmt::While { cond, body, .. } => loop {
            let cond_val = eval_expr(cond, env, fns, output, metrics)?.as_int(cond.span)?;
            if cond_val == 0 {
                return Ok(Flow::Next);
            }
            match eval_stmts(body, env, fns, output, metrics)? {
                Flow::Next => {}
                Flow::Break => return Ok(Flow::Next),
                Flow::Return(v) => return Ok(Flow::Return(v)),
            }
        },
        Stmt::If { cond, body, .. } => {
            let cond_val = eval_expr(cond, env, fns, output, metrics)?.as_int(cond.span)?;
            if cond_val != 0 {
                return eval_stmts(body, env, fns, output, metrics);
            }
            Ok(Flow::Next)
        }
        Stmt::Print { value, .. } => {
            let val = eval_expr(value, env, fns, output, metrics)?;
            output.push(match val {
                Value::Int(n) => n.to_string(),
                Value::Array(a) => format!("[array; len={}]", a.borrow().len()),
            });
            Ok(Flow::Next)
        }
        Stmt::FnDef {
            name, params, body, ..
        } => {
            fns.insert(
                name.clone(),
                FnDef {
                    params: params.iter().map(|(n, _)| n.clone()).collect(),
                    body: body.clone(),
                },
            );
            Ok(Flow::Next)
        }
        Stmt::Return { value, .. } => {
            let val = eval_expr(value, env, fns, output, metrics)?;
            Ok(Flow::Return(val))
        }
        Stmt::Break { .. } => Ok(Flow::Break),
        Stmt::ExprStmt { value, .. } => {
            eval_expr(value, env, fns, output, metrics)?;
            Ok(Flow::Next)
        }
        Stmt::IndexAssign {
            name,
            index,
            op,
            value,
            ..
        } => {
            let idx = eval_expr(index, env, fns, output, metrics)?.as_int(index.span)?;
            let val = eval_expr(value, env, fns, output, metrics)?.as_int(value.span)?;
            let arr = env.get(name).ok_or_else(|| SpannedError {
                message: format!("undefined variable: {name}"),
                span: stmt.span,
            })?;
            if let Value::Array(arr) = arr {
                let mut arr = arr.borrow_mut();
                let idx = idx as usize;
                if idx >= arr.len() {
                    return Err(SpannedError {
                        message: format!("index {} out of bounds (len {})", idx, arr.len()),
                        span: index.span,
                    });
                }
                match op {
                    AssignOp::Assign => arr[idx] = val,
                    AssignOp::AddAssign => arr[idx] += val,
                }
            } else {
                return Err(SpannedError {
                    message: "cannot index into non-array".to_string(),
                    span: stmt.span,
                });
            }
            Ok(Flow::Next)
        }
    }
}

fn eval_expr(
    expr: &Spanned<Expr>,
    env: &mut Env,
    fns: &mut Fns,
    output: &mut Vec<String>,
    metrics: &mut ExecutionMetrics,
) -> Result<Value, SpannedError> {
    match &expr.node {
        Expr::Int(n) => Ok(Value::Int(*n)),
        Expr::Var(name) => env.get(name).cloned().ok_or_else(|| SpannedError {
            message: format!("undefined variable: {name}"),
            span: expr.span,
        }),
        Expr::BinOp {
            left, op, right, ..
        } => {
            let l = eval_expr(left, env, fns, output, metrics)?.as_int(left.span)?;
            let r = eval_expr(right, env, fns, output, metrics)?.as_int(right.span)?;
            Ok(Value::Int(match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => {
                    if r == 0 {
                        return Err(SpannedError {
                            message: "division by zero".to_string(),
                            span: expr.span,
                        });
                    }
                    l / r
                }
                BinOp::Mod => {
                    if r == 0 {
                        return Err(SpannedError {
                            message: "division by zero".to_string(),
                            span: expr.span,
                        });
                    }
                    l % r
                }
                BinOp::Eq => (l == r) as i64,
                BinOp::Ne => (l != r) as i64,
                BinOp::Lt => (l < r) as i64,
                BinOp::Gt => (l > r) as i64,
                BinOp::Le => (l <= r) as i64,
                BinOp::Ge => (l >= r) as i64,
                BinOp::And => ((l != 0) && (r != 0)) as i64,
                BinOp::Or => ((l != 0) || (r != 0)) as i64,
            }))
        }
        Expr::Call { name, args, .. } => {
            // Evaluate arguments first
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(eval_expr(arg, env, fns, output, metrics)?);
            }

            // Built-in: array(size, default)
            if name == "array" {
                if arg_vals.len() != 2 {
                    return Err(SpannedError {
                        message: "array() takes 2 arguments".to_string(),
                        span: expr.span,
                    });
                }
                let size = arg_vals[0].as_int(args[0].span)?;
                let default = arg_vals[1].as_int(args[1].span)?;
                return Ok(Value::Array(Rc::new(RefCell::new(vec![
                    default;
                    size as usize
                ]))));
            }

            // Built-in: len(arr)
            if name == "len" {
                if arg_vals.len() != 1 {
                    return Err(SpannedError {
                        message: "len() takes 1 argument".to_string(),
                        span: expr.span,
                    });
                }
                return match &arg_vals[0] {
                    Value::Array(arr) => Ok(Value::Int(arr.borrow().len() as i64)),
                    _ => Err(SpannedError {
                        message: "len() requires array".to_string(),
                        span: args[0].span,
                    }),
                };
            }

            // User-defined function
            let (params, body) = {
                let fndef = fns.get(name.as_str()).ok_or_else(|| SpannedError {
                    message: format!("undefined function: {name}"),
                    span: expr.span,
                })?;
                (fndef.params.clone(), fndef.body.clone())
            };
            if arg_vals.len() != params.len() {
                return Err(SpannedError {
                    message: format!(
                        "{name}() takes {} arguments, got {}",
                        params.len(),
                        arg_vals.len()
                    ),
                    span: expr.span,
                });
            }
            let mut local_env = Env::new();
            for (param, val) in params.iter().zip(arg_vals) {
                local_env.insert(param.clone(), val);
            }
            match eval_stmts(&body, &mut local_env, fns, output, metrics)? {
                Flow::Return(v) => Ok(v),
                Flow::Break => Err(SpannedError {
                    message: "break outside of loop".to_string(),
                    span: expr.span,
                }),
                Flow::Next => Ok(Value::Int(0)),
            }
        }
        Expr::Index { name, index, .. } => {
            let idx = eval_expr(index, env, fns, output, metrics)?.as_int(index.span)?;
            let arr = env.get(name).ok_or_else(|| SpannedError {
                message: format!("undefined variable: {name}"),
                span: expr.span,
            })?;
            match arr {
                Value::Array(arr) => {
                    let arr = arr.borrow();
                    let idx = idx as usize;
                    if idx >= arr.len() {
                        return Err(SpannedError {
                            message: format!("index {} out of bounds (len {})", idx, arr.len()),
                            span: index.span,
                        });
                    }
                    Ok(Value::Int(arr[idx]))
                }
                _ => Err(SpannedError {
                    message: "cannot index into non-array".to_string(),
                    span: expr.span,
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observe;
    use crate::parser;

    fn run(source: &str) -> Vec<String> {
        let stmts = parser::parse(source).unwrap();
        eval(&stmts).unwrap()
    }

    fn run_err(source: &str) -> String {
        let stmts = parser::parse(source).unwrap();
        eval(&stmts).unwrap_err().message
    }

    fn run_with_metrics(source: &str) -> EvalArtifacts {
        let stmts = parser::parse(source).unwrap();
        eval_with_metrics(&stmts).unwrap()
    }

    #[test]
    fn eval_simple_print() {
        assert_eq!(run("x = 42\nprint x"), vec!["42"]);
    }

    #[test]
    fn eval_arithmetic() {
        assert_eq!(run("x = 2 + 3 * 4\nprint x"), vec!["14"]);
    }

    #[test]
    fn eval_comparison() {
        assert_eq!(run("print 5 == 5"), vec!["1"]);
        assert_eq!(run("print 5 != 3"), vec!["1"]);
        assert_eq!(run("print 3 < 5"), vec!["1"]);
        assert_eq!(run("print 5 > 3"), vec!["1"]);
    }

    #[test]
    fn eval_boolean_ops() {
        assert_eq!(run("print 1 and 1"), vec!["1"]);
        assert_eq!(run("print 1 and 0"), vec!["0"]);
        assert_eq!(run("print 0 or 1"), vec!["1"]);
        assert_eq!(run("print 0 or 0"), vec!["0"]);
    }

    #[test]
    fn eval_for_loop() {
        assert_eq!(
            run("sum = 0\nfor i from 0 to 5\n  sum += i\nend\nprint sum"),
            vec!["10"]
        );
    }

    #[test]
    fn eval_while_loop() {
        assert_eq!(
            run("x = 10\nwhile x > 0\n  x = x - 1\nend\nprint x"),
            vec!["0"]
        );
    }

    #[test]
    fn eval_break_in_while() {
        assert_eq!(
            run("x = 0\nwhile 1\n  if x == 5\n    break\n  end\n  x += 1\nend\nprint x"),
            vec!["5"]
        );
    }

    #[test]
    fn eval_break_in_for() {
        assert_eq!(
            run(
                "s = 0\nfor i from 0 to 100\n  if i == 3\n    break\n  end\n  s += i\nend\nprint s"
            ),
            vec!["3"]
        );
    }

    #[test]
    fn eval_function_call() {
        assert_eq!(
            run("fn add(a, b)\n  return a + b\nend\nprint add(3, 4)"),
            vec!["7"]
        );
    }

    #[test]
    fn eval_function_with_while() {
        let src = "fn gcd(a, b)\n  while b != 0\n    temp = a % b\n    a = b\n    b = temp\n  end\n  return a\nend\nprint gcd(12, 8)";
        assert_eq!(run(src), vec!["4"]);
    }

    #[test]
    fn eval_recursive_function() {
        // factorial via recursion
        let src = "fn fact(n)\n  if n <= 1\n    return 1\n  end\n  return n * fact(n - 1)\nend\nprint fact(5)";
        assert_eq!(run(src), vec!["120"]);
    }

    #[test]
    fn eval_array_basic() {
        assert_eq!(
            run("a = array(3, 0)\na[0] = 10\na[1] = 20\na[2] = 30\nprint a[0] + a[1] + a[2]"),
            vec!["60"]
        );
    }

    #[test]
    fn eval_array_len() {
        assert_eq!(run("a = array(5, 0)\nprint len(a)"), vec!["5"]);
    }

    #[test]
    fn eval_array_passed_to_function() {
        let src = "fn fill(arr, n)\n  for i from 0 to n\n    arr[i] = i * i\n  end\n  return 0\nend\na = array(5, 0)\nfill(a, 5)\nprint a[3]";
        assert_eq!(run(src), vec!["9"]);
    }

    #[test]
    fn eval_if_true() {
        assert_eq!(run("x = 1\nif x == 1\n  print 42\nend"), vec!["42"]);
    }

    #[test]
    fn eval_if_false() {
        let output = run("x = 0\nif x == 1\n  print 42\nend");
        assert!(output.is_empty());
    }

    #[test]
    fn eval_add_assign() {
        assert_eq!(run("x = 10\nx += 5\nprint x"), vec!["15"]);
    }

    #[test]
    fn eval_nested_loops() {
        assert_eq!(
            run("sum = 0\nfor i from 0 to 3\n  for j from 0 to 3\n    sum += 1\n  end\nend\nprint sum"),
            vec!["9"]
        );
    }

    #[test]
    fn eval_euler_001() {
        let src = "total = 0\n\nfor i from 0 to 1000\n  if i % 3 == 0 or i % 5 == 0\n    total += i\n  end\nend\n\nprint total";
        assert_eq!(run(src), vec!["233168"]);
    }

    #[test]
    fn eval_collects_line_metrics() {
        let src = "fn square_slow(n)\n  total = 0\n  for j from 0 to n\n    total += n\n  end\n  return total\nend\n\nsum = 0\nfor i from 0 to 4\n  sum += square_slow(i)\nend\nprint sum";
        let artifacts = run_with_metrics(src);
        let report = observe::build_report(src, &artifacts.metrics);
        let counts = report
            .lines
            .iter()
            .map(|entry| (entry.line, entry.count))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(counts.get(&2), Some(&4));
        assert_eq!(counts.get(&3), Some(&4));
        assert_eq!(counts.get(&4), Some(&6));
        assert_eq!(counts.get(&6), Some(&4));
        assert_eq!(counts.get(&10), Some(&1));
        assert_eq!(counts.get(&11), Some(&4));
    }

    #[test]
    fn eval_undefined_variable() {
        assert_eq!(run_err("print x"), "undefined variable: x");
    }

    #[test]
    fn eval_division_by_zero() {
        assert_eq!(run_err("print 1 / 0"), "division by zero");
    }

    #[test]
    fn eval_modulo_by_zero() {
        assert_eq!(run_err("print 1 % 0"), "division by zero");
    }

    #[test]
    fn eval_index_out_of_bounds() {
        let msg = run_err("a = array(3, 0)\nprint a[5]");
        assert!(msg.contains("out of bounds"));
    }
}
