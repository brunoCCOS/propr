# propr

`propr` is a Rust command-line tool that parses a small expression language,
type-checks it, and generates TikZ output.

## What it accepts

The expression language currently supports:

- `id(n)`
- `swap(a,b)`
- generator names like `foo` and `bar(1,2,3)`
- composition with `;`
- tensor with `*`
- parentheses for grouping

Examples:

- `a * b ; c * d`
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
