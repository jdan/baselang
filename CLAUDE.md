# baselang

A small interpreted language implemented in Rust.

## Build & Run

```bash
cargo build                              # build everything
cargo run -- examples/euler_001.code     # run a file (expected: 233168)
cargo test                               # run all tests
cargo test --lib                         # library tests only
cargo test --bin baselang-lsp            # LSP tests only
```

## Project Structure

- `src/ast.rs` — Span, Spanned<T>, Stmt, Expr, BinOp, AssignOp
- `src/lexer.rs` — hand-written tokenizer
- `src/parser.rs` — recursive descent parser
- `src/eval.rs` — tree-walking interpreter
- `src/lsp.rs` — LSP server (semantic tokens, go-to-def, diagnostics)
- `src/main.rs` — file runner + REPL
