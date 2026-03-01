use crate::ast::*;
use crate::lexer::{self, Token, TokenKind};

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn advance_span(&mut self) -> Span {
        let span = self.tokens[self.pos].span;
        self.pos += 1;
        span
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn expect(&mut self, kind: &TokenKind, msg: &str) -> Result<Span, SpannedError> {
        if std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(kind) {
            Ok(self.advance_span())
        } else {
            Err(SpannedError {
                message: msg.to_string(),
                span: self.peek_span(),
            })
        }
    }

    fn expect_ident(&mut self) -> Result<(String, Span), SpannedError> {
        if let TokenKind::Ident(name) = self.peek_kind() {
            let name = name.clone();
            let span = self.advance_span();
            Ok((name, span))
        } else {
            Err(SpannedError {
                message: "expected identifier".to_string(),
                span: self.peek_span(),
            })
        }
    }

    fn expect_newline(&mut self) -> Result<(), SpannedError> {
        if matches!(self.peek_kind(), TokenKind::Newline) {
            self.pos += 1;
            Ok(())
        } else {
            Err(SpannedError {
                message: "expected newline".to_string(),
                span: self.peek_span(),
            })
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Newline) {
            self.pos += 1;
        }
    }

    fn parse_program(&mut self) -> Result<Vec<Spanned<Stmt>>, SpannedError> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !self.at_eof() {
            stmts.push(self.parse_stmt()?);
            if !self.at_eof() {
                self.expect_newline()?;
                self.skip_newlines();
            }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        match self.peek_kind() {
            TokenKind::For => self.parse_for(),
            TokenKind::While => self.parse_while(),
            TokenKind::If => self.parse_if(),
            TokenKind::Print => self.parse_print(),
            TokenKind::Fn => self.parse_fn_def(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Ident(_) => self.parse_ident_stmt(),
            _ => Err(SpannedError {
                message: "expected statement".to_string(),
                span: self.peek_span(),
            }),
        }
    }

    fn parse_stmt_list(&mut self) -> Result<Vec<Spanned<Stmt>>, SpannedError> {
        let mut stmts = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek_kind(), TokenKind::End | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
            self.expect_newline()?;
            self.skip_newlines();
        }
        Ok(stmts)
    }

    fn parse_for(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let for_span = self.advance_span();
        let (var, var_span) = self.expect_ident()?;
        let from_span = self.expect(&TokenKind::From, "expected 'from'")?;
        let from = self.parse_expr()?;
        let to_span = self.expect(&TokenKind::To, "expected 'to'")?;
        let to = self.parse_expr()?;
        self.expect_newline()?;
        let body = self.parse_stmt_list()?;
        let end_span = self.expect(&TokenKind::End, "expected 'end'")?;
        Ok(Spanned {
            span: Span {
                start: for_span.start,
                end: end_span.end,
            },
            node: Stmt::For {
                for_span,
                var,
                var_span,
                from_span,
                from,
                to_span,
                to,
                body,
                end_span,
            },
        })
    }

    fn parse_while(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let while_span = self.advance_span();
        let cond = self.parse_expr()?;
        self.expect_newline()?;
        let body = self.parse_stmt_list()?;
        let end_span = self.expect(&TokenKind::End, "expected 'end'")?;
        Ok(Spanned {
            span: Span {
                start: while_span.start,
                end: end_span.end,
            },
            node: Stmt::While {
                while_span,
                cond,
                body,
                end_span,
            },
        })
    }

    fn parse_if(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let if_span = self.advance_span();
        let cond = self.parse_expr()?;
        self.expect_newline()?;
        let body = self.parse_stmt_list()?;
        let end_span = self.expect(&TokenKind::End, "expected 'end'")?;
        Ok(Spanned {
            span: Span {
                start: if_span.start,
                end: end_span.end,
            },
            node: Stmt::If {
                if_span,
                cond,
                body,
                end_span,
            },
        })
    }

    fn parse_print(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let print_span = self.advance_span();
        let value = self.parse_expr()?;
        Ok(Spanned {
            span: Span {
                start: print_span.start,
                end: value.span.end,
            },
            node: Stmt::Print { print_span, value },
        })
    }

    fn parse_fn_def(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let fn_span = self.advance_span();
        let (name, name_span) = self.expect_ident()?;
        self.expect(&TokenKind::LParen, "expected '('")?;
        let mut params = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RParen) {
            let (p, ps) = self.expect_ident()?;
            params.push((p, ps));
            while matches!(self.peek_kind(), TokenKind::Comma) {
                self.advance_span();
                let (p, ps) = self.expect_ident()?;
                params.push((p, ps));
            }
        }
        self.expect(&TokenKind::RParen, "expected ')'")?;
        self.expect_newline()?;
        let body = self.parse_stmt_list()?;
        let end_span = self.expect(&TokenKind::End, "expected 'end'")?;
        Ok(Spanned {
            span: Span {
                start: fn_span.start,
                end: end_span.end,
            },
            node: Stmt::FnDef {
                fn_span,
                name,
                name_span,
                params,
                body,
                end_span,
            },
        })
    }

    fn parse_return(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let return_span = self.advance_span();
        let value = self.parse_expr()?;
        Ok(Spanned {
            span: Span {
                start: return_span.start,
                end: value.span.end,
            },
            node: Stmt::Return {
                return_span,
                value,
            },
        })
    }

    fn parse_break(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let break_span = self.advance_span();
        Ok(Spanned {
            span: break_span,
            node: Stmt::Break { break_span },
        })
    }

    fn parse_ident_stmt(&mut self) -> Result<Spanned<Stmt>, SpannedError> {
        let (name, name_span) = self.expect_ident()?;
        if matches!(self.peek_kind(), TokenKind::LBracket) {
            self.parse_index_assign(name, name_span)
        } else if matches!(self.peek_kind(), TokenKind::LParen) {
            // Function call as statement
            self.advance_span(); // consume (
            let mut args = Vec::new();
            if !matches!(self.peek_kind(), TokenKind::RParen) {
                args.push(self.parse_expr()?);
                while matches!(self.peek_kind(), TokenKind::Comma) {
                    self.advance_span();
                    args.push(self.parse_expr()?);
                }
            }
            let rp = self.expect(&TokenKind::RParen, "expected ')'")?;
            let span = Span {
                start: name_span.start,
                end: rp.end,
            };
            Ok(Spanned {
                span,
                node: Stmt::ExprStmt {
                    value: Spanned {
                        span,
                        node: Expr::Call {
                            name,
                            name_span,
                            args,
                        },
                    },
                },
            })
        } else {
            self.parse_assign_rest(name, name_span)
        }
    }

    fn parse_assign_rest(
        &mut self,
        name: String,
        name_span: Span,
    ) -> Result<Spanned<Stmt>, SpannedError> {
        let (op, op_span) = if matches!(self.peek_kind(), TokenKind::Eq) {
            (AssignOp::Assign, self.advance_span())
        } else if matches!(self.peek_kind(), TokenKind::PlusEq) {
            (AssignOp::AddAssign, self.advance_span())
        } else {
            return Err(SpannedError {
                message: "expected '=' or '+='".to_string(),
                span: self.peek_span(),
            });
        };
        let value = self.parse_expr()?;
        Ok(Spanned {
            span: Span {
                start: name_span.start,
                end: value.span.end,
            },
            node: Stmt::Assign {
                name,
                name_span,
                op,
                op_span,
                value,
            },
        })
    }

    fn parse_index_assign(
        &mut self,
        name: String,
        name_span: Span,
    ) -> Result<Spanned<Stmt>, SpannedError> {
        self.advance_span(); // consume [
        let index = self.parse_expr()?;
        self.expect(&TokenKind::RBracket, "expected ']'")?;
        let (op, op_span) = if matches!(self.peek_kind(), TokenKind::Eq) {
            (AssignOp::Assign, self.advance_span())
        } else if matches!(self.peek_kind(), TokenKind::PlusEq) {
            (AssignOp::AddAssign, self.advance_span())
        } else {
            return Err(SpannedError {
                message: "expected '=' or '+='".to_string(),
                span: self.peek_span(),
            });
        };
        let value = self.parse_expr()?;
        Ok(Spanned {
            span: Span {
                start: name_span.start,
                end: value.span.end,
            },
            node: Stmt::IndexAssign {
                name,
                name_span,
                index,
                op,
                op_span,
                value,
            },
        })
    }

    // Expression parsing: precedence climbing
    // or < and < comparison < add/sub < mul/div/mod < atom

    fn parse_expr(&mut self) -> Result<Spanned<Expr>, SpannedError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Spanned<Expr>, SpannedError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek_kind(), TokenKind::Or) {
            let op_span = self.advance_span();
            let right = self.parse_and()?;
            let span = Span {
                start: left.span.start,
                end: right.span.end,
            };
            left = Spanned {
                node: Expr::BinOp {
                    left: Box::new(left),
                    op: BinOp::Or,
                    op_span,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Spanned<Expr>, SpannedError> {
        let mut left = self.parse_cmp()?;
        while matches!(self.peek_kind(), TokenKind::And) {
            let op_span = self.advance_span();
            let right = self.parse_cmp()?;
            let span = Span {
                start: left.span.start,
                end: right.span.end,
            };
            left = Spanned {
                node: Expr::BinOp {
                    left: Box::new(left),
                    op: BinOp::And,
                    op_span,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn cmp_op(&self) -> Option<BinOp> {
        match self.peek_kind() {
            TokenKind::EqEq => Some(BinOp::Eq),
            TokenKind::BangEq => Some(BinOp::Ne),
            TokenKind::Lt => Some(BinOp::Lt),
            TokenKind::Gt => Some(BinOp::Gt),
            TokenKind::LtEq => Some(BinOp::Le),
            TokenKind::GtEq => Some(BinOp::Ge),
            _ => None,
        }
    }

    fn parse_cmp(&mut self) -> Result<Spanned<Expr>, SpannedError> {
        let left = self.parse_add()?;
        if let Some(op) = self.cmp_op() {
            let op_span = self.advance_span();
            let right = self.parse_add()?;
            let span = Span {
                start: left.span.start,
                end: right.span.end,
            };
            Ok(Spanned {
                node: Expr::BinOp {
                    left: Box::new(left),
                    op,
                    op_span,
                    right: Box::new(right),
                },
                span,
            })
        } else {
            Ok(left)
        }
    }

    fn add_op(&self) -> Option<BinOp> {
        match self.peek_kind() {
            TokenKind::Plus => Some(BinOp::Add),
            TokenKind::Minus => Some(BinOp::Sub),
            _ => None,
        }
    }

    fn parse_add(&mut self) -> Result<Spanned<Expr>, SpannedError> {
        let mut left = self.parse_mul()?;
        while let Some(op) = self.add_op() {
            let op_span = self.advance_span();
            let right = self.parse_mul()?;
            let span = Span {
                start: left.span.start,
                end: right.span.end,
            };
            left = Spanned {
                node: Expr::BinOp {
                    left: Box::new(left),
                    op,
                    op_span,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn mul_op(&self) -> Option<BinOp> {
        match self.peek_kind() {
            TokenKind::Star => Some(BinOp::Mul),
            TokenKind::Slash => Some(BinOp::Div),
            TokenKind::Percent => Some(BinOp::Mod),
            _ => None,
        }
    }

    fn parse_mul(&mut self) -> Result<Spanned<Expr>, SpannedError> {
        let mut left = self.parse_atom()?;
        while let Some(op) = self.mul_op() {
            let op_span = self.advance_span();
            let right = self.parse_atom()?;
            let span = Span {
                start: left.span.start,
                end: right.span.end,
            };
            left = Spanned {
                node: Expr::BinOp {
                    left: Box::new(left),
                    op,
                    op_span,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn parse_atom(&mut self) -> Result<Spanned<Expr>, SpannedError> {
        match self.peek_kind() {
            TokenKind::Int(n) => {
                let n = *n;
                let span = self.advance_span();
                Ok(Spanned {
                    node: Expr::Int(n),
                    span,
                })
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                let name_span = self.advance_span();
                // Function call: name(args...)
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.advance_span(); // consume (
                    let mut args = Vec::new();
                    if !matches!(self.peek_kind(), TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        while matches!(self.peek_kind(), TokenKind::Comma) {
                            self.advance_span();
                            args.push(self.parse_expr()?);
                        }
                    }
                    let rp = self.expect(&TokenKind::RParen, "expected ')'")?;
                    Ok(Spanned {
                        span: Span {
                            start: name_span.start,
                            end: rp.end,
                        },
                        node: Expr::Call {
                            name,
                            name_span,
                            args,
                        },
                    })
                }
                // Array index: name[expr]
                else if matches!(self.peek_kind(), TokenKind::LBracket) {
                    self.advance_span(); // consume [
                    let index = self.parse_expr()?;
                    let rb = self.expect(&TokenKind::RBracket, "expected ']'")?;
                    Ok(Spanned {
                        span: Span {
                            start: name_span.start,
                            end: rb.end,
                        },
                        node: Expr::Index {
                            name,
                            name_span,
                            index: Box::new(index),
                        },
                    })
                }
                // Simple variable
                else {
                    Ok(Spanned {
                        node: Expr::Var(name),
                        span: name_span,
                    })
                }
            }
            TokenKind::LParen => {
                let lp = self.advance_span();
                let inner = self.parse_expr()?;
                let rp = self.expect(&TokenKind::RParen, "expected ')'")?;
                Ok(Spanned {
                    node: inner.node,
                    span: Span {
                        start: lp.start,
                        end: rp.end,
                    },
                })
            }
            _ => Err(SpannedError {
                message: "expected expression".to_string(),
                span: self.peek_span(),
            }),
        }
    }
}

pub fn parse(source: &str) -> Result<Vec<Spanned<Stmt>>, SpannedError> {
    let tokens = lexer::lex(source)?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_assign() {
        let stmts = parse("x = 5").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::Assign {
                name, op, value, ..
            } => {
                assert_eq!(name, "x");
                assert_eq!(*op, AssignOp::Assign);
                assert_eq!(value.node, Expr::Int(5));
            }
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_add_assign() {
        let stmts = parse("x += 1").unwrap();
        match &stmts[0].node {
            Stmt::Assign { name, op, .. } => {
                assert_eq!(name, "x");
                assert_eq!(*op, AssignOp::AddAssign);
            }
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_print() {
        let stmts = parse("print 42").unwrap();
        match &stmts[0].node {
            Stmt::Print { value, .. } => {
                assert_eq!(value.node, Expr::Int(42));
            }
            _ => panic!("expected Print"),
        }
    }

    #[test]
    fn parse_precedence_mul_over_add() {
        let stmts = parse("x = 1 + 2 * 3").unwrap();
        match &stmts[0].node {
            Stmt::Assign { value, .. } => match &value.node {
                Expr::BinOp {
                    left,
                    op: BinOp::Add,
                    right,
                    ..
                } => {
                    assert_eq!(left.node, Expr::Int(1));
                    assert!(matches!(right.node, Expr::BinOp { op: BinOp::Mul, .. }));
                }
                _ => panic!("expected Add"),
            },
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_precedence_and_or() {
        let stmts = parse("x = a or b and c").unwrap();
        match &stmts[0].node {
            Stmt::Assign { value, .. } => match &value.node {
                Expr::BinOp {
                    op: BinOp::Or,
                    right,
                    ..
                } => {
                    assert!(matches!(right.node, Expr::BinOp { op: BinOp::And, .. }));
                }
                _ => panic!("expected Or"),
            },
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_comparison() {
        let stmts = parse("x = a == b").unwrap();
        match &stmts[0].node {
            Stmt::Assign { value, .. } => {
                assert!(matches!(value.node, Expr::BinOp { op: BinOp::Eq, .. }));
            }
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_parenthesized() {
        let stmts = parse("x = (1 + 2) * 3").unwrap();
        match &stmts[0].node {
            Stmt::Assign { value, .. } => match &value.node {
                Expr::BinOp {
                    left,
                    op: BinOp::Mul,
                    ..
                } => {
                    assert!(matches!(left.node, Expr::BinOp { op: BinOp::Add, .. }));
                }
                _ => panic!("expected Mul"),
            },
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_for_loop() {
        let stmts = parse("for i from 0 to 10\n  x = i\nend").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::For {
                var, body, from, to, ..
            } => {
                assert_eq!(var, "i");
                assert_eq!(from.node, Expr::Int(0));
                assert_eq!(to.node, Expr::Int(10));
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected For"),
        }
    }

    #[test]
    fn parse_while_loop() {
        let stmts = parse("while x > 0\n  x = x - 1\nend").unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0].node, Stmt::While { .. }));
    }

    #[test]
    fn parse_if_stmt() {
        let stmts = parse("if x == 0\n  y = 1\nend").unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0].node, Stmt::If { .. }));
    }

    #[test]
    fn parse_fn_def() {
        let stmts = parse("fn foo(a, b)\n  return a + b\nend").unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::FnDef {
                name, params, body, ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "a");
                assert_eq!(params[1].0, "b");
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_fn_no_params() {
        let stmts = parse("fn bar()\n  return 42\nend").unwrap();
        match &stmts[0].node {
            Stmt::FnDef { name, params, .. } => {
                assert_eq!(name, "bar");
                assert_eq!(params.len(), 0);
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_call_expr() {
        let stmts = parse("x = foo(1, 2)").unwrap();
        match &stmts[0].node {
            Stmt::Assign { value, .. } => match &value.node {
                Expr::Call { name, args, .. } => {
                    assert_eq!(name, "foo");
                    assert_eq!(args.len(), 2);
                }
                _ => panic!("expected Call"),
            },
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_index_expr() {
        let stmts = parse("x = arr[i + 1]").unwrap();
        match &stmts[0].node {
            Stmt::Assign { value, .. } => match &value.node {
                Expr::Index { name, .. } => {
                    assert_eq!(name, "arr");
                }
                _ => panic!("expected Index"),
            },
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_index_assign() {
        let stmts = parse("arr[0] = 5").unwrap();
        match &stmts[0].node {
            Stmt::IndexAssign { name, .. } => {
                assert_eq!(name, "arr");
            }
            _ => panic!("expected IndexAssign"),
        }
    }

    #[test]
    fn parse_break() {
        let stmts = parse("while 1\n  break\nend").unwrap();
        match &stmts[0].node {
            Stmt::While { body, .. } => {
                assert!(matches!(body[0].node, Stmt::Break { .. }));
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn parse_return() {
        let stmts = parse("fn f()\n  return 0\nend").unwrap();
        match &stmts[0].node {
            Stmt::FnDef { body, .. } => {
                assert!(matches!(body[0].node, Stmt::Return { .. }));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn parse_nested_blocks() {
        let src = "for i from 0 to 10\n  if i > 5\n    print i\n  end\nend";
        let stmts = parse(src).unwrap();
        assert_eq!(stmts.len(), 1);
        match &stmts[0].node {
            Stmt::For { body, .. } => {
                assert_eq!(body.len(), 1);
                match &body[0].node {
                    Stmt::If { body, .. } => assert_eq!(body.len(), 1),
                    _ => panic!("expected If inside For"),
                }
            }
            _ => panic!("expected For"),
        }
    }

    #[test]
    fn parse_multiple_stmts() {
        let stmts = parse("x = 1\ny = 2\nprint x").unwrap();
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn parse_error_unexpected_token() {
        let err = parse("42").unwrap_err();
        assert_eq!(err.message, "expected statement");
    }

    #[test]
    fn parse_span_tracking() {
        let stmts = parse("x = 10").unwrap();
        assert_eq!(stmts[0].span, Span { start: 0, end: 6 });
        match &stmts[0].node {
            Stmt::Assign {
                name_span, op_span, ..
            } => {
                assert_eq!(*name_span, Span { start: 0, end: 1 });
                assert_eq!(*op_span, Span { start: 2, end: 3 });
            }
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn parse_euler_001() {
        let src = "total = 0\n\nfor i from 0 to 1000\n  if i % 3 == 0 or i % 5 == 0\n    total += i\n  end\nend\n\nprint total";
        let stmts = parse(src).unwrap();
        assert_eq!(stmts.len(), 3);
    }
}
