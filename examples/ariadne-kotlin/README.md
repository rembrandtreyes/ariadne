# ariadne-kotlin -- Example Ariadne Plugin

A minimal Ariadne language plugin that adds Kotlin (`.kt`, `.kts`) support
using basic line-by-line parsing. This is an **example** -- it demonstrates the
WIT bindgen pattern, not production-grade parsing.

## Prerequisites

```bash
# Install the wasm32-wasip1 target
rustup target add wasm32-wasip1

# Install cargo-component
cargo install cargo-component
```

## Build

```bash
cargo component build --release
```

The output WASM component will be at:

```
target/wasm32-wasip1/release/ariadne_kotlin.wasm
```

## Install

Copy the `.wasm` file to a plugin directory that Ariadne scans:

```bash
# Per-repository (highest priority)
cp target/wasm32-wasip1/release/ariadne_kotlin.wasm \
   /path/to/your/repo/.ariadne/plugins/

# Or user-global
cp target/wasm32-wasip1/release/ariadne_kotlin.wasm \
   ~/.ariadne/plugins/
```

## What It Parses

This example plugin detects:

| Pattern | Extracted As |
|---------|-------------|
| `fun name(` | Function/Method symbol |
| `class Name` | Class symbol |
| `data class Name` | Class symbol |
| `interface Name` | Interface symbol |
| `object Name` | Module symbol |
| `enum class Name` | Enum symbol |
| `import com.example.Foo` | Import |
| `someFunction(` | Call relationship |

## Project Structure

```
ariadne-kotlin/
  Cargo.toml          # Build config targeting wasm32-wasip1
  wit/
    ariadne-plugin.wit # The ABI contract (copied from Ariadne source)
  src/
    lib.rs             # Plugin implementation
```

## How It Works

1. `wit_bindgen::generate!` reads the WIT contract and generates Rust types and
   a `Guest` trait.
2. `KotlinPlugin` implements `Guest` with `get_info()` and `parse_file()`.
3. `export!(KotlinPlugin)` wires the struct to the WASM component exports.
4. `cargo component build` compiles everything into a single `.wasm` file.

For full documentation, see `docs/PLUGINS.md` in the Ariadne repository.
