# Contributing to propc

This guide walks through every piece of the compiler so that a new contributor
can read the source top-to-bottom and know exactly what each line is doing and
why. The aim is to be exhaustive rather than terse — when in doubt, prefer
explaining over assuming background.

The reader is assumed to know Go and basic LaTeX/TikZ. No prior exposure to
PROPs (the algebraic structure being compiled) is required.

---

## 1. What the tool does

A **PROP** ("product and permutations category") is a small algebraic
formalism: morphisms have an *arity* `A` (number of input wires) and a
*coarity* `C` (number of output wires), and they can be combined in two ways:

- **Sequential composition** `f ; g`: only valid when `coarity(f) = arity(g)`;
  produces a morphism `arity(f) → coarity(g)`.
- **Parallel composition** (tensor) `f * g`: always valid; produces a morphism
  `arity(f) + arity(g) → coarity(f) + coarity(g)`.

Two structural primitives are built in:

- `id(n)` — identity on `n` wires (`n → n`).
- `swap(m, n)` — symmetry σ permuting an `m`-block past an `n`-block
  (`m+n → m+n`).

All other morphisms are **user-defined generators**: named atoms declared in a
JSON file with their arity, coarity, and an associated TikZ pic.

The compiler takes an expression like `(id(1) * mult) ; swap(1,1)` and emits a
`tikzpicture` block that renders it as a string diagram. That's it.

---

## 2. Repository layout

```
cmd/propc/        CLI entry point. main.go drives the pipeline.
src/lexer/        Tokenizer, token kinds, generator metadata struct.
src/parser/       Pratt-style parser. Produces the AST.
src/typecheck/    Arity/coarity checker.
src/codegen/      TikZ emitter. The bulk of the geometric reasoning.
src/loader/       Reads generators.json (from disk or from the embedded copy).
src/assets/       go:embed targets. The canonical .tikz files live in
                  material/ — sync-assets copies them here before each build.

material/         Author-facing TikZ files. main.tex \input's these directly.
cmd/propc/        propc CLI.
.github/workflows/test.yml   CI (vet + race tests + build).
Makefile          build / test / lint / sync-assets / install / clean.
```

Two things to internalise:

1. **The Go module path is `propc`**. When you publish, change `go.mod` to
   `github.com/<user>/propc` so `go install ...@latest` works for end users.
2. **`material/` is the source of truth** for the TikZ pic definitions and
   styles. `src/assets/` exists only because `//go:embed` requires the files
   to be inside the embedding package's directory tree. The `sync-assets`
   Makefile target enforces the copy. Never edit `src/assets/generator.tikz`
   by hand — your changes will be overwritten on the next `make build`.

---

## 3. The compilation pipeline

```
source string
  │
  ▼  lexer.Tokenize           (src/lexer)
[]lexer.Token
  │
  ▼  parser.Parse             (src/parser)
parser.Node  (an AST)
  │
  ▼  typecheck.Check          (src/typecheck)
typecheck.Sig                 — succeeds or fails with a clear error
  │
  ▼  codegen.Generate         (src/codegen)
string  (a complete \begin{tikzpicture}…\end{tikzpicture})
```

Each stage is independently testable and has its own `_test.go` file. Errors
flow up via `error` returns; no panics.

---

## 4. Lexer (`src/lexer`)

### 4.1 Tokens (`tokens.go`)

The token kinds are:

```go
EOF, LPAREN, RPAREN, COMMA, COMP, TENSOR, NUMBER, ID, SWAP, IDENT
```

`COMP` is `;`, `TENSOR` is `*`. `ID` and `SWAP` are *keywords* — the lexer
matches the literal strings `"id"` and `"swap"` and returns these kinds
instead of `IDENT`. Anything else that looks like an identifier becomes
`IDENT` and is later resolved against the generator table by the typechecker.

`Token` carries the kind, the original string slice (`Value`), and the byte
offset in the source (`Pos`). `Pos` is used in error messages so a user can
pinpoint where their expression broke.

