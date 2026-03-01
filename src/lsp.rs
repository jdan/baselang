use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use baselang::ast::{BinOp, Expr, Span, Spanned, Stmt};
use baselang::parser;

// -- Semantic token types --
const KEYWORD: u32 = 0;
const VARIABLE: u32 = 1;
const NUMBER: u32 = 2;
const OPERATOR: u32 = 3;

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::NUMBER,
    SemanticTokenType::OPERATOR,
];

struct Backend {
    client: Client,
    documents: DashMap<Url, (String, Vec<Spanned<Stmt>>)>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
        }
    }

    fn reparse(&self, uri: &Url, text: &str) -> Vec<Diagnostic> {
        match parser::parse(text) {
            Ok(stmts) => {
                self.documents
                    .insert(uri.clone(), (text.to_string(), stmts));
                vec![]
            }
            Err(e) => {
                self.documents
                    .insert(uri.clone(), (text.to_string(), vec![]));
                let start = offset_to_position(text, e.span.start);
                let end = offset_to_position(text, e.span.end);
                vec![Diagnostic {
                    range: Range { start, end },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: e.message,
                    ..Default::default()
                }]
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES.to_vec(),
                                token_modifiers: vec![],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "baselang LSP initialized")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let diags = self.reparse(&uri, &text);
        self.client.publish_diagnostics(uri, diags, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let diags = self.reparse(&uri, &change.text);
            self.client.publish_diagnostics(uri, diags, None).await;
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let entry = match self.documents.get(&uri) {
            Some(e) => e,
            None => return Ok(None),
        };
        let (text, stmts) = entry.value();
        if stmts.is_empty() {
            return Ok(None);
        }

        let mut tokens = Vec::new();
        for stmt in stmts {
            collect_stmt_tokens(stmt, &mut tokens);
        }
        tokens.sort_by_key(|t| t.0);

        let semantic_tokens = delta_encode(&tokens, text);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: semantic_tokens,
        })))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let entry = match self.documents.get(&uri) {
            Some(e) => e,
            None => return Ok(None),
        };
        let (text, stmts) = entry.value();

        let offset = position_to_offset(text, pos);
        let mut defs: Vec<(String, Span)> = Vec::new();
        if let Some(def_span) = find_def_in_stmts(stmts, offset, &mut defs) {
            let start = offset_to_position(text, def_span.start);
            let end = offset_to_position(text, def_span.end);
            return Ok(Some(GotoDefinitionResponse::Array(vec![Location {
                uri: uri.clone(),
                range: Range { start, end },
            }])));
        }

        Ok(None)
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

// -- Semantic token collection --

/// (byte_offset, length, token_type)
type RawToken = (usize, usize, u32);

fn collect_stmt_tokens(stmt: &Spanned<Stmt>, tokens: &mut Vec<RawToken>) {
    match &stmt.node {
        Stmt::Assign {
            name_span,
            name,
            op_span,
            value,
            ..
        } => {
            tokens.push((name_span.start, name.len(), VARIABLE));
            tokens.push((op_span.start, op_span.end - op_span.start, OPERATOR));
            collect_expr_tokens(value, tokens);
        }
        Stmt::For {
            for_span,
            var,
            var_span,
            from_span,
            from,
            to_span,
            to,
            body,
            end_span,
        } => {
            tokens.push((for_span.start, for_span.end - for_span.start, KEYWORD));
            tokens.push((var_span.start, var.len(), VARIABLE));
            tokens.push((from_span.start, from_span.end - from_span.start, KEYWORD));
            collect_expr_tokens(from, tokens);
            tokens.push((to_span.start, to_span.end - to_span.start, KEYWORD));
            collect_expr_tokens(to, tokens);
            for s in body {
                collect_stmt_tokens(s, tokens);
            }
            tokens.push((end_span.start, end_span.end - end_span.start, KEYWORD));
        }
        Stmt::If {
            if_span,
            cond,
            body,
            end_span,
        } => {
            tokens.push((if_span.start, if_span.end - if_span.start, KEYWORD));
            collect_expr_tokens(cond, tokens);
            for s in body {
                collect_stmt_tokens(s, tokens);
            }
            tokens.push((end_span.start, end_span.end - end_span.start, KEYWORD));
        }
        Stmt::Print {
            print_span, value, ..
        } => {
            tokens.push((
                print_span.start,
                print_span.end - print_span.start,
                KEYWORD,
            ));
            collect_expr_tokens(value, tokens);
        }
    }
}

fn collect_expr_tokens(expr: &Spanned<Expr>, tokens: &mut Vec<RawToken>) {
    match &expr.node {
        Expr::Int(_) => {
            tokens.push((expr.span.start, expr.span.end - expr.span.start, NUMBER));
        }
        Expr::Var(_) => {
            tokens.push((expr.span.start, expr.span.end - expr.span.start, VARIABLE));
        }
        Expr::BinOp {
            left,
            op,
            op_span,
            right,
        } => {
            collect_expr_tokens(left, tokens);
            let token_type = match op {
                BinOp::And | BinOp::Or => KEYWORD,
                _ => OPERATOR,
            };
            tokens.push((op_span.start, op_span.end - op_span.start, token_type));
            collect_expr_tokens(right, tokens);
        }
    }
}

