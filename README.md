# FerroPhase

FerroPhase is a multi-language compiler framework. Frontends translate source
languages into one shared AST, and the compiler lowers that AST through HIR,
typing, MIR, and LIR before interpretation, bytecode emission, native codegen,
or source-target printing.

The project is designed for two related workflows:

- **Compile mode** resolves and types the program for a concrete backend. High-
  level `#[op = "..."]` calls may be lowered to ordinary `std` wrappers, while
  `#[intrinsic = "..."]` declarations provide compiler-pipeline primitives.
- **Transpile mode** keeps the AST/source-level shape as high-level as possible
  and prints the requested target language. `--target rust` prints Rust
  source; `--target fp` prints FerroPhase source through a runtime-registered
  backend when one is available.

The same semantic contract is intended to hold across AST, HIR, MIR, LIR,
interpreters, bytecode, and compiled backends. A representation change must not
change observable program behavior.

## Quick Start

Build the CLI from the workspace:

```bash
cargo build --release -p fp-cli
export PATH="$PWD/target/release:$PATH"
```

Most compiler commands require a package identity:

```bash
fp check --package demo src/main.fp
fp interpret --package demo src/main.fp
fp eval "1 + 2 * 3"
fp parse src/main.fp
```

Compile to a native binary or inspect the shared pipeline:

```bash
fp compile src/main.fp --package demo --backend binary --output demo
fp compile src/main.fp --package demo --backend bytecode --output demo.fbc
fp compile src/main.fp --package demo --emit ast --emit ast-typed --emit hir
```

Print a source target through the same frontend, HIR, typing, and AST-lift
pipeline (typing is always performed):

```bash
fp compile src/main.fp --package demo --target rust --output main.rs
```

There is no separate `transpile` subcommand; source-target emission is selected
with `fp compile --target`.

## Architecture

```text
source
  -> LanguageFrontend
  -> shared AST
  -> mode-specific intrinsic normalization
  -> package/module resolution
  -> HIR
  -> typing and HIR type information
  -> MIR
  -> LIR
  -> interpreter, bytecode, native backend, or AST printer
```

Package providers own package discovery. The compiler driver services package
loads, including the provider-owned top-level `::libc` package. Module and
package resolution is an asynchronous compiler concern; lower layers consume
resolved package and module identities rather than implementing their own
filesystem or package lookup.

## Intrinsics And Standard Library

Use `#[op = "name"]` for high-level operations that should remain visible to
transpilers. Use `#[intrinsic = "name"]` for compiler-pipeline primitives.
Standard-library wrappers live in `std`; low-level compiler hooks live under
`std::intrinsics::*` and are marked with `#[intrinsic]`.

The C library bindings are a separate top-level `::libc` package. They are
generated manually with:

```bash
scripts/codegen_libc.sh crates/fp-lang/src/libc
```

The script uses Clang headers and emits platform-specific modules with target
`cfg` declarations in `libc/mod.fp`. The generated bindings use C ABI types and
raw pointers. `std::ffi` provides FerroPhase-facing wrappers such as `CStr`.
The old `std::libc` compatibility package is retired.

## Frontends And Backends

The workspace ships the FerroPhase frontend and a Rust frontend (`fp-rust`).
The C/C++ frontend infrastructure in `fp-clang` remains available for
embedding, but no other source-language crates are included.

Available backend families include the interpreter, bytecode and text
bytecode, native, LLVM, and Cranelift. The Rust source-target printer is
provided by `fp-lang`; additional targets can be added at runtime through the
target-backend registry.

## Documentation

- [Compile](docs/Compile.md) - CLI workflows and output modes
- [Pipeline](docs/Pipeline.md) - shared compiler stages and artefacts
- [Design](docs/Design.md) - scheduler and scoped lowering design
- [Modules](docs/Modules.md) - module paths and providers
- [Packages](docs/Packages.md) - package graphs and dependencies
- [Intrinsics](docs/Intrinsics.md) - operation and intrinsic normalization
- [Language](docs/Language.md) - semantic contract

Examples are in `examples/`.
