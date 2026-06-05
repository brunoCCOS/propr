# propr

`propr` is a Rust compiler for **string diagrams of symmetric monoidal theories**
(also called **PROPs** — product and permutation categories). It parses an
expression, type-checks arities and coarities, and emits a TikZ picture of the
corresponding string diagram.

## What it accepts

The expression language implements the syntax of PROPs:

- `id(n)` — identity on `n` wires
- `swap(m,n)` — symmetry permuting an `m` block past an `n` block
- user-defined generators like `mult` or `sum(1,2)`
- sequential composition with `;`
- parallel composition (tensor) with `*`
- parentheses for grouping

Examples:

- `mult * copy ; id(1) * swap(1,1)`
- `id(3) ; swap(1,2)`
- `f(1,2) ; (g * h)`

## Install

### From source (current option)

You need Rust installed (`rustup` + `cargo`):

```bash
git clone <your-repo-url>
cd propr
cargo install --path .
```

Then run:

```bash
propr "a * b ; c * d"
```

### Planned install options

Once published, users will also be able to install with:

```bash
cargo install propr
```

And via prebuilt binaries from GitHub Releases.

## Usage

```bash
propr "<expression>"
```

If no expression is provided, the CLI prints:

```text
usage: propr <expression>
```

If parsing or type-checking fails, it prints an error like:

```text
propr: <error message>
```

## Development

Run local checks before committing:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

MIT. See `LICENSE`.
