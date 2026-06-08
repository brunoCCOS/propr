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
```

Examples:
- `id(3) ; swap(1,2)`
- `mult * copy ; id(1) * swap(1,1)`
- `(f ; g) * (h ; i)`

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

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

MIT. See `LICENSE`.