### 4.2 Tokenizing (`lexer.go`)

The lexer is a plain rune-slice scanner with a `pos` index. The whole API is:

- `New(input)` — constructor.
- `Next()` — return the next token (or EOF).
- `Tokenize()` — drive `Next()` in a loop and return `[]Token`.

`peek()` and `advance()` are the standard helpers. `skipWhitespace()` is run
before each token. Identifiers follow the usual rule: start with a letter or
underscore, continue with letters, digits, or underscores. Numbers are an
unsigned run of digits (no negatives, no decimals — arity is `uint`).

The only unusual choice is that `\*` is the tensor operator. It's an artefact
of using ASCII; rebind in `Next()` if you want a different glyph.

### 4.3 Generator metadata (`generators.go`)

`Generator` is the metadata struct, not the AST node:

```go
type Generator struct {
    Value   string  `json:"name"`
    Arity   uint    `json:"arity"`
    Coarity uint    `json:"coarity"`
    Symbol  string  `json:"symbol,omitempty"`
    Pic     string  `json:"pic,omitempty"`
    Width   float64 `json:"width,omitempty"`
    Height  float64 `json:"height,omitempty"`
}
```

`Value` is the lookup key used by the typechecker (the field is called
`Value` for historical reasons; in the JSON it's `"name"`). `Pic` is the
name of the TikZ pic that draws this generator; if empty the lookup falls
back to `Value`. `Symbol` is currently informational only — it's where a
unicode glyph for the operator would live if you ever wanted to render a
prettified expression. `Width` and `Height` override the per-box defaults
(1 × 1) in codegen.

This struct lives in `lexer` for cyclic-import reasons: the typechecker
imports `lexer`, so does `loader`, so does `cmd/propc`, and putting it
anywhere else would force a new shared package. It's a known wart.

---

## 5. Parser (`src/parser`)

### 5.1 AST (`ast.go`)

The AST is five concrete node types, all implementing the empty
`Node` marker interface:

```go
Comp   { Left, Right Node }       // f ; g
Tensor { Left, Right Node }       // f * g
Id     { N uint }                 // id(n)
Swap   { M, N uint }              // swap(m,n)
Gen    { Name string }            // a user-defined name
```

That's it. There is no `Paren` node — grouping disappears during parsing.

### 5.2 Grammar and precedence (`parser.go`)

The grammar is:

```
expr  := term ( ';' term )*
term  := atom ( '*' atom )*
atom  := 'id'   '(' NUMBER ')'
       | 'swap' '(' NUMBER ',' NUMBER ')'
       | IDENT
       | '(' expr ')'
```

`*` binds tighter than `;`, both are left-associative. So `a*b ; c*d` parses
as `(a*b) ; (c*d)`, and `a;b;c` parses as `(a;b);c`. These are properties of
the recursive-descent structure — `parseExpr` consumes one `parseTerm` then a
sequence of `; parseTerm`, and `parseTerm` does the same for `* parseAtom`.

`parseArgs(n)` is the helper for the keyword forms. It parses a fixed-length
comma-separated number list inside parens and returns `[]uint`. Errors
surface immediately with the position carried by the offending token.

The parser is hand-written; there is no generated state machine. Keep it that
way unless the grammar starts to grow.

---

## 6. Typechecker (`src/typecheck`)

### 6.1 Sig and Env

```go
type Sig struct{ Arity, Coarity uint }
type Env map[string]lexer.Generator
```

`Sig` is the pair `(A, C)` the typechecker derives for every node. `Env`
maps a generator's `Value` (its name as used in expressions) to its metadata.
`NewEnv([]Generator)` builds the map and rejects nothing — duplicate
detection happens in the loader (§9).

### 6.2 Check recursion (`typecheck.go`)

`Check(n, env)` is a textbook structural recursion:

- `Id{N}` → `Sig{N, N}`.
- `Swap{M, N}` → `Sig{M+N, M+N}`.
- `Gen{Name}` → look up in env, return its `Sig`. Returns an
  "unknown generator" error if missing.
- `Tensor{L, R}` → recurse on both, return `Sig{L.A + R.A, L.C + R.C}`.
- `Comp{L, R}` → recurse on both. **Verifies `L.Coarity == R.Arity`**.
  On mismatch, returns a structured error naming both sides.

There are no inference unknowns — every node has a definite signature. That
makes errors precise and the algorithm linear in AST size.

---

## 7. Codegen (`src/codegen`)

This is the largest and most subtle file. Read this section carefully before
making changes — many small choices interact.

### 7.1 Mental model: every subdiagram is a `box`

```go
type box struct {
    w, h        float64
    left, right []string
    body        string
}
```

A `box` is the result of rendering a subexpression. Two invariants hold for
every box:

1. **Tip alignment**: the box exposes coordinate *names* (TikZ
   `\coordinate (…)` identifiers) on its left and right edges. The
   left names correspond to its arity wires, top-first; the right
   names correspond to its coarity wires, top-first.
2. **Frame**: in the box's own local coordinate system, the
   bounding rectangle is `[0, w] × [0, h]`. Every left tip is at
   `x = 0`. Every right tip is at `x = w`. Tips at the same wire
   index have the same y after composition by construction.

When boxes are combined (tensor / composition) they are placed inside
TikZ `scope` blocks with `shift={…}`. The named coordinates remain
globally accessible because TikZ coordinates are not scope-local.

`body` is the TikZ source for the box's interior. It is emitted *as-is*
inside the parent scope. Because the body uses globally-unique anchor
names (generated by `renderer.id()`), there is no name collision when
the same subexpression is shifted multiple times.

### 7.2 The pic frame convention

User-defined generators are drawn by `\pic`. Each pic in
`material/generator.tikz` lives in **pic-local frame `x ∈ [−0.5, 0.5]`,
`y ∈ [−0.5, 0.5]`**. The pic exposes its tips as

```
(<name>-in-0), (<name>-in-1), …    on the left edge (x = -0.5)
(<name>-out-0), (<name>-out-1), …  on the right edge (x = +0.5)
```

Top-first ordering: for `n` tips, `y_k = 0.5 − k/(n−1)` for `n ≥ 2`, or
`y_0 = 0` for `n = 1`.

To map the pic into the box convention `[0, w] × [0, h]`, codegen
emits `\pic (id) at (w/2, h/2) {pic};`. With defaults `w = h = 1`,
the pic's local origin lands at `(0.5, 0.5)`, so its tip at pic-local
`x = −0.5` lands at scope-local `x = 0` (the box's left edge), and
similarly on the right. **This is exactly why the box convention says
"tips on the left at x = 0, on the right at x = w"** — it lines up.

### 7.3 `renderer` and unique anchor names

```go
type renderer struct {
    env     typecheck.Env
    counter int
}
func (r *renderer) id() string { r.counter++; return fmt.Sprintf("n%d", r.counter) }
```

Every box gets a fresh `nK` prefix from `r.id()`. Anchor names like
`n3-in-0` are then globally unique even when the same generator name
is used twice in an expression.

### 7.4 `renderId` — identity on N wires

```go
const w, h, y = 1.0, 1.0, 0.5
```

A single straight wire from `(0, 0.5)` to `(1, 0.5)`. Regardless of
`N`, only **one** wire is drawn — the type-level multiplicity `N` is
not depicted. The box's `left[]` and `right[]` slices each contain
`N` entries, **all aliasing the same anchor**. So a composition like
`id(2) ; mult` legitimately attaches both of `mult`'s in-tips to the
same identity tip; the wire fans out at the join.

This is the "visual arity ≠ type arity" model. It keeps the diagram
clean without sacrificing typechecker precision.

### 7.5 `renderSwap` — visual swap is always 2 × 2

Like `Id`, `Swap` ignores `M, N` for drawing purposes. Two visual
tips on each side:

```
inTop  at (0, 1)    outTop  at (w, 1)
inBot  at (0, 0)    outBot  at (w, 0)
```

The two crossing wires are drawn as **smooth S-curves**:

```
\draw (inBot) .. controls (w/2, 0) and (w/2, 1) .. (outTop);
\draw[preaction={draw=white,line width=4pt}]
      (inTop) .. controls (w/2, 1) and (w/2, 0) .. (outBot);
```

The bezier control points are at the midpoint x with the start and
end y of the *same* wire. This is the standard horizontal-tangent
S-curve: it leaves the input flat, bends towards the opposite y,
crosses at `(w/2, h/2)`, and arrives flat at the output. The
`preaction={draw=white,line width=4pt}` on the top-to-bottom wire is
a TikZ trick: before drawing the line, paint a fatter white line
over it, which erases the crossing line underneath and produces the
over/under effect.

`left[]` and `right[]` alias the visual tips: the top `M` left
entries all point at `inTop`, the bottom `N` at `inBot`, and the
output side is permuted (`outTop` receives the bottom `N`, `outBot`
receives the top `M`).

### 7.6 `renderGen` — user-defined generators

```go
pic := g.Pic
if pic == "" { pic = g.Value }
w := g.Width   ; if w == 0 { w = 1 }
h := g.Height  ; if h == 0 { h = 1 }
\pic (id) at (w/2, h/2) {pic};
```

Width and height default to 1 × 1. They can be overridden per generator
in the JSON if a particular glyph needs more room (e.g. a scalar with a
long label). `left[i] = "<id>-in-<i>"`, `right[j] = "<id>-out-<j>"`.

### 7.7 `renderTensor` — vertical stacking

`A * B` places A *above* B (A in the upper half, B in the lower).
Width is `max(A.w, B.w)`; height is `A.h + B.h`. Each child is
horizontally centered within the parent width: `axOff = (w - A.w)/2`,
`bxOff = (w - B.w)/2`. A is shifted up by `B.h` so it lands above B.

After placing the children, `extendWires` adds horizontal stubs when a
child is narrower than the parent — so the parent's left and right
edges still receive flush tips. There are four calls:

```go
r.extendWires(&sb, a, axOff,         0, true)   // a's left tips out to x=0
r.extendWires(&sb, a, axOff + a.w,   w, false)  // a's right tips out to x=w
r.extendWires(&sb, b, bxOff,         0, true)
r.extendWires(&sb, b, bxOff + b.w,   w, false)
```

A common past bug was passing `targetX = w` for the *left* extension —
which drew stubs going from input tips out to the right edge of the
box. Always pass `0` for left and `w` for right.

`extendWires` skips emission when `|currentX − targetX| < 1e-9` —
i.e. when the child is already flush with the parent's edge.

### 7.8 `renderComp` — sequential composition

`f ; g` places f and g side by side with a `gap` (currently 0.25)
between them. Both children are vertically centered within the
combined height `max(A.h, B.h)`. Then for every output wire of A there
is a wire `\draw (a.right[i]) to[out=0,in=180] (b.left[i]);` — leaving
A's tip horizontally and arriving at B's tip horizontally.

`to[out=0,in=180]` is the standard TikZ "head-to-tail" curve. If both
tips happen to sit at the same y the curve degenerates into a straight
line; otherwise it's a smooth bezier. Future work (mentioned in §11)
will replace this with shift-to-align so wires are *always* straight.

### 7.9 `emitScoped`

Wraps a child's body in `\begin{scope}[shift={(x,y)}] … \end{scope}`
unless both offsets are zero, in which case the body is emitted
inline. Saves a few lines of output noise on uncomposed pictures.

### 7.10 `Generate` — the public entry

`Generate(node, env)` builds a `renderer`, calls `render(node)`,
and wraps the resulting `body` in `\begin{tikzpicture}…\end{tikzpicture}`.
That's the entire emitter API.

---

## 8. Pic conventions (`material/generator.tikz`)

A pic is a TikZ macro (`<name>/.pic = { … }`) that draws one generator
glyph in pic-local coordinates. The rules:

1. **Frame**: `x ∈ [−0.5, 0.5]`, `y ∈ [−0.5, 0.5]`. Stay inside this
   box; codegen assumes nothing leaks out.
2. **Tip anchors**: declare `\coordinate (-in-k)` and `\coordinate
   (-out-k)` at the appropriate y, top-first. Codegen references
   these names externally as `<pic-id>-in-k` and `<pic-id>-out-k`.
3. **Wires first, nodes last**. Draw any internal lines first, then
   place the coloured node so its fill covers the wire stubs that
   pass through the node's interior. Without this ordering, the
   wires would punch through the white-filled `whitedot` and make
   it look as if there's a black line crossing the dot.
4. **Don't extend past the tip x-coordinates** (`±0.5`). The visible
   ends of the wires must be exactly at the box edges so composition
   joins look continuous.

### 8.1 Styles (`material/generator.tikzstyles`)

Four named styles are referenced by the pics:

- `whitedot` — solid white circle with a black outline.
- `blackdot` — solid black circle.
- `boxnode_right` / `boxnode_left` — rounded rectangles with one
  flat side, used for the scalar / coscalar boxes.

If you add a new generator that needs a different glyph (a triangle,
say), add the style here and reference it in the pic.

### 8.2 Adding a new generator (end-to-end)

1. Define the pic in `material/generator.tikz`. Follow the four rules
   in §8. Use existing pics as templates.
2. Add an entry to the generator JSON. For the built-in defaults,
   edit `src/assets/generators.json`; for a per-project override,
   create a separate file and pass `-g`.
3. (Optional) Add a style in `material/generator.tikzstyles` if you
   need a new node shape.
4. Run `make build` — `sync-assets` will copy the .tikz files into
   `src/assets/`, the embed will pick them up, and the binary will
   know about the new generator.
5. Add a typechecker test if the generator's arity/coarity are
   non-obvious.

---

## 9. Loader (`src/loader`)

```go
LoadDefault() ([]lexer.Generator, error)    // reads embedded JSON
LoadFile(path string) ([]lexer.Generator, error)
```

Both go through `parse(b []byte)`, which:

1. Unmarshals into `struct { Generators []lexer.Generator }`.
2. Rejects entries with empty `name`.
3. Rejects duplicate names.

These checks live here, not in `typecheck`, so an invalid generator
file is caught immediately at startup rather than the first time the
user composes that name.

---

## 10. Embedded assets (`src/assets`)

```go
//go:embed generator.tikz
var GeneratorTikz string

//go:embed generator.tikzstyles
var GeneratorTikzstyles string

//go:embed generators.json
var GeneratorsJSON []byte
```

The `//go:embed` directives require the source files to be inside the
same package directory, which is why `src/assets/` exists as a copy
target rather than `material/` being embedded directly. The
`sync-assets` Makefile rule keeps the two in lockstep:

```
sync-assets:
    cp material/generator.tikz       src/assets/generator.tikz
    cp material/generator.tikzstyles src/assets/generator.tikzstyles
```

Note that `generators.json` lives only in `src/assets/`. There is no
canonical copy in `material/` because the main LaTeX document doesn't
need it — only the Go binary does.

---

## 11. CLI (`cmd/propc/main.go`)

The CLI is intentionally minimal. Flags:

```
-i path        read expression from a file (default: stdin if no positional argument)
-o path        write output to a file (default: stdout)
-g path        use a custom generators.json (default: built-in)
--check        typecheck only; print "ok: A -> C" on stderr and exit
--standalone   wrap output in a compilable LaTeX document
```

`run()` is the single business-logic function — `main()` only parses
flags and routes errors. The pipeline inside `run()` is exactly the
diagram in §3: read expression, load generators, parse, typecheck,
optionally short-circuit on `--check`, codegen, optionally wrap with
`wrapStandalone()`, write.

`wrapStandalone()` assembles a self-contained `.tex` document by
concatenating the embedded styles, the embedded pic library, and the
generated tikzpicture inside `\begin{document}…\end{document}`. It's
the easiest way to compile an example without setting up a LaTeX
project.

---

## 12. Tests

Every package except `cmd/propc` and `src/assets` has a `_test.go`.

- **lexer**: token-kind sequences, identifier values, unexpected-char
  error.
- **parser**: atoms, precedence (`*` over `;`), left-associativity,
  parens, malformed input.
- **typecheck**: atoms, tensor, comp, mismatch, unknown generator,
  nested composite.
- **loader**: embedded default loads, file load, duplicate rejection,
  empty-name rejection, missing-file error.
- **codegen**: *smoke tests only*. The exact TikZ output is not
  pinned because the layout is still evolving. We assert that:
  - the result is wrapped in `\begin{tikzpicture}…\end{tikzpicture}`,
  - generator pics appear in the output by name,
  - compositions succeed,
  - unknown generators produce an error.

If you make a layout change that you want to lock in, add a golden
test in `codegen_test.go` against a known-good string. Until then,
prefer smoke assertions to avoid churn-by-test.

Run everything with:

```
make test
```

CI runs `go vet`, `go test -race`, and `go build` on every push.

---

## 13. Known limitations and future work

- **Wire routing during composition is bezier-based**, not
  straight. When two boxes have mismatched heights, the connecting
  wires curve. The agreed direction is to *shift the smaller box*
  vertically so output tips of `f` align with input tips of `g`,
  which produces straight wires at the cost of a taller overall
  bounding box. Not yet implemented.
- **No parametric generators.** A generator's arity/coarity are
  fixed at JSON-declaration time. The agreed extension is a
  `params` array and arity/coarity *expressions* over those
  params (e.g. `"arity": "2*n"`) so a single declaration of `sum`
  covers `sum(1)`, `sum(2)`, etc. Parser already needs to be
  taught the `name(args)` form for `IDENT` to make this usable.
- **No backward error reporting from typecheck to source position.**
  Errors say *what* went wrong but not *which token* — because the
  AST doesn't currently carry position info. Add `Pos` to each AST
  node if you care about precise error locations.
- **Codegen ignores `g.Height`** for tip placement; tip y-positions
  are derived from the canonical even-spacing convention regardless
  of the box's declared height. This is fine for the current
  one-tall pics but will need work if you add a tall generator.

---

## 14. Coding conventions

- Go: format with `gofmt`, vet with `go vet`. The CI fails on either.
- One thing per package. Don't grow `codegen` into a layout DSL;
  if you need helpers, put them in a new file inside the same
  package.
- Errors are formatted with `fmt.Errorf("%w", err)` when wrapping,
  plain `fmt.Errorf("…")` when introducing a new error. No
  sentinel errors yet; add them only when callers genuinely need
  to switch on the error type.
- Tests live next to the code (`x.go` + `x_test.go`). Table-driven
  cases preferred for anything with more than two scenarios.
- Public names are documented with a leading sentence in the godoc
  style. Internal helpers don't need a comment unless the
  *intent* is non-obvious.
- Avoid bringing in dependencies. The compiler is intentionally
  stdlib-only.

---

## 15. PR process

1. Branch from `master`.
2. `make sync-assets test` before pushing — the embed step is
   silent and easy to forget.
3. Open a PR. CI runs on push.
4. Include in the description: *what* changed, *why*, and *one
   end-to-end example* (an expression and the resulting tikz).
5. For codegen changes, include a screenshot or PDF excerpt of
   the rendered diagram in the PR — visual regressions are not
   caught by tests.

That's the lot. If anything in this document is wrong, out of
date, or unclear, fix it in the same PR as the code change that
exposed it.
