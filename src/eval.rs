use std::collections::HashMap;

use crate::ast::*;

pub fn eval(stmts: &[Spanned<Stmt>]) -> Result<Vec<String>, SpannedError> {
    let mut env: HashMap<String, i64> = HashMap::new();
    let mut output: Vec<String> = Vec::new();
    eval_stmts(stmts, &mut env, &mut output)?;
    Ok(output)
}

fn eval_stmts(
    stmts: &[Spanned<Stmt>],
    env: &mut HashMap<String, i64>,
    output: &mut Vec<String>,
) -> Result<(), SpannedError> {
    for stmt in stmts {
        eval_stmt(stmt, env, output)?;
    }
    Ok(())
}

fn eval_stmt(
    stmt: &Spanned<Stmt>,
    env: &mut HashMap<String, i64>,
    output: &mut Vec<String>,
) -> Result<(), SpannedError> {
    match &stmt.node {
        Stmt::Assign {
            name, op, value, ..
        } => {
            let val = eval_expr(value, env)?;
            match op {
                AssignOp::Assign => {
                    env.insert(name.clone(), val);
                }
                AssignOp::AddAssign => {
                    let current = env.get(name).copied().unwrap_or(0);
                    env.insert(name.clone(), current + val);
                }
            }
        }
        Stmt::For {
            var, from, to, body, ..
        } => {
            let from_val = eval_expr(from, env)?;
            let to_val = eval_expr(to, env)?;
            for i in from_val..to_val {
                env.insert(var.clone(), i);
                eval_stmts(body, env, output)?;
            }
        }
        Stmt::If { cond, body, .. } => {
            let cond_val = eval_expr(cond, env)?;
            if cond_val != 0 {
                eval_stmts(body, env, output)?;
            }
        }
        Stmt::Print { value, .. } => {
            let val = eval_expr(value, env)?;
            output.push(val.to_string());
        }
    }
    Ok(())
}

fn eval_expr(expr: &Spanned<Expr>, env: &HashMap<String, i64>) -> Result<i64, SpannedError> {
    match &expr.node {
        Expr::Int(n) => Ok(*n),
        Expr::Var(name) => env.get(name).copied().ok_or_else(|| SpannedError {
            message: format!("undefined variable: {name}"),
            span: expr.span,
        }),
        Expr::BinOp {
            left, op, right, ..
        } => {
            let l = eval_expr(left, env)?;
            let r = eval_expr(right, env)?;
            Ok(match op {
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
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn run(source: &str) -> Vec<String> {
        let stmts = parser::parse(source).unwrap();
        eval(&stmts).unwrap()
    }

    fn run_err(source: &str) -> String {
        let stmts = parser::parse(source).unwrap();
        eval(&stmts).unwrap_err().message
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
}
