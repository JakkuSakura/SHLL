pub mod backend;
pub mod backend_registry;
pub mod frontend;
pub mod in_memory;
pub mod package_provider_registry;

use std::path::Path;

// Language identifier constants
/// A pre-compiled native object file given directly as `fp compile`'s
/// input (not FerroPhase source) — see `fp_native::NativeObjectPackageProvider`.
pub const NATIVE_OBJECT: &str = "object";
/// A native archive (`.a`/`.lib`) given directly as `fp compile`'s input —
/// see `fp_native::NativeObjectPackageProvider::from_archive`.
pub const NATIVE_ARCHIVE: &str = "archive";
/// Raw native assembly text given directly as `fp compile`'s input (auto
/// x86_64/aarch64 dialect detection) — see
/// `fp_native::NativeObjectPackageProvider::from_asm`.
pub const NATIVE_ASM: &str = "native-asm";
pub const RUST: &str = "rust";
pub const FERROPHASE: &str = fp_lang::FERROPHASE;

/// Language information structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub ast_target_supported: bool,
}

/// All supported languages
pub const SUPPORTED_LANGUAGES: &[Language] = &[
    Language {
        name: RUST,
        extensions: &["rs"],
        ast_target_supported: true,
    },
    Language {
        name: NATIVE_OBJECT,
        extensions: &["o", "obj"],
        ast_target_supported: false,
    },
    Language {
        name: NATIVE_ARCHIVE,
        extensions: &["a", "lib"],
        ast_target_supported: false,
    },
    Language {
        name: NATIVE_ASM,
        extensions: &["s", "asm"],
        ast_target_supported: false,
    },
    Language {
        name: FERROPHASE,
        extensions: &["fp"],
        ast_target_supported: true,
    },
];

/// Detect source language from file extension
pub fn detect_source_language(path: &Path) -> Option<&'static Language> {
    let ext = path.extension()?.to_str()?;
    SUPPORTED_LANGUAGES
        .iter()
        .find(|lang| lang.extensions.contains(&ext))
}

/// Detect source language for a directory project by manifest presence.
///
/// A real `Cargo.toml` means real `.rs` sources meant to resolve against
/// rustc's actual std (`"rust"`, see `fp_rust`/`docs/RustStd.md`). A
/// `Magnet.toml`-only project is FerroPhase's own `.fp` dialect (`"ferrophase"`)
/// regardless of whether its files happen to be named `.rs` or `.fp` —
/// `FerroFrontend` accepts both extensions for that dialect. Walks up from
/// `dir` to find the nearest manifest, mirroring `fp_lang::project::find_manifest`.
pub fn detect_project_language(dir: &Path) -> Option<&'static Language> {
    let root = fp_lang::project::find_manifest(dir)?;
    let name = if root.join("Cargo.toml").exists() {
        RUST
    } else if root.join("Magnet.toml").exists() {
        FERROPHASE
    } else {
        return None;
    };
    SUPPORTED_LANGUAGES.iter().find(|lang| lang.name == name)
}

/// Detect target language from string identifier
pub fn detect_target_language(target: &str) -> Option<&'static Language> {
    SUPPORTED_LANGUAGES
        .iter()
        .find(|lang| lang.name == target || lang.extensions.contains(&target))
}

/// Check if a target language is supported as an AST output target.
pub fn is_ast_target_supported(target: &str) -> bool {
    detect_target_language(target)
        .map(|lang| lang.ast_target_supported)
        .unwrap_or(false)
}
