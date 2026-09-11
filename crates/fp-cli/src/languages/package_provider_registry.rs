use std::path::Path;
use std::sync::Arc;

use fp_core::ast::package::provider::PackageProvider;
use fp_lang::magnet_provider::MagnetWorkspaceProvider;

use super::backend_registry::LanguageProviderFactory;

/// Factory: maps a source language to a `PackageProvider` implementation.
/// A thin wrapper over `backend_registry::find_registered_language_provider` — the
/// built-ins below are registered into that exact same registry
/// (`builtin_language_providers`, seeded once), not looked up through a
/// separate match statement; an embedding binary's own
/// `register_language_provider` call sits in the same table, at the same
/// dispatch step.
pub fn provider_for_language(lang: &str, root: &Path) -> Option<Arc<dyn PackageProvider>> {
    super::backend_registry::find_registered_language_provider(lang, root)
}

fn factory<F>(f: F) -> LanguageProviderFactory
where
    F: Fn(&Path) -> Option<Arc<dyn PackageProvider>> + Send + Sync + 'static,
{
    Arc::new(f)
}

/// Every language `fp-cli` itself ships a provider for, as `(name,
/// factory)` pairs — the initial contents of the shared language-provider
/// registry (`backend_registry::language_provider_registry`'s `OnceLock` seed).
pub(crate) fn builtin_language_providers() -> Vec<(&'static str, LanguageProviderFactory)> {
    let mut entries: Vec<(&'static str, LanguageProviderFactory)> = Vec::new();

    let ferrophase = factory(|root: &Path| {
        MagnetWorkspaceProvider::discover(root)
            .ok()
            .map(|p| Arc::new(p) as Arc<dyn PackageProvider>)
    });
    entries.push(("ferrophase", ferrophase.clone()));
    entries.push(("fp", ferrophase));

    let rust = factory(|root: &Path| {
        Some(
            Arc::new(fp_rust::RustPackageProvider::new(root.to_path_buf()))
                as Arc<dyn PackageProvider>,
        )
    });
    entries.push(("rust", rust.clone()));
    entries.push(("rs", rust));

    entries.push(("object", factory(fp_native::package::object_provider)));
    entries.push(("archive", factory(fp_native::package::archive_provider)));

    // Raw asm text has no manifest/project shape either — same one-file,
    // one-package treatment as `object`, just lifted from a parsed
    // `AsmX86_64Program`/`AsmAarch64Program` instead of a binary object.
    let native_asm_auto = factory(|root: &Path| {
        fp_native::package::asm_text_provider(root, fp_native::package::AsmDialect::Auto)
    });
    entries.push(("native-asm", native_asm_auto.clone()));
    entries.push(("asm", native_asm_auto));
    let native_asm_x86_64 = factory(|root: &Path| {
        fp_native::package::asm_text_provider(root, fp_native::package::AsmDialect::X86_64)
    });
    entries.push(("x86_64-asm", native_asm_x86_64.clone()));
    entries.push(("asm-x86_64", native_asm_x86_64.clone()));
    entries.push(("x86asm", native_asm_x86_64.clone()));
    entries.push(("x86_64asm", native_asm_x86_64));
    let native_asm_aarch64 = factory(|root: &Path| {
        fp_native::package::asm_text_provider(root, fp_native::package::AsmDialect::Aarch64)
    });
    entries.push(("aarch64-asm", native_asm_aarch64.clone()));
    entries.push(("asm-aarch64", native_asm_aarch64.clone()));
    entries.push(("arm64-asm", native_asm_aarch64.clone()));
    entries.push(("aarch64asm", native_asm_aarch64));

    entries
}
