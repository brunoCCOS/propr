# propr

`propr` is a Rust compiler for **string diagrams of symmetric monoidal theories**
(also called **PROPs**). It parses an expression, type-checks arities and
coarities against a user-defined generator table, and emits a TikZ picture of
the corresponding string diagram.

## The language

```
id(n)       — identity on n wires
swap(m,n)   — symmetry permuting an m block past an n block
<name>      — user-defined generator (arity/coarity from config)
f ; g       — sequential composition (coarity(f) == arity(g))
f * g       — parallel composition (tensor)
( ... )     — grouping
lhs = rhs   — equation (diagram equality)
lhs ⊆ rhs   — equation (inclusion; ASCII alias `<=`)
```

Examples:
- `id(3) ; swap(1,2)`
- `mult * copy ; id(1) * swap(1,1)`
- `(f ; g) * (h ; i)`
- `copy ; mult = id(1)`

Equations relate two (or more) diagrams with `=` or `⊆` (aliased as `<=` in
ASCII input). Chains are allowed, e.g. `a = b ⊆ c`, and are read left to right
as a sequence of pairwise relations. All sides of an equation must have the
same arity and the same coarity. An equation is rendered as a single TikZ
picture containing each side's diagram in turn, with the relation symbol
placed between adjacent diagrams.

## Install

```bash
cargo install propr
```

Or from source:

```bash
git clone <url>
cd propr
cargo install --path .
```

## Usage

Without a config file, only `id` and `swap` are available:

```bash
propr "id(3) ; swap(1,2)"
```

With a config file defining custom generators:

```bash
propr --config generators.toml "mult * copy"
```

Config format (`generators.toml`):

```toml
[generators.mult]
arity = 2
coarity = 1
pic = "multiplication"
symbol = "⋅"

[generators.copy]
arity = 1
coarity = 2
pic = "copy"
```

Required fields per generator: `arity`, `coarity`.
Optional: `pic` (defaults to generator name), `params`, `visual_arity`,
`visual_coarity`, `symbol`, `width`, `height`.

## Anchor protocol

Custom generator pics are TikZ `pic` definitions supplied by the user (see
`pic` in the config format above). To let `propr` route wires to and from a
custom pic, the pic's TikZ code must define anchors named:

```
{pic-id}-in-{n}    — n-th input anchor, n starting at 0
{pic-id}-out-{n}   — n-th output anchor, n starting at 0
```

where `{pic-id}` is the name under which the pic is invoked (i.e. `pic (...)
{<pic-id>};`). Input anchors must be placed along the pic's left edge and
output anchors along its right edge, in order. The number of anchors a pic
must expose is governed by `visual_arity`/`visual_coarity` when set (see
above), otherwise by `arity`/`coarity`.

This naming scheme is part of `propr`'s public interface: any pic library
intended for use with `propr` must follow it exactly, since `propr` connects
wires by anchor name rather than by inspecting the pic's drawing.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

MIT. See `LICENSE`.
