# Contributing to propr

This guide explains the architecture of the compiler so you can navigate the
source and make changes with confidence.

---

## 1. What the tool does

A **PROP** ("product and permutations category") is an algebraic structure:
morphisms have an *arity* (number of input wires) and a *coarity* (number of
output wires), and they combine in two ways:

- **Sequential composition** `f ; g`: valid when `coarity(f) = arity(g)`;
  produces a morphism `arity(f) → coarity(g)`.
- **Parallel composition** (tensor) `f * g`: always valid; produces a morphism
  `arity(f) + arity(g) → coarity(f) + coarity(g)`.

Two structural primitives are built in:

- `id(n)` — identity on `n` wires (`n → n`).
- `swap(m,n)` — symmetry permuting an `m`-block past an `n`-block
  (`m+n → m+n`).

All other morphisms are **user-defined generators**: named atoms declared in
code with their arity, coarity, visual wire counts, and an associated TikZ pic
name.

The compiler takes an expression like `mult * copy ; id(1) * swap(1,1)` and
emits a `tikzpicture` block that renders it as a string diagram.

---

## 2. Repository layout

```
src/
  main.rs           CLI entry point.
  lib.rs            Public API: compile(expr, env) -> TikZ string.
  lexer/
    scan.rs         Character scanner / tokenizer.
    tokens.rs       TokenKind and Token definitions.
    generator.rs    Generator metadata struct.
  parser/
    engine.rs       Recursive-descent parser.
    ast.rs          AST node types.
  typechecker.rs    Arity/coarity checker.
  codegen.rs        TikZ emitter — the largest module.
```

---

## 3. The compilation pipeline

```
source string
  │
  ▼  scan::Lexer::tokenize        (src/lexer/scan.rs)
Vec<Token>
  │
  ▼  engine::parse                (src/parser/engine.rs)
ast::Expr
  │
  ▼  typechecker::check           (src/typechecker.rs)
typechecker::Sig                  — succeeds or fails with a clear error
  │
  ▼  codegen::generate            (src/codegen.rs)
String  (a complete \begin{tikzpicture}…\end{tikzpicture})
```

Each stage is independently testable.

---

## 4. Lexer (`src/lexer/`)

### 4.1 Tokens (`tokens.rs`)

```rust
pub enum TokenKind {
    Number(u32),
    Ident(String),
    Id,
    Swap,
    Lparen,
    Rparen,
    Comma,
    Comp,   // ;
    Tensor, // *
    Eof,
}
```

`Id` and `Swap` are **keywords** — the lexer matches `"id"` and `"swap"` and
returns these kinds instead of `Ident`. Everything else alphabetic becomes
`Ident`.

`Token` carries the kind and the byte offset in the source (`pos`), used in
error messages.

### 4.2 Scanning (`scan.rs`)

The lexer is a plain char-slice scanner with a `pos` index:

- `new(input)` — constructor.
- `advance_token()` — return the next token (or `Eof`).
- `tokenize()` — drive `advance_token()` in a loop and return `Vec<Token>`.

`peek()` and `advance()` are the standard helpers. `skip_whitespace()` runs
before each token. Identifiers start with a letter or underscore and continue
with letters, digits, or underscores. Numbers are unsigned digit runs.

### 4.3 Generator metadata (`generator.rs`)

```rust
pub struct Generator {
    pub value: String,
    pub params: Vec<String>,
    pub arity: u32,
    pub coarity: u32,
    pub visual_arity: Option<u32>,
    pub visual_coarity: Option<u32>,
    pub symbol: String,
    pub pic: String,
    pub width: f32,
    pub height: f32,
}
```

`value` is the lookup key used by the typechecker. `pic` is the name of the
TikZ pic that draws this generator; if empty, falls back to `value`.
`visual_arity` / `visual_coarity` control how many visual wires the box has
(several type-level wires can bundle into one visual wire). `width` and
`height` override the per-box default (1×1) in codegen.

---

## 5. Parser (`src/parser/`)

### 5.1 AST (`ast.rs`)

```rust
pub enum Expr {
    Comp(Box<Expr>, Box<Expr>),     // f ; g
    Tensor(Box<Expr>, Box<Expr>),   // f * g
    Id(u32),                        // id(n)
    Swap(u32, u32),                 // swap(m,n)
    Gen { name: String, args: Vec<u32> },  // user-defined
}
```

No `Paren` node — grouping disappears during parsing.

### 5.2 Grammar and precedence (`engine.rs`)

```
expr  := term ( ';' term )*
term  := atom ( '*' atom )*
atom  := 'id'   '(' NUMBER ')'
       | 'swap' '(' NUMBER ',' NUMBER ')'
       | IDENT  ( '(' NUMBER ( ',' NUMBER )* ')' )?
       | '(' expr ')'
```

`*` binds tighter than `;`, both are left-associative. So `a*b ; c*d` parses
as `(a*b) ; (c*d)`, and `a;b;c` parses as `(a;b);c`. The parser is hand-written
recursive descent.

---

