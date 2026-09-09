use crate::serializer::*;
use fp_core::ast::package::AstPackage;
use fp_core::ast::{Ident, Path};
use fp_core::backend::{BackendConfig, PackageWriter, TargetBackend};
use std::collections::{HashMap, HashSet};
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

struct KotlinScan {
    ctx: KotlinWorkspaceContext,
    workspace_packages: HashSet<String>,
    /// Every package selected for Kotlin emission, sorted — used by
    /// `write_workspace` for `settings.gradle.kts`'s `include(...)`
    /// lines.
    package_names: Vec<String>,
    kotlin_packages: HashMap<String, String>,
}

const RUNTIME_PROJECT: &str = "fp-kotlin-runtime";

fn collect_kotlin_operation_decls(
    declarations: &[crate::kt_parser::KtDecl],
    module_path: &[String],
    registry: &mut fp_core::lang::LangItemRegistry,
) {
    for declaration in declarations {
        if let Some(op_name) = declaration.op_func.as_deref() {
            let mut path = module_path.to_vec();
            path.push(declaration.name.clone());
            registry.insert_op(
                op_name,
                Path::plain(path.into_iter().map(Ident::new).collect()),
            );
        }
        if let (Some(op_class), Some(op_method)) = (
            declaration.op_class.as_deref(),
            declaration.op_method.as_deref(),
        ) {
            let mut path = module_path.to_vec();
            path.push(declaration.name.clone());
            registry.insert_method_declaration(
                op_class,
                op_method,
                declaration.params.len(),
                fp_core::intrinsics::ResultTypeRule::NotStaticallyKnowable,
                Path::plain(path.into_iter().map(Ident::new).collect()),
            );
        }
        if let Some(op_class) = declaration.op_class.as_deref() {
            for member in &declaration.members {
                let Some(op_method) = member.op_method.as_deref() else {
                    continue;
                };
                let mut path = module_path.to_vec();
                path.push(declaration.name.clone());
                path.push(member.name.clone());
                registry.insert_method_declaration(
                    op_class,
                    op_method,
                    member.params.len(),
                    fp_core::intrinsics::ResultTypeRule::NotStaticallyKnowable,
                    Path::plain(path.into_iter().map(Ident::new).collect()),
                );
            }
        }
        let mut nested_path = module_path.to_vec();
        nested_path.push(declaration.name.clone());
        collect_kotlin_operation_decls(&declaration.members, &nested_path, registry);
    }
}

fn kotlin_operation_registry() -> Option<fp_core::lang::LangItemRegistry> {
    let diagnostics = fp_core::diagnostics::DiagnosticManager::new();
    let mut registry = fp_core::lang::LangItemRegistry::default();
    for (relative, declarations) in crate::kt_parser::load_std_declarations(&diagnostics)? {
        let mut module_path = vec!["kotlin".to_string()];
        if let Some(parent) = relative.parent() {
            module_path.extend(
                parent
                    .iter()
                    .filter_map(|segment| segment.to_str().map(str::to_owned)),
            );
        }
        collect_kotlin_operation_decls(&declarations, &module_path, &mut registry);
    }
    Some(registry)
}

/// `TargetBackend` wrapper around [`KotlinSerializer`]. Kotlin needs
/// workspace-wide context beyond what `BackendConfig` carries — the
/// workspace-wide `KotlinScan` is read lazily from `&AstProgram` on
/// first `emit_package`/`write_workspace` call, same as every
/// other backend gets its input. `config.root_name` (the *source* project
/// directory's name, not `config.workspace_root`, the output directory)
/// is read straight off `self.config` — `AstProgram` has no way to
/// reconstruct it, it isn't package data at all.
pub struct KotlinBackend {
    serializer: KotlinSerializer,
    config: BackendConfig,
    scan: std::sync::OnceLock<KotlinScan>,
    staging_root: std::sync::OnceLock<PathBuf>,
    output_published: std::sync::OnceLock<()>,
}

impl KotlinBackend {
    pub fn new(config: BackendConfig) -> Self {
        Self {
            serializer: KotlinSerializer,
            config,
            scan: std::sync::OnceLock::new(),
            staging_root: std::sync::OnceLock::new(),
            output_published: std::sync::OnceLock::new(),
        }
    }