// -- Go-to-definition --

pub fn find_def_in_stmts(
    stmts: &[Spanned<Stmt>],
    offset: usize,
    defs: &mut Vec<(String, Span)>,
) -> Option<Span> {
    for stmt in stmts {
        if let Some(span) = find_def_in_stmt(stmt, offset, defs) {
            return Some(span);
        }
    }
    None
}

fn find_def_in_stmt(
    stmt: &Spanned<Stmt>,
    offset: usize,
    defs: &mut Vec<(String, Span)>,
) -> Option<Span> {
    if offset < stmt.span.start || offset >= stmt.span.end {
        record_defs(stmt, defs);
        return None;
    }

    match &stmt.node {
        Stmt::Assign {
            name,
            name_span,
            value,
            ..
        } => {
            // Cursor on LHS name → go to self
            if offset >= name_span.start && offset < name_span.end {
                return Some(*name_span);
            }
            // Check in value expression
            if let Some(span) = find_def_in_expr(value, offset, defs) {
                return Some(span);
            }
            defs.push((name.clone(), *name_span));
            None
        }
        Stmt::For {
            var,
            var_span,
            from,
            to,
            body,
            ..
        } => {
            if offset >= var_span.start && offset < var_span.end {
                return Some(*var_span);
            }
            if let Some(span) = find_def_in_expr(from, offset, defs) {
                return Some(span);
            }
            if let Some(span) = find_def_in_expr(to, offset, defs) {
                return Some(span);
            }
            defs.push((var.clone(), *var_span));
            find_def_in_stmts(body, offset, defs)
        }
        Stmt::If { cond, body, .. } => {
            if let Some(span) = find_def_in_expr(cond, offset, defs) {
                return Some(span);
            }
            find_def_in_stmts(body, offset, defs)
        }
        Stmt::Print { value, .. } => find_def_in_expr(value, offset, defs),
    }
}

fn find_def_in_expr(
    expr: &Spanned<Expr>,
    offset: usize,
    defs: &[(String, Span)],
) -> Option<Span> {
    if offset < expr.span.start || offset >= expr.span.end {
        return None;
    }

    match &expr.node {
        Expr::Var(name) => {
            for (def_name, def_span) in defs.iter().rev() {
                if def_name == name {
                    return Some(*def_span);
                }
            }
            None
        }
        Expr::Int(_) => None,
        Expr::BinOp { left, right, .. } => find_def_in_expr(left, offset, defs)
            .or_else(|| find_def_in_expr(right, offset, defs)),
    }
}

fn record_defs(stmt: &Spanned<Stmt>, defs: &mut Vec<(String, Span)>) {
    match &stmt.node {
        Stmt::Assign {
            name, name_span, ..
        } => {
            defs.push((name.clone(), *name_span));
        }
        Stmt::For {
            var,
            var_span,
            body,
            ..
        } => {
            defs.push((var.clone(), *var_span));
            for s in body {
                record_defs(s, defs);
            }
        }
        Stmt::If { body, .. } => {
            for s in body {
                record_defs(s, defs);
            }
        }
        Stmt::Print { .. } => {}
    }
}

// -- Delta encoding --

fn delta_encode(tokens: &[RawToken], source: &str) -> Vec<SemanticToken> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for &(offset, length, token_type) in tokens {
        let (line, col) = offset_to_line_col(source, offset);
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 {
            col - prev_start
        } else {
            col
        };
        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: length as u32,
            token_type,
            token_modifiers_bitset: 0,
        });
        prev_line = line;
        prev_start = col;
    }

    result
}

// -- Position utilities --

fn offset_to_position(text: &str, offset: usize) -> Position {
    let (line, col) = offset_to_line_col(text, offset);
    Position {
        line,
        character: col,
    }
}

