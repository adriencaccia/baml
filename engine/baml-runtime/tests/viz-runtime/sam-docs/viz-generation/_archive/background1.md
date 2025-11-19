# BAML `//#` Annotation Pipeline

This note captures the pieces of the BAML engine that already understand markdown header annotations (lines that begin with `//#`, `//##`, …) and how those annotations flow from parsing through code generation, runtime notifications, and visualization tooling. Hand this to any coding agent that needs to extend the feature.

## Repository & Language Map
- Core compiler/runtime lives under `engine/` and is written in Rust (workspace managed by `engine/Cargo.toml`).
- The surface grammar and parser are in `engine/baml-lib/ast/src/parser/` (Pest-based).
- High-level IR (HIR/THIR), bytecode generation, and VM runtime are in `engine/baml-compiler/src/` and `engine/baml-vm/src/`.
- Visualization helpers (header collectors, graph rendering) live in `engine/baml-lib/ast/src/ast/baml_vis/`.

## Parsing Layer (Pest)
- Grammar rule that recognizes comment blocks & single-line comments: `comment_block` / `comment` in `engine/baml-lib/ast/src/parser/datamodel.pest`.
- `parse_expr.rs` handles block parsing. `parse_expr_block` does a two-pass walk:
  1. Collect all headers found in `comment_block` tokens, normalize them so each block starts at level 1 (`normalize_headers`).
  2. Re-walk items, attaching the normalized headers to the next statement or trailing expression via `bind_headers_to_statement`.
- Helper that parses `//### header text` lines into `Header` structs lives in `parse_expr.rs::parse_comment_header_pair`.
- Top-level headers (before a `function`, `test`, etc.) are captured in `parse.rs`, using a `pending_headers` queue that drains into `expr_fn.annotations`, `value_expression_block.annotations`, etc.

## AST Representation
- Structs that carry annotations:
  - `ast::Header` (level, title, span) in `engine/baml-lib/ast/src/ast/stmt.rs`.
  - Every statement variant that can be labeled has `annotations: Vec<Arc<Header>>` (see `ExprStmt`, `LetStmt`, loop structs, etc.).
  - `ast::ExpressionBlock` retains `expr_headers` for headers that apply to the trailing expression.
- Headers are stored in `Arc` so the same header instance can be reused across statements and diagnostic/visualization passes.

## Lowering: AST → HIR → THIR
- `engine/baml-compiler/src/hir/lowering.rs` converts annotated AST statements into `hir::Statement::AnnotatedStatement { headers, statement }` via `maybe_annotated_statement`.
- `engine/baml-compiler/src/hir.rs` and `thir.rs` both define the `AnnotatedStatement` wrapper so headers propagate through type checking.
- When an `ExpressionBlock` has trailing `expr_headers`, the lowering phase injects an `AnnotatedStatement` with `statement: None` so headers tied to the return expression survive.