    /// Builds and caches the workspace-wide scan from `&AstProgram`
    /// on first call. Safe to call from any package's `emit_package` —
    /// including the very first — since `run_named_target`'s typecheck
    /// phase already ran for every package in the workspace before any
    /// `emit_package` call happens.
    fn ensure_scan(
        &self,
        workspace: &fp_core::ast::program::AstProgram,
    ) -> fp_core::error::Result<&KotlinScan> {
        if let Some(scan) = self.scan.get() {
            return Ok(scan);
        }
        // The compiler loads dependencies for resolution, but only selected
        // roots become Kotlin projects. Keeping this set separate prevents
        // Rust sysroot crates such as `core` and `libc` from becoming Gradle
        // project dependencies without emitted Kotlin artifacts.
        let workspace_packages: HashSet<String> = self
            .config
            .emitted_packages
            .iter()
            .map(|package_id| package_id.as_str().to_owned())
            .collect();
        let kotlin_packages = workspace_packages
            .iter()
            .flat_map(|name| {
                let package =
                    kotlin_package_name(self.config.kotlin_package_prefix.as_deref(), name);
                [
                    (name.clone(), package.clone()),
                    (name.replace('-', "_"), package),
                ]
            })
            .collect::<HashMap<_, _>>();
        let sources: Vec<AstPackage> = workspace_packages
            .iter()
            .map(|name| workspace.package_source(&fp_core::ast::package::PackageId::new(name)))
            .collect::<fp_core::error::Result<_>>()?;
        let ctx = KotlinWorkspaceContext::collect(sources.iter());
        let mut package_names: Vec<String> = sources.iter().map(|s| s.name.clone()).collect();
        package_names.sort();
        let _ = self.scan.set(KotlinScan {
            ctx,
            workspace_packages,
            package_names,
            kotlin_packages,
        });
        Ok(self.scan.get().expect("just set above"))
    }

    fn initialize_output(&self) -> fp_core::error::Result<()> {
        if self.staging_root.get().is_none() {
            let staging_root = create_staging_directory(&self.config.workspace_root)?;
            self.staging_root
                .set(staging_root)
                .map_err(|_| fp_core::error::Error::from("Kotlin output initialization raced"))?;
        }
        Ok(())
    }

    fn output_root(&self) -> fp_core::error::Result<&FsPath> {
        self.initialize_output()?;
        self.staging_root
            .get()
            .map(PathBuf::as_path)
            .ok_or_else(|| fp_core::error::Error::from("Kotlin output staging was not initialized"))
    }