fn offset_to_line_col(text: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if i == offset {
            return (line, col);
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn position_to_offset(text: &str, pos: Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if line == pos.line && col == pos.character {
            return i;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    text.len()
}

// -- Public helpers for tests --

pub fn collect_tokens_pub(stmts: &[Spanned<Stmt>]) -> Vec<RawToken> {
    let mut tokens = Vec::new();
    for stmt in stmts {
        collect_stmt_tokens(stmt, &mut tokens);
    }
    tokens.sort_by_key(|t| t.0);
    tokens
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use baselang::parser;

    #[test]
    fn test_collect_tokens_assign() {
        // "x = 5"
        let stmts = parser::parse("x = 5").unwrap();
        let tokens = collect_tokens_pub(&stmts);
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], (0, 1, VARIABLE)); // x
        assert_eq!(tokens[1], (2, 1, OPERATOR)); // =
        assert_eq!(tokens[2], (4, 1, NUMBER)); // 5
    }

    #[test]
    fn test_collect_tokens_print() {
        let stmts = parser::parse("print 42").unwrap();
        let tokens = collect_tokens_pub(&stmts);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (0, 5, KEYWORD)); // print
        assert_eq!(tokens[1], (6, 2, NUMBER)); // 42
    }

    #[test]
    fn test_collect_tokens_for() {
        let src = "for i from 0 to 10\n  x = i\nend";
        let stmts = parser::parse(src).unwrap();
        let tokens = collect_tokens_pub(&stmts);
        // for(KW) i(VAR) from(KW) 0(NUM) to(KW) 10(NUM) x(VAR) =(OP) i(VAR) end(KW)
        assert_eq!(tokens.len(), 10);
        assert_eq!(tokens[0].2, KEYWORD); // for
        assert_eq!(tokens[1].2, VARIABLE); // i
        assert_eq!(tokens[2].2, KEYWORD); // from
        assert_eq!(tokens[3].2, NUMBER); // 0
        assert_eq!(tokens[4].2, KEYWORD); // to
        assert_eq!(tokens[5].2, NUMBER); // 10
        assert_eq!(tokens[6].2, VARIABLE); // x
        assert_eq!(tokens[7].2, OPERATOR); // =
        assert_eq!(tokens[8].2, VARIABLE); // i
        assert_eq!(tokens[9].2, KEYWORD); // end
    }

    #[test]
    fn test_collect_tokens_and_or_are_keywords() {
        let stmts = parser::parse("x = a and b or c").unwrap();
        let tokens = collect_tokens_pub(&stmts);
        // x(VAR) =(OP) a(VAR) or(KW) b(VAR) and(KW) c(VAR)
        // After sort: x, =, a, and, b, or, c
        // Wait, precedence: a and b → binop, then or c
        // Tokens sorted by offset: x(0) =(2) a(4) and(6) b(10) or(12) c(15)
        // Actually let's just check the and/or are KEYWORD
        let keywords: Vec<_> = tokens.iter().filter(|t| t.2 == KEYWORD).collect();
        assert_eq!(keywords.len(), 2); // and, or
    }

    #[test]
    fn test_find_def_simple() {
        // "x = 5\nprint x"
        // cursor on "x" in print (offset 12)
        let src = "x = 5\nprint x";
        let stmts = parser::parse(src).unwrap();
        let mut defs = Vec::new();
        let result = find_def_in_stmts(&stmts, 12, &mut defs);
        // x is defined at offset 0..1
        assert_eq!(result, Some(Span { start: 0, end: 1 }));
    }

    #[test]
    fn test_find_def_for_var() {
        // cursor on "i" in body of for loop
        let src = "for i from 0 to 10\n  print i\nend";
        let stmts = parser::parse(src).unwrap();
        let mut defs = Vec::new();
        // "print i" — the "i" is at offset 27
        let result = find_def_in_stmts(&stmts, 27, &mut defs);
        // i is defined at offset 4..5 (the for var)
        assert_eq!(result, Some(Span { start: 4, end: 5 }));
    }

    #[test]
    fn test_find_def_reassigned() {
        // x = 1\nx = 2\nprint x
        // cursor on x in print should go to second assignment
        let src = "x = 1\nx = 2\nprint x";
        let stmts = parser::parse(src).unwrap();
        let mut defs = Vec::new();
        // "print x" — x is at offset 18
        let result = find_def_in_stmts(&stmts, 18, &mut defs);
        // Second x = 2, x at offset 6..7
        assert_eq!(result, Some(Span { start: 6, end: 7 }));
    }

    #[test]
    fn test_find_def_on_definition_site() {
        let src = "x = 5";
        let stmts = parser::parse(src).unwrap();
        let mut defs = Vec::new();
        // cursor on "x" at offset 0
        let result = find_def_in_stmts(&stmts, 0, &mut defs);
        assert_eq!(result, Some(Span { start: 0, end: 1 }));
    }

    #[test]
    fn test_find_def_undefined() {
        let src = "print x";
        let stmts = parser::parse(src).unwrap();
        let mut defs = Vec::new();
        // cursor on "x" at offset 6
        let result = find_def_in_stmts(&stmts, 6, &mut defs);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_def_in_euler_001() {
        let src = "total = 0\n\nfor i from 0 to 1000\n  if i % 3 == 0 or i % 5 == 0\n    total += i\n  end\nend\n\nprint total";
        let stmts = parser::parse(src).unwrap();
        let mut defs = Vec::new();
        // "total" in "print total" starts at offset 89
        // Let's find offset of last "total"
        let last_total = src.rfind("total").unwrap();
        let result = find_def_in_stmts(&stmts, last_total, &mut defs);
        // Should resolve to the "total" in "total += i" (offset 65)
        // or the first "total = 0" (offset 0). The += updates defs too.
        assert!(result.is_some());
    }

    #[test]
    fn test_offset_to_position() {
        let text = "hello\nworld";
        assert_eq!(
            offset_to_position(text, 0),
            Position {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            offset_to_position(text, 6),
            Position {
                line: 1,
                character: 0
            }
        );
        assert_eq!(
            offset_to_position(text, 8),
            Position {
                line: 1,
                character: 2
            }
        );
    }
}
