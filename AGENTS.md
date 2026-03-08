# baselang

Repository notes for any coding agent or contributor working in this repo.

baselang is a small interpreted language implemented in Rust.

## Agent Notes

- Prefer minimal, targeted changes that match the existing style.
- If you change the lexer, parser, evaluator, or LSP behavior, add or update tests when practical.
- After modifying lexer/LSP behavior, run `cargo build` so the local LSP binary used by `.nvim.lua` is rebuilt.
- Preserve the `$`-identifier and view-sidecar workflow described below; it is a core project feature.

## Build & Run

```bash
cargo build                              # build everything
cargo run -- examples/euler_001.code     # run a file (expected: 233168)
cargo test                               # run all tests
cargo test --lib                         # library tests only
cargo test --bin baselang-lsp            # LSP tests only
```

## Project Structure

- `src/ast.rs` -- Span, Spanned<T>, Stmt, Expr, BinOp, AssignOp
- `src/lexer.rs` -- hand-written tokenizer (supports `$`-prefixed identifiers for views)
- `src/parser.rs` -- recursive descent parser
- `src/eval.rs` -- tree-walking interpreter
- `src/lsp.rs` -- LSP server (semantic tokens, go-to-def, diagnostics)
- `src/main.rs` -- file runner + REPL
- `editors/nvim/baselang-views.lua` -- Neovim plugin for views (projectional editing)
- `.nvim.lua` -- project-local Neovim config (LSP + views setup)

## Views (Projectional Editing)

Source files can use `$`-prefixed identifiers (e.g., `$e3a`) with a `.code.view` JSON sidecar that maps IDs to display names per mode:

```json
{ "$e3a": { "main": "sum", "maths": "Σ", "compact": "s" } }
```

In Neovim, `:BaselangView main` / `:BaselangView maths` / `:BaselangView off` switches modes. The cursor line always reveals raw `$` IDs for editing; all other lines show projected names.

After modifying the lexer/LSP, run `cargo build` to rebuild the LSP binary used by `.nvim.lua`.

## Testing the Neovim Plugin

Use headless Neovim to verify the views plugin without a GUI:

```bash
nvim --headless --clean -u .nvim.lua \
  -c "edit examples/euler/euler_001.code" \
  -c "lua vim.schedule(function()
    local ns = vim.api.nvim_get_namespaces()['baselang_views']
    local marks = vim.api.nvim_buf_get_extmarks(0, ns, 0, -1, {details=true})
    for _, m in ipairs(marks) do
      print(string.format('line %d col %d -> %s', m[2]+1, m[3], m[4].virt_text[1][1]))
    end
    vim.cmd('quit')
  end)" 2>&1
```

Key things to check:
- Extmarks exist on all lines with `$` identifiers except the cursor line
- `virt_text_pos` is `inline` and `conceal` is `""`
- Switching modes via `require('baselang-views').set_mode('maths')` updates the projected names
