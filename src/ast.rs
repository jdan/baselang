#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    AddAssign,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    Var(String),
    BinOp {
        left: Box<Spanned<Expr>>,
        op: BinOp,
        op_span: Span,
        right: Box<Spanned<Expr>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Assign {
        name: String,
        name_span: Span,
        op: AssignOp,
        op_span: Span,
        value: Spanned<Expr>,
    },
    For {
        for_span: Span,
        var: String,
        var_span: Span,
        from_span: Span,
        from: Spanned<Expr>,
        to_span: Span,
        to: Spanned<Expr>,
        body: Vec<Spanned<Stmt>>,
        end_span: Span,
    },
    If {
        if_span: Span,
        cond: Spanned<Expr>,
        body: Vec<Spanned<Stmt>>,
        end_span: Span,
    },
    Print {
        print_span: Span,
        value: Spanned<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedError {
    pub message: String,
    pub span: Span,
}