## 6. Typechecker (`src/typechecker.rs`)

The typechecker verifies arity/coarity consistency:

```rust
pub struct Sig {
    pub arity: u32,
    pub coarity: u32,
}

pub type Env = HashMap<String, Generator>;
```

`check(expr, env)` is structural recursion:

- `Id(n)` → `Sig { n, n }`.
- `Swap(m,n)` → `Sig { m+n, m+n }`.
- `Gen { name, .. }` → look up in env, return its `Sig`. Error if missing.
- `Tensor(l, r)` → recurse, return `Sig { l.arity + r.arity, l.coarity + r.coarity }`.
- `Comp(l, r)` → recurse, verify `l.coarity == r.arity`. Error on mismatch.

The checker is linear in AST size.

---

## 7. Codegen (`src/codegen.rs`)

This is the largest module. Read this section carefully before making changes.

### 7.1 Mental model: every subdiagram is a `Layout`

```rust
struct Layout {
    width: f32,
    height: f32,
    left: Vec<String>,   // anchor names on the left edge
    right: Vec<String>,  // anchor names on the right edge
    body: String,        // TikZ source for the interior
}
```

Two invariants hold for every `Layout`:

1. **Tip alignment**: the `left` anchors correspond to arity wires (top-first),
   `right` anchors to coarity wires (top-first).
2. **Frame**: bounding rectangle is `[0, width] × [0, height]`. Left tips at
   `x = 0`, right tips at `x = width`. Matching wire indices share the same y
   after composition by construction.

### 7.2 `Renderer`

```rust
struct Renderer<'a> {
    env: &'a Env,
    next_id: usize,
}
```

Every box gets a fresh prefix from `fresh(prefix)`, generating globally-unique
names like `g1`, `id_in_2`, `a3`.

### 7.3 `render_id` — identity on N wires

A single straight wire from `(0, 0.5)` to `(1, 0.5)`. Regardless of `N`, only
one visual wire is drawn. The `left` and `right` slices each contain `N`
entries all aliasing the same anchor, so composition with a multi-input
generator fans out at the join.

### 7.4 `render_swap` — visual swap is always 2×2

Two visual tips on each side. The two crossing wires are drawn as smooth
S-curves using bezier control points with a white `preaction` stroke for the
over/under crossing effect. The left/right anchor arrays alias the visual tips
to match the type-level `m` and `n` counts.

### 7.5 `render_gen` — user-defined generators

Looks up the generator in the environment, determines visual vs type-level wire
counts, validates the ratios, and emits a `\pic` with the generator's width and
height. The pic-local coordinate frame is expected to be `[-0.5, 0.5]` in both
axes, with anchors named `<pic-id>-in-k` and `<pic-id>-out-k`.

### 7.6 `render_tensor` — vertical stacking

A is placed above B. Width is `max(A.w, B.w)`, height is `A.h + B.h`.
Each child is horizontally centered. `reanchor_to` adds horizontal wire stubs
when a child is narrower than the parent, ensuring both edges remain flush.

### 7.7 `render_comp` — sequential composition

Places left and right side by side with `COMP_GAP` (0.25) between them. Both
children are vertically centered within the combined height. Connecting wires
use `to[out=0,in=180]` for smooth horizontal-to-horizontal curves.

### 7.8 `generate` — the public entry

Builds a `Renderer`, calls `render`, and wraps the body in
`\begin{tikzpicture}…\end{tikzpicture}`.

---

## 8. Coding conventions

- Format with `cargo fmt`.
- No clippy warnings — run `cargo clippy --all-targets -- -D warnings` before
  pushing.
- Errors are `Result<T, String>` throughout. Error messages include position
  info from the lexer where possible.
- Tests use Rust's built-in `#[test]` attribute. Table-driven tests preferred
  for multiple scenarios.
- Avoid adding dependencies. The compiler is intentionally dependency-free.

---

## 9. Tests

Every module has tests:

- **lexer**: token-kind sequences, identifier values, unexpected-char errors.
- **parser**: atoms, precedence, associativity, parens, malformed input.
- **typechecker**: atoms, tensor, comp, mismatch, unknown generator.
- **codegen**: smoke tests — wraps `tikzpicture`, emits pics, unknown generators.

Run all:

```bash
cargo test
```

---

## 10. PR process

1. Branch from `master`.
2. Run `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test` before pushing.
3. Open a PR. CI runs on push.
4. Include in the description: *what* changed, *why*, and an example expression
   with the resulting TikZ output.
5. For codegen changes, include a rendered diagram if possible.

---

## 11. Known limitations

- **Wire routing in composition is bezier-based**, not straight. When two boxes
  have different heights, connecting wires curve. The fix is to shift the smaller
  box vertically so tips align.
- **No parametric generators.** Arity/coarity are fixed per generator. The
  `params` field and `args` on `Gen` nodes exist as scaffolding for future
  parametric generators but are not yet used by the typechecker or codegen.
- **Error positions are not propagated from typechecker/codegen back to the
  source.** The AST does not currently carry position info.
