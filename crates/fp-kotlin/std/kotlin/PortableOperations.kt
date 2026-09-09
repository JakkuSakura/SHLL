/*
 * Portable-operation declarations whose Rust representation has no concrete
 * Kotlin declaration.  The file is consumed by the Kotlin operation registry
 * as metadata; the serializer lowers these nullable constructors to `value`
 * and `null`, respectively.
 */
@Op(class = "Option")
public enum class Option {
    @Op(variant = "none") None,
    @Op(variant = "some") Some;
}

// Runtime-backed filesystem operations are declared here so the destination
// mapping remains attribute-driven. The declaration is metadata only; the
// emitted implementation lives in RustKotlinRuntime.
@Op(func = "fs_read_to_string")
public fun portableFsReadToString(path: Any): Any = TODO()

@Op(func = "env_var")
public fun portableEnvVar(key: Any): Any = TODO()

@Op(func = "env_current_dir")
public fun portableEnvCurrentDir(): Any = TODO()

@Op(func = "env_home_dir")
public fun portableEnvHomeDir(): Any = TODO()

@Op(func = "env_temp_dir")
public fun portableEnvTempDir(): Any = TODO()

@Op(func = "fs_canonicalize")
public fun portableFsCanonicalize(path: Any): Any = TODO()

@Op(func = "fs_create_dir")
public fun portableFsCreateDir(path: Any): Any = TODO()

@Op(func = "fs_create_dir_all")
public fun portableFsCreateDirAll(path: Any): Any = TODO()

@Op(func = "fs_exists")
public fun portableFsExists(path: Any): Any = TODO()

@Op(func = "fs_read")
public fun portableFsRead(path: Any): Any = TODO()

@Op(func = "fs_read_dir")
public fun portableFsReadDir(path: Any): Any = TODO()

@Op(func = "fs_remove_dir_all")
public fun portableFsRemoveDirAll(path: Any): Any = TODO()

@Op(func = "fs_remove_file")
public fun portableFsRemoveFile(path: Any): Any = TODO()

@Op(func = "fs_write_string")
public fun portableFsWriteString(path: Any, content: Any): Any = TODO()
