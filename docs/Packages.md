# Packages and Workspaces

Packages define distribution and dependency management for FerroPhase
projects. They wrap module trees and metadata that the toolchain uses to
assemble reproducible builds. This guide explains how packages are structured,
how manifests are authored, and how the compiler consumes them. Compiler
internals are scheduler-driven.

## Anatomy of a Package

```
awesome-lib/
├── Magnet.toml
├── src/
│   ├── lib.fp
│   └── math/
│       ├── mod.fp
│       └── vector.fp
├── tests/
│   ├── compile/
│   └── runtime/
└── target/               # build artefacts (generated)
```

- `Magnet.toml` – package manifest (see below).
- `src/` – FerroPhase modules (see `Modules.md`).
- `tests/` – language-aware tests.
- `target/` – build outputs, caches, and transpiled artefacts. The CLI manages
  this directory.

## Manifest Overview

A package manifest is a TOML document split into logical sections:

```toml
[package]
name = "awesome-lib"
version = "0.3.0"
edition = "2024"
authors = ["ACME Labs <dev@acme.test>"]
license = "MIT"
description = "Vector utilities shared across backends"

[dependencies]
ferro-math = { version = "^2.1", features = ["linalg"] }

[features]
default = ["std"]
std = []
serde = ["ferro-math/serde"]

[build]
toolchain = "nightly-2024-08-15"
const_eval = { enable = true, allow_io = false }
optimization = "speed"
```

### Key Sections

- `[package]` – identity and metadata. Edition controls language features.
- `[dependencies]` – semver-based requirements. Optional fields:
  - `features` – feature flags to enable on the dependency.
  - `package` – rename the dependency within the package namespace.
- `[features]` – feature flag graph. Values list other features or dependency
  flags to enable.
- `[build]` - compiler configuration: toolchain pinning, comptime policy,
  optimisation mode, codegen flags.

### In-Memory Package Model

Internally the toolchain represents each package with an immutable snapshot.
Providers described in `Modules.md` build these snapshots from the filesystem:

```rust
pub struct PackageDescriptor {
    pub id: PackageId,
    pub name: String,
    pub version: Version,
    pub manifest_path: VirtualPath,
    pub root: VirtualPath,
    pub metadata: PackageMetadata,
    pub modules: Vec<ModuleDescriptor>,
}

pub struct PackageMetadata {
    pub edition: Option<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub keywords: Vec<String>,
    pub registry: Option<String>,
    pub features: BTreeMap<String, Vec<FeatureRef>>,
    pub dependencies: Vec<DependencyDescriptor>,
}
```

- `manifest_path` and `root` live inside the virtual filesystem layer.
- `modules` stores module descriptors collected by the owning provider.
- `dependencies` captures the normalized dependency graph, including feature
  edges.

### Language-Specific Resolution

fp resolves modules and symbols via a language-specific strategy:

- **FerroPhase**: `crate::`, `self::`, `super::`, `use` trees, and Rust
  visibility rules.
- **Rust**: real `.rs` Cargo projects, resolved against rustc source through
  `fp_rust::RustPackageProvider`.

Each strategy maps imports to `ModuleId` and resolves symbols within a module.
The shared compiler scheduler coordinates resolution, diagnostics, and follow-up
work.

## Workspaces

A workspace coordinates multiple packages with a shared lockfile:

```toml
[workspace]
members = ["crates/*", "tools/cli"]

[workspace.metadata]
toolchain = "nightly-2024-08-15"
```

- Each member includes its own package manifest.
- Dependencies are resolved once and reused across members.

## Dependency Resolution & Lockfiles

- `Magnet.lock` records exact versions and checksums per dependency.
- Target-specific builds prune dependencies via target filters already captured
  in the graph.
- fp assumes the graph is consistent (no cycles, compatible versions).

## Build Work

1. **Parse and normalize** source modules into canonical AST.
2. **Type requested scopes** and record `CompileTimeNeed` blockers.
3. **Answer comptime requests** for const values, generated declarations,
   explicit comptime arguments, and requested specializations.
4. **Lower requested scopes** through typed AST, HIR, MIR, and LIR.
5. **Emit requested artefacts** such as LLVM IR, bytecode, and transpiled
   sources.

The CLI stores intermediate artefacts under `target/<lang>/` so repeated builds
reuse previous work when inputs, feature sets, and toolchain versions match.

## Versioning Strategy

- Semantic versioning: breaking API changes bump the major version.
- `Magnet.lock` pins exact versions; CI pipelines should check it in to
  guarantee reproducible builds.
- Version governance, deprecation windows, and semantic freeze policies are
  defined in `docs/VersionGovernance.md` and apply to every published package.
- Release artifacts and attestations required for publishing are defined in
  `docs/ReleaseArtifacts.md`.

## Best Practices

- Keep manifests declarative; avoid custom build scripts until absolutely
  necessary.
- Leverage features to gate expensive comptime request paths or optional
  bindings.
- Use workspaces for mono-repos: they shorten build times by sharing caches and
  lockfiles.
- Run `fp audit` regularly to verify dependency integrity and license
  compliance.