    fn publish_output(&self) -> fp_core::error::Result<()> {
        if self.output_published.get().is_some() {
            return Ok(());
        }
        publish_staged_workspace(self.output_root()?, &self.config.workspace_root)?;
        self.output_published
            .set(())
            .map_err(|_| fp_core::error::Error::from("Kotlin output publication raced"))?;
        Ok(())
    }
}

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn create_staging_directory(workspace_root: &FsPath) -> fp_core::error::Result<PathBuf> {
    let parent = workspace_root.parent().unwrap_or_else(|| FsPath::new("."));
    std::fs::create_dir_all(parent)?;
    let name = workspace_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("workspace");

    for _ in 0..100 {
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging_root = parent.join(format!(
            ".{name}.fp-kotlin-staging-{}-{sequence}",
            std::process::id()
        ));
        match std::fs::create_dir(&staging_root) {
            Ok(()) => return Ok(staging_root),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(fp_core::error::Error::from(format!(
        "could not create Kotlin staging directory beside {}",
        workspace_root.display()
    )))
}

fn publish_staged_workspace(
    staging_root: &FsPath,
    workspace_root: &FsPath,
) -> fp_core::error::Result<()> {
    match std::fs::symlink_metadata(workspace_root) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::rename(staging_root, workspace_root)?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    }

    let backup_root = create_staging_directory(workspace_root)?;
    std::fs::remove_dir(&backup_root)?;
    std::fs::rename(workspace_root, &backup_root)?;
    if let Err(publish_error) = std::fs::rename(staging_root, workspace_root) {
        return match std::fs::rename(&backup_root, workspace_root) {
            Ok(()) => Err(publish_error.into()),
            Err(restore_error) => Err(fp_core::error::Error::from(format!(
                "failed to publish Kotlin output ({publish_error}) and restore the previous workspace ({restore_error})"
            ))),
        };
    }

    remove_path(&backup_root)?;
    Ok(())
}

fn remove_path(path: &FsPath) -> std::io::Result<()> {
    if std::fs::symlink_metadata(path)?.file_type().is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

fn kotlin_package_name(prefix: Option<&str>, package: &str) -> String {
    let crate_name = package.replace('-', "_");
    match prefix
        .map(|prefix| prefix.trim_end_matches('.'))
        .filter(|prefix| !prefix.is_empty())
    {
        Some(prefix) => format!("{prefix}.{crate_name}"),
        None => crate_name,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        collect_kotlin_operation_decls, create_staging_directory, kotlin_operation_registry,
        kotlin_package_name, kotlin_runtime_source, publish_staged_workspace, remove_path,
    };

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn kotlin_operation_declarations_become_attribute_paths() {
        let member = crate::kt_parser::KtDecl {
            kind: crate::kt_parser::KtDeclKind::Function,
            name: "unwrapOr".to_string(),
            type_params: Vec::new(),
            receiver: None,
            params: Vec::new(),
            return_type: None,
            supertypes: Vec::new(),
            is_mutable: false,
            members: Vec::new(),
            op_class: None,
            op_method: Some("unwrap_or".to_string()),
            op_func: None,
        };
        let class = crate::kt_parser::KtDecl {
            kind: crate::kt_parser::KtDeclKind::Class,
            name: "OptionBox".to_string(),
            type_params: Vec::new(),
            receiver: None,
            params: Vec::new(),
            return_type: None,
            supertypes: Vec::new(),
            is_mutable: false,
            members: vec![member],
            op_class: Some("Option".to_string()),
            op_method: None,
            op_func: None,
        };
        let mut registry = fp_core::lang::LangItemRegistry::default();
        collect_kotlin_operation_decls(&[class], &["kotlin".to_string()], &mut registry);
        assert!(
            registry
                .resolve_operation(fp_core::lang::OperationSelector::DeclarationKey(
                    "Option.unwrap_or"
                ))
                .is_some()
        );
        assert!(registry.get_op_path("unwrap_or").is_none());
    }

    #[test]
    fn vendored_kotlin_std_registers_native_portable_operations() {
        let registry = kotlin_operation_registry().expect("load Kotlin std operations");
        for key in [
            "Any.to_string",
            "str.to_string",
            "str.starts_with",
            "str.ends_with",
            "str.trim",
            "str.trim_start",
            "str.trim_end",
        ] {
            assert!(
                registry
                    .resolve_operation(fp_core::lang::OperationSelector::DeclarationKey(key))
                    .is_some(),
                "missing Kotlin std operation declaration: {key}"
            );
        }
    }

    fn test_workspace_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fp-kotlin-{name}-{}-{}",
            std::process::id(),
            TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn incomplete_generation_leaves_existing_workspace_untouched() {
        let workspace_root = test_workspace_root("incomplete-output");
        std::fs::create_dir_all(&workspace_root).expect("create old workspace");
        let old_file = workspace_root.join("previous.kt");
        std::fs::write(&old_file, "previous generation").expect("write old workspace");

        let staging_root = create_staging_directory(&workspace_root).expect("create staging");
        std::fs::write(staging_root.join("partial.kt"), "partial generation")
            .expect("write staged output");

        assert_eq!(
            std::fs::read_to_string(&old_file).expect("read old workspace"),
            "previous generation"
        );
        assert!(!workspace_root.join("partial.kt").exists());

        remove_path(&staging_root).expect("remove abandoned staging");
        remove_path(&workspace_root).expect("remove test workspace");
    }

    #[test]
    fn completed_generation_replaces_existing_workspace() {
        let workspace_root = test_workspace_root("published-output");
        std::fs::create_dir_all(&workspace_root).expect("create old workspace");
        std::fs::write(workspace_root.join("previous.kt"), "previous generation")
            .expect("write old workspace");

        let staging_root = create_staging_directory(&workspace_root).expect("create staging");
        std::fs::write(staging_root.join("current.kt"), "current generation")
            .expect("write staged output");
        publish_staged_workspace(&staging_root, &workspace_root).expect("publish staging");

        assert!(!workspace_root.join("previous.kt").exists());
        assert_eq!(
            std::fs::read_to_string(workspace_root.join("current.kt")).expect("read output"),
            "current generation"
        );

        remove_path(&workspace_root).expect("remove test workspace");
    }

    #[test]
    fn kotlin_package_name_normalizes_cargo_names_under_prefix() {
        assert_eq!(
            kotlin_package_name(Some("com.example.generated"), "skln-git"),
            "com.example.generated.skln_git"
        );
        assert_eq!(kotlin_package_name(None, "skln-git"), "skln_git");
    }

    #[test]
    fn runtime_template_contains_portable_adapters() {
        let runtime = kotlin_runtime_source(None);
        for helper in [
            "fun <T> listPush",
            "fun <T> listExtend",
            "fun <T> resultSuccess",
            "fun <T, R> mapResult",
            "fun pathExists",
            "fun commandSpawn",
            "fun childKill",
            "fun exitStatusSuccess",
        ] {
            assert!(runtime.contains(helper), "missing runtime helper: {helper}");
        }
    }
}
impl TargetBackend for KotlinBackend {
    fn plan(&self) -> fp_core::backend::BackendPlan {
        fp_core::backend::BackendPlan::transpile()
    }
    fn emit(&self, context: &fp_core::backend::BackendContext) -> fp_core::error::Result<()> {
        for package_id in &context.emitted_packages {
            let mir = context
                .mir_program
                .package(package_id)
                .map(|package| {
                    let package = package.borrow();
                    let mut unit = fp_core::mir::MirCodeUnit::new();
                    unit.items.extend(package.items().cloned());
                    unit.bodies
                        .extend(package.bodies().map(|(id, body)| (*id, body.clone())));
                    unit
                })
                .unwrap_or_else(fp_core::mir::MirCodeUnit::new);
            let lir = context.lir_program.merged_blob_for_package(package_id).ok();
            self.emit_package(context, package_id, &mir, lir.as_ref())?;
        }
        self.write_workspace(context.ast_program.as_ref(), &context.hir_program.borrow())
    }

    fn capabilities(&self) -> fp_core::capabilities::LanguageCapabilities {
        crate::CAPABILITIES
    }

    fn portable_operation_registry(&self) -> Option<fp_core::lang::LangItemRegistry> {
        kotlin_operation_registry()
    }
}

impl KotlinBackend {
    fn emit_package(
        &self,
        context: &fp_core::backend::BackendContext,
        package_id: &fp_core::ast::package::PackageId,
        mir: &fp_core::mir::MirCodeUnit,
        lir: Option<&fp_core::lir::LirBlob>,
    ) -> fp_core::error::Result<()> {
        let output_root = self.output_root()?;
        let workspace = context.ast_program.as_ref();
        let scan = self.ensure_scan(workspace)?;
        // Build a fresh AST package from HIR for this backend. The compiled
        // provider package is treated as immutable source metadata only; no
        // backend state is reassigned or mutated.
        let source_package = workspace.package_source(package_id)?;
        let hir_program = context.hir_program.borrow();
        let hir_package = hir_program.package(package_id).ok_or_else(|| {
            fp_core::error::Error::from(format!("HIR package `{package_id}` is unavailable"))
        })?;
        let lifter = fp_backend::transforms::HirToAstLifter::new(&hir_package, &hir_program)
            .with_capabilities(crate::CAPABILITIES);
        let lifter = if let Some(operations) = self.portable_operation_registry() {
            lifter.with_target_operations(operations)
        } else {
            lifter
        };
        let lifter = if let Some(operations) = context.source_operations.clone() {
            lifter.with_source_operations(operations)
        } else {
            lifter
        };
        let package = lifter.lift_package(&source_package)?;
        let files = self.serializer.serialize_package(
            &package,
            &scan.workspace_packages,
            &scan.kotlin_packages,
            &scan.ctx,
        )?;
        let writer = PackageWriter::new(output_root.join(&package.name));
        for (mod_path, code) in files {
            let rel = if mod_path.contains('.') {
                mod_path
            } else {
                format!("{}.kt", mod_path)
            };
            writer.write_file(&rel, code)?;
        }
        Ok(())
    }

    fn write_workspace(
        &self,
        workspace: &fp_core::ast::program::AstProgram,
        _hir_program: &fp_core::hir::HirProgram,
    ) -> fp_core::error::Result<()> {
        let output_root = self.output_root()?;
        let scan = self.ensure_scan(workspace)?;
        let root_name = self.config.root_name.replace('-', "_");
        let mut projects = vec![format!("include(\":{RUNTIME_PROJECT}\")")];
        projects.extend(
            scan.package_names
                .iter()
                .map(|name| format!("include(\":{name}\")")),
        );
        let settings = format!(
            "rootProject.name = \"{root_name}\"\n\n{}\n",
            projects.join("\n")
        );
        let writer = PackageWriter::new(output_root.to_path_buf());
        writer.write_file("settings.gradle.kts", settings)?;
        writer.write_file(
            "build.gradle.kts",
            "plugins {\n    kotlin(\"jvm\") version \"2.1.0\" apply false\n}\n\n\
             allprojects {\n    repositories { mavenCentral() }\n}\n",
        )?;
        writer.write_file(
            &format!("{RUNTIME_PROJECT}/build.gradle.kts"),
            runtime_build_gradle(),
        )?;
        writer.write_file(
            &format!("{RUNTIME_PROJECT}/src/main/kotlin/runtime.kt"),
            kotlin_runtime_source(self.config.kotlin_package_prefix.as_deref()),
        )?;
        self.publish_output()
    }
}

fn runtime_build_gradle() -> &'static str {
    "plugins {\n    kotlin(\"jvm\") version \"2.1.0\"\n    kotlin(\"plugin.serialization\") version \"2.1.0\"\n}\n\n\
     repositories {\n    mavenCentral()\n}\n\n\
     dependencies {\n\
         implementation(\"org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0\")\n\
         implementation(\"org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3\")\n\
         implementation(\"org.tomlj:tomlj:1.1.1\")\n\
         implementation(\"com.fasterxml.jackson.module:jackson-module-kotlin:2.18.2\")\n\
     }\n\n\
     kotlin {\n    jvmToolchain(21)\n}\n"
}

fn kotlin_runtime_source(prefix: Option<&str>) -> String {
    let package = match prefix
        .map(|prefix| prefix.trim_end_matches('.'))
        .filter(|prefix| !prefix.is_empty())
    {
        Some(prefix) => format!("{prefix}.runtime"),
        None => "runtime".to_owned(),
    };
    format!(
        "package {package}\n\n\
         import com.fasterxml.jackson.core.type.TypeReference\n\
         import com.fasterxml.jackson.databind.ObjectMapper\n\
         import com.fasterxml.jackson.module.kotlin.jacksonObjectMapper\n\
         import java.net.Socket\n\
         import java.nio.charset.StandardCharsets\n\
         import kotlinx.coroutines.Dispatchers\n\
         import kotlinx.coroutines.delay\n\
         import kotlinx.coroutines.withContext\n\n\
         object RustKotlinRuntime {{\n\
             @PublishedApi internal val mapper: ObjectMapper = jacksonObjectMapper()\n\
             fun decodeUtf8(bytes: ByteArray): String = bytes.toString(StandardCharsets.UTF_8)\n\
             fun encodeUtf8(value: String): ByteArray = value.toByteArray(StandardCharsets.UTF_8)\n\
             fun appendByte(bytes: ByteArray, byte: Byte): ByteArray = bytes + byte\n\
             fun appendBytes(bytes: ByteArray, suffix: ByteArray): ByteArray = bytes + suffix\n\
             fun repeatByte(byte: Byte, count: Int): ByteArray = ByteArray(count) {{ byte }}\n\
             fun <T> listPush(values: MutableList<T>, value: T): MutableList<T> = values.apply {{ add(value) }}\n\
             fun <T> listExtend(values: MutableList<T>, suffix: Iterable<T>): MutableList<T> = values.apply {{ addAll(suffix) }}\n\
             fun <T> mutableListFromIterable(values: Iterable<T>): MutableList<T> = values.toMutableList()\n\
             fun bytesFromIterable(values: Iterable<Byte>): ByteArray = values.toList().toByteArray()\n\
             fun bytesFromIterable(values: ByteArray): ByteArray = values.copyOf()\n\
             fun <T> filterIterable(values: Iterable<T>, predicate: (T) -> Boolean): MutableList<T> = values.filter(predicate).toMutableList()\n\
             fun splitWhitespace(value: String): MutableList<String> = value.trim().split(Regex(\"\\\\s+\")).filter {{ it.isNotEmpty() }}.toMutableList()\n\
             fun splitString(value: String, delimiter: Char): MutableList<String> = value.split(delimiter).toMutableList()\n\
             fun splitString(value: String, delimiter: String): MutableList<String> = value.split(delimiter).toMutableList()\n\
             fun stringLines(value: String): MutableList<String> = value.lines().toMutableList()\n\
             fun charIndices(value: String): MutableList<Pair<Int, Char>> = value.withIndex().map {{ Pair(it.index, it.value) }}.toMutableList()\n\
             fun splitAt(value: String, index: Long): Pair<String, String> = Pair(value.substring(0, index.toInt()), value.substring(index.toInt()))\n\
             fun stripPrefix(value: String, prefix: String): String? = value.takeIf {{ it.startsWith(prefix) }}?.removePrefix(prefix)\n\
             fun <T> thenSome(condition: Boolean, value: T): T? = if (condition) value else null\n\
             fun <T, R> findMap(values: Iterable<T>, transform: (T) -> R?): R? = values.firstNotNullOfOrNull(transform)\n\
             fun rangeInclusiveContains(range: Any?, value: Long): Boolean = when (range) {{ is ClosedRange<*> -> (range.start as? Long)?.let {{ start -> (range.endInclusive as? Long)?.let {{ end -> value in start..end }} }} ?: false; else -> false }}\n\
             fun readDirectory(path: java.nio.file.Path): Result<List<DirEntry>> = runCatching {{\n\
                 java.nio.file.Files.list(path).use {{ entries -> entries.map(::DirEntry).toList() }}\n\
             }}\n\
             fun createDirectory(path: java.nio.file.Path): Result<Unit> = runCatching<Unit> {{ java.nio.file.Files.createDirectory(path); Unit }}\n\
             fun createDirectories(path: java.nio.file.Path): Result<Unit> = runCatching<Unit> {{ java.nio.file.Files.createDirectories(path); Unit }}\n\
             fun createFile(path: java.nio.file.Path): Result<java.io.OutputStream> = runCatching {{ java.nio.file.Files.newOutputStream(path, java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.TRUNCATE_EXISTING, java.nio.file.StandardOpenOption.WRITE) }}\n\
             fun canonicalize(path: java.nio.file.Path): Result<java.nio.file.Path> = runCatching {{ path.toRealPath() }}\n\
             fun writeAll(stream: java.io.OutputStream, bytes: ByteArray): Result<Unit> = runCatching<Unit> {{ stream.write(bytes); Unit }}\n\
             inline fun <reified T> jsonFromString(input: String): Result<T> = runCatching {{ mapper.readValue(input, object : TypeReference<T>() {{}}) }}\n\
             fun jsonToString(value: Any?): Result<String> = runCatching {{ mapper.writeValueAsString(value) }}\n\
             inline fun <reified T> tomlFromString(input: String): Result<T> = runCatching {{ mapper.convertValue(org.tomlj.Toml.parse(input).toMap(), object : TypeReference<T>() {{}}) }}\n\
             suspend fun tcpConnect(address: String): Result<Socket> = runCatching {{ withContext(Dispatchers.IO) {{ val separator = address.lastIndexOf(':'); require(separator > 0) {{ \"TCP address must be host:port\" }}; Socket(address.substring(0, separator), address.substring(separator + 1).toInt()) }} }}\n\
             suspend fun tcpWriteAll(stream: Socket, bytes: ByteArray): Result<Unit> = runCatching<Unit> {{ withContext(Dispatchers.IO) {{ stream.getOutputStream().write(bytes); Unit }} }}\n\
             suspend fun sleep(duration: java.time.Duration) {{ delay(duration.toMillis()) }}\n\
             fun normalizeError(error: Any?): Throwable = error as? Throwable ?: IllegalStateException(error?.toString() ?: \"unknown error\")\n\
             fun ioError(error: Any?): java.io.IOException = when (error) {{\n\
                 is java.io.IOException -> error\n\
                 is Throwable -> java.io.IOException(error.message, error)\n\
                 else -> java.io.IOException(error?.toString() ?: \"unknown I/O error\")\n\
             }}\n\
             fun <T : Any> optionUnwrap(value: T?): T = requireNotNull(value)\n\
             @Suppress(\"UNCHECKED_CAST\")\n\
             fun <T> resultSuccess(value: T): Result<T> = when (value) {{\n\
                 is Result<*> -> value as Result<T>\n\
                 else -> Result.success(value)\n\
             }}\n\
             fun <T> resultFailure(error: Any?): Result<T> = Result.failure(normalizeError(error))\n\
             fun <T, R> mapResult(result: Result<T>, transform: (T) -> R): Result<R> = result.map(transform)\n\
             fun <T> resultIsSuccess(result: Result<T>): Boolean = result.exceptionOrNull() == null\n\
             fun <T> resultIsFailure(result: Result<T>): Boolean = result.exceptionOrNull() != null\n\
             fun <T> resultOkValue(result: Result<T>): T? = result.getOrNull()\n\
             fun <T> resultErrValue(result: Result<T>): Throwable? = result.exceptionOrNull()\n\
             fun <T> resultException(result: Result<T>): Throwable = requireNotNull(result.exceptionOrNull())\n\
             fun <T> resultUnwrap(result: Result<T>): T = result.getOrNull() ?: throw resultException(result)\n\
             fun <T> resultDefault(result: Result<T>, defaultValue: T): T = result.getOrElse {{ defaultValue }}\n\
             inline fun <reified T> parse(input: String): Result<T> = runCatching {{ when (T::class) {{ Int::class -> input.toInt(); Long::class -> input.toLong(); Short::class -> input.toShort(); Byte::class -> input.toByte(); Double::class -> input.toDouble(); Float::class -> input.toFloat(); Boolean::class -> input.toBooleanStrict(); String::class -> input; else -> error(\"unsupported Rust FromStr target: ${{T::class.qualifiedName}}\") }} as T }}\n\
             fun <T> unwrapOr(value: T?, defaultValue: T): T = value ?: defaultValue\n\
             fun <T, R> mapOr(value: T?, defaultValue: R, transform: (T) -> R): R = value?.let(transform) ?: defaultValue\n\
             fun <T> mapError(result: Result<T>, transform: (Throwable) -> Throwable): Result<T> {{\n\
                 val error = result.exceptionOrNull() ?: return result\n\
                 return Result.failure(transform(error))\n\
             }}\n\
             fun pathExists(path: java.nio.file.Path): Boolean = java.nio.file.Files.exists(path)\n\
             fun pathResolve(path: java.nio.file.Path, other: Any?): java.nio.file.Path = path.resolve(other.toString())\n\
             fun deleteRecursively(path: java.nio.file.Path) {{\n\
                 if (!java.nio.file.Files.exists(path)) return\n\
                 java.nio.file.Files.walk(path).use {{ paths ->\n\
                     paths.sorted(java.util.Comparator.reverseOrder()).forEach(java.nio.file.Files::delete)\n\
                 }}\n\
             }}\n\
             enum class Stdio {{ PIPED, INHERIT, NULL }}\n\
             fun pipedStdio(): Stdio = Stdio.PIPED\n\
             fun inheritStdio(): Stdio = Stdio.INHERIT\n\
             fun nullStdio(): Stdio = Stdio.NULL\n\
             fun command(program: String): Command = Command(program)\n\
             fun commandArg(command: Command, value: Any?): Command = command.arg(value)\n\
             fun commandArgs(command: Command, values: Iterable<*>): Command = command.args(values)\n\
             fun commandCurrentDir(command: Command, path: java.nio.file.Path): Command = command.currentDir(path)\n\
             fun commandStdin(command: Command, value: Stdio): Command = command.stdin(value)\n\
             fun commandStdout(command: Command, value: Stdio): Command = command.stdout(value)\n\
             fun commandStderr(command: Command, value: Stdio): Command = command.stderr(value)\n\
             fun commandSpawn(command: Command): Result<Child> = runCatching {{ command.spawn() }}\n\
             fun commandOutput(command: Command): Result<Output> = runCatching {{ command.output() }}\n\
             fun commandStatus(command: Command): Result<ExitStatus> = runCatching {{ command.status() }}\n\
             fun childKill(child: Child): Result<Unit> = runCatching<Unit> {{ child.kill(); Unit }}\n\
             fun childWait(child: Child): Result<ExitStatus> = runCatching {{ child.waitForStatus() }}\n\
             fun childTryWait(child: Child): Result<ExitStatus?> = runCatching {{ child.tryWait() }}\n\
             fun childWaitWithOutput(child: Child): Result<Output> = runCatching {{ child.waitWithOutput() }}\n\
             fun exitStatusSuccess(status: ExitStatus): Boolean = status.success()\n\
             class ExitStatus(private val code: Int) {{\n\
                 fun success(): Boolean = code == 0\n\
                 override fun toString(): String = code.toString()\n\
             }}\n\
             class FileType(private val attributes: java.nio.file.attribute.BasicFileAttributes) {{\n\
                 fun isDirectory(): Boolean = attributes.isDirectory\n\
             }}\n\
             class DirEntry(private val value: java.nio.file.Path) {{\n\
                 fun path(): java.nio.file.Path = value\n\
                 fun fileType(): FileType = FileType(java.nio.file.Files.readAttributes(value, java.nio.file.attribute.BasicFileAttributes::class.java))\n\
             }}\n\
             data class Output(val status: ExitStatus, val stdout: ByteArray, val stderr: ByteArray)\n\
             class ChildStdin(private val stream: java.io.OutputStream) {{\n\
                 fun write(bytes: ByteArray): Result<Int> = runCatching {{ stream.write(bytes); bytes.size }}\n\
                 fun write(bytes: Iterable<*>): Result<Int> = runCatching {{\n\
                     val materialized = bytes.map {{ (it as Number).toByte() }}.toByteArray()\n\
                     stream.write(materialized)\n\
                     materialized.size\n\
                 }}\n\
                 fun close() {{ stream.close() }}\n\
             }}\n\
             class ChildStdinSlot(stream: java.io.OutputStream?) {{\n\
                 private var stream: java.io.OutputStream? = stream\n\
                 fun take(): ChildStdin? = stream?.let {{ ChildStdin(it).also {{ stream = null }} }}\n\
                 fun close() {{ stream?.close(); stream = null }}\n\
             }}\n\
             class Child(private val process: Process) {{\n\
                 val stdin = ChildStdinSlot(process.outputStream)\n\
                 fun kill(): Unit = process.destroyForcibly().let {{ }}\n\
                 fun waitForStatus(): ExitStatus {{ stdin.close(); return ExitStatus(process.waitFor()) }}\n\
                 fun wait(): ExitStatus = waitForStatus()\n\
                 fun tryWait(): ExitStatus? = if (process.isAlive) null else ExitStatus(process.exitValue())\n\
                 fun waitWithOutput(): Output {{\n\
                     stdin.close()\n\
                     val stdout = process.inputStream.readBytes()\n\
                     val stderr = process.errorStream.readBytes()\n\
                     return Output(ExitStatus(process.waitFor()), stdout, stderr)\n\
                 }}\n\
             }}\n\
             class Command(private val program: String) {{\n\
                 private val builder = ProcessBuilder(program)\n\
                 fun arg(value: Any?): Command = apply {{ builder.command().add(value.toString()) }}\n\
                 fun args(values: Iterable<*>): Command = apply {{ values.forEach {{ builder.command().add(it.toString()) }} }}\n\
                 fun currentDir(path: java.nio.file.Path): Command = apply {{ builder.directory(path.toFile()) }}\n\
                 fun stdin(value: Stdio): Command = apply {{ builder.redirectInput(value.redirect()) }}\n\
                 fun stdout(value: Stdio): Command = apply {{ builder.redirectOutput(value.redirect()) }}\n\
                 fun stderr(value: Stdio): Command = apply {{ builder.redirectError(value.redirect()) }}\n\
                 fun spawn(): Child = Child(builder.start())\n\
                 fun output(): Output {{\n\
                     builder.redirectInput(ProcessBuilder.Redirect.PIPE)\n\
                     builder.redirectOutput(ProcessBuilder.Redirect.PIPE)\n\
                     builder.redirectError(ProcessBuilder.Redirect.PIPE)\n\
                     val process = builder.start()\n\
                     val stdout = process.inputStream.readBytes()\n\
                     val stderr = process.errorStream.readBytes()\n\
                     return Output(ExitStatus(process.waitFor()), stdout, stderr)\n\
                 }}\n\
                 fun status(): ExitStatus = ExitStatus(builder.start().waitFor())\n\
             }}\n\
             private fun Stdio.redirect(): ProcessBuilder.Redirect = when (this) {{\n\
                 Stdio.PIPED -> ProcessBuilder.Redirect.PIPE\n\
                 Stdio.INHERIT -> ProcessBuilder.Redirect.INHERIT\n\
                 Stdio.NULL -> ProcessBuilder.Redirect.DISCARD\n\
             }}\n\
         }}\n"
    )
}
