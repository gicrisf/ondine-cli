# ondine-cli

Native Mermaid rendering with the [Ondine](https://ondine.sh) compiler.

Ondine compiles Mermaid diagrams to monochrome SVG with a pure Rust compiler. This project is a
small, independent command-line front end for that compiler. It reads Mermaid files or standard
input, writes SVG, and prints diagrams as ASCII text. It needs no Node.js, npm, or WebAssembly at
runtime, and it makes no network requests.

## Advantages

- **Native:** One Rust binary. No Node.js or WebAssembly runtime is needed.
- **Fast:** Avoids Node startup and WebAssembly compilation.
- **Identical output:** Uses the same Ondine compiler and produces the same SVG.
- **Complete:** All 33 Ondine diagram types.
- **Simple:** Install with Cargo, or run through the Nix flake.
- **Build-friendly:** Fits CI jobs, Makefiles, and static-site pipelines.

## Performance

Native startup is a few milliseconds. The table compares ASCII output against `ondine@0.0.3`:

| Workload                       |     Rust |      npm | Ratio |
|--------------------------------|---------:|---------:|------:|
| 522 diagrams, per-type batches |  0.292 s |  9.041 s |   31x |
| 522 diagrams, one process      |  0.136 s |  0.959 s |    7x |
| Per diagram, one process       | 0.261 ms | 1.836 ms |    7x |

## Install

```sh
cargo install --git https://github.com/gicrisf/ondine-cli --locked --bin ondine
```

The build requires Rust 1.97.0 or newer.

## Nix

The flake provides the package, a development shell, and a formatter:

```sh
# Runs the command without installing it
nix run github:gicrisf/ondine-cli -- flow.mmd

# Builds the package
nix build

# Enters a shell with cargo, rustc, clippy, rustfmt, and nixfmt
nix develop
```

## Usage

```sh
# Writes flow.svg beside flow.mmd
ondine flow.mmd

# Writes every SVG file into public/diagrams
ondine diagrams/*.mmd --out-dir public/diagrams

# Renders standard input to standard output
cat flow.mmd | ondine > flow.svg

# Prints flow.mmd as ASCII text instead of writing SVG
ondine --ascii flow.mmd
```

Options:

- `-a, --ascii` prints diagrams as ASCII text instead of writing SVG files.
- `-o, --out-dir <DIR>` writes every rendered file into one directory.
- `-h, --help` prints usage.
- `-V, --version` prints the version.

Files named `.mmd` or `.mermaid` replace that suffix with `.svg`. Other names keep their extension,
so `flow.txt` becomes `flow.txt.svg`.

A compiler error prints `source:line:column: message`, or `source: message` when it has no position,
to standard error. The command renders the remaining files and exits with status 1 if any file fails.

## Scope

This release renders SVG and ASCII. It does not rewrite Markdown fences, print terminal images,
produce JSON, or implement `--check`.

## Compiler Revision

The `compiler` crate is pinned to a commit in `wevm/ondine` because it is not published on
crates.io. Update the `rev` in `Cargo.toml` deliberately, then run `cargo test`.

## Credits

This project exists because of [Ondine](https://github.com/wevm/ondine).

Ondine ports [Pele](https://github.com/obsidianmd/pele).

This project is independent and unofficial. It is not maintained by or affiliated with the Ondine
project.

## License

MIT.
