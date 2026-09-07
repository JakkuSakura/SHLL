//! Process-independent compiler artifact cache.

use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAGIC: &[u8] = b"FP-CACHE\0";
const FORMAT_VERSION: u8 = 2;

#[derive(Clone, Debug)]
pub struct CacheConfig {
    pub root: PathBuf,
    pub schema_version: u32,
    pub enabled: bool,
}

impl CacheConfig {
    pub fn for_project(project_root: impl AsRef<Path>) -> Self {
        Self {
            root: project_root.as_ref().join("target/fp"),
            schema_version: 1,
            enabled: true,
        }
    }

    /// Temporary project-root policy: the compiler is currently invoked with
    /// the project root as its working directory. Replace this with an
    /// explicit session/project resolver when that context is available.
    pub fn for_current_project() -> Self {
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::for_project(project_root)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey(String);

impl CacheKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheStage {
    Ast,
    Hir,
    Mir,
    Lir,
}

impl CacheStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ast => "ast",
            Self::Hir => "hir",
            Self::Mir => "mir",
            Self::Lir => "lir",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("cache I/O failed at {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("cache serialization failed: {0}")]
    Serialization(String),
}

#[derive(Clone, Debug)]
pub struct CacheProvider {
    config: CacheConfig,
}

impl CacheProvider {
    pub fn new(config: CacheConfig) -> Self {
        Self { config }
    }
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }

    pub fn load<T: DeserializeOwned>(&self, key: &CacheKey) -> Result<Option<T>, CacheError> {
        let Some(bytes) = self.load_bytes(key)? else {
            return Ok(None);
        };
        match bincode::deserialize(&bytes) {
            Ok(value) => Ok(Some(value)),
            Err(_) => match serde_json::from_slice(&bytes) {
                Ok(value) => Ok(Some(value)),
                Err(_) => Ok(None),
            },
        }
    }

    pub fn save<T: Serialize>(&self, key: &CacheKey, value: &T) -> Result<(), CacheError> {
        let bytes = match bincode::serialize(value) {
            Ok(bytes) => bytes,
            Err(binary_error) => serde_json::to_vec(value).map_err(|json_error| {
                CacheError::Serialization(format!(
                    "binary codec: {binary_error}; JSON codec: {json_error}"
                ))
            })?,
        };
        self.save_bytes(key, &bytes)
    }

    pub fn load_bytes(&self, key: &CacheKey) -> Result<Option<Vec<u8>>, CacheError> {
        if !self.config.enabled {
            return Ok(None);
        }
        let path = self.path_for(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(CacheError::Io { path, source }),
        };
        let header_len = MAGIC.len() + 1 + 8;
        if bytes.len() < header_len || &bytes[..MAGIC.len()] != MAGIC {
            let _ = fs::remove_file(&path);
            return Ok(None);
        }
        if bytes[MAGIC.len()] != FORMAT_VERSION {
            let _ = fs::remove_file(&path);
            return Ok(None);
        }
        let mut key_len_bytes = [0u8; 8];
        key_len_bytes.copy_from_slice(&bytes[MAGIC.len() + 1..header_len]);
        let Ok(key_len) = usize::try_from(u64::from_le_bytes(key_len_bytes)) else {
            let _ = fs::remove_file(&path);
            return Ok(None);
        };
        let Some(payload_start) = header_len.checked_add(key_len) else {
            let _ = fs::remove_file(&path);
            return Ok(None);
        };
        if payload_start > bytes.len()
            || bytes[header_len..payload_start] != *key.as_str().as_bytes()
        {
            let _ = fs::remove_file(&path);
            return Ok(None);
        }
        Ok(Some(bytes[payload_start..].to_vec()))
    }

    pub fn save_bytes(&self, key: &CacheKey, payload: &[u8]) -> Result<(), CacheError> {
        if !self.config.enabled {
            return Ok(());
        }
        fs::create_dir_all(&self.config.root).map_err(|source| CacheError::Io {
            path: self.config.root.clone(),
            source,
        })?;
        let path = self.path_for(key);
        let temp = path.with_extension(format!("tmp-{}-{}", std::process::id(), unique_suffix()));
        let mut bytes =
            Vec::with_capacity(MAGIC.len() + 1 + 8 + key.as_str().len() + payload.len());
        bytes.extend_from_slice(MAGIC);
        bytes.push(FORMAT_VERSION);
        bytes.extend_from_slice(&(key.as_str().len() as u64).to_le_bytes());
        bytes.extend_from_slice(key.as_str().as_bytes());
        bytes.extend_from_slice(payload);
        fs::write(&temp, bytes).map_err(|source| CacheError::Io {
            path: temp.clone(),
            source,
        })?;
        fs::rename(&temp, &path).map_err(|source| {
            let _ = fs::remove_file(&temp);
            CacheError::Io { path, source }
        })
    }

    pub fn clear(&self) -> Result<(), CacheError> {
        match fs::remove_dir_all(&self.config.root) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(CacheError::Io {
                path: self.config.root.clone(),
                source,
            }),
        }
    }

    fn path_for(&self, key: &CacheKey) -> PathBuf {
        let mut digest = Sha256::new();
        digest.update(self.config.schema_version.to_le_bytes());
        digest.update(key.as_str().as_bytes());
        self.config
            .root
            .join(format!("{:x}.bin", digest.finalize()))
    }
}

fn unique_suffix() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

pub fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Computes a deterministic digest for a serde artifact. Compiler stage
/// keys must not use `Debug` output: map iteration order is not stable across
/// processes, while the serialized representation is the artifact's actual
/// cache input.
pub fn digest_serializable<T: Serialize>(value: &T) -> Result<String, CacheError> {
    let bytes = match bincode::serialize(value) {
        Ok(bytes) => bytes,
        Err(binary_error) => serde_json::to_vec(value).map_err(|json_error| {
            CacheError::Serialization(format!(
                "binary codec: {binary_error}; JSON codec: {json_error}"
            ))
        })?,
    };
    Ok(digest_bytes(&bytes))
}

pub fn stage_key(
    stage: CacheStage,
    package: &str,
    unit: Option<&str>,
    inputs: &[(&str, &str)],
) -> CacheKey {
    let mut key = format!("fp-cache-v1/{}/{}", stage.as_str(), package);
    if let Some(unit) = unit {
        key.push('/');
        key.push_str(unit);
    }
    for (name, value) in inputs {
        key.push('/');
        key.push_str(name);
        key.push('=');
        key.push_str(value);
    }
    CacheKey::new(key)
}

#[cfg(test)]
mod tests {
    use super::{CacheConfig, CacheKey, CacheProvider, digest_bytes, digest_serializable};

    #[test]
    fn round_trips_json_values() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheProvider::new(CacheConfig {
            root: dir.path().into(),
            schema_version: 1,
            enabled: true,
        });
        let key = CacheKey::new("ast/package");
        cache.save(&key, &vec![1u32, 2, 3]).unwrap();
        assert_eq!(cache.load::<Vec<u32>>(&key).unwrap(), Some(vec![1, 2, 3]));
    }

    #[test]
    fn missing_and_corrupt_entries_are_misses() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheProvider::new(CacheConfig {
            root: dir.path().into(),
            schema_version: 1,
            enabled: true,
        });
        assert!(
            cache
                .load_bytes(&CacheKey::new("missing"))
                .unwrap()
                .is_none()
        );
        cache.save_bytes(&CacheKey::new("x"), b"ok").unwrap();
        let path = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(path, b"bad").unwrap();
        assert!(cache.load_bytes(&CacheKey::new("x")).unwrap().is_none());
    }

    #[test]
    fn digest_is_stable() {
        assert_eq!(digest_bytes(b"abc"), digest_bytes(b"abc"));
    }

    #[test]
    fn serializable_digest_is_stable() {
        assert_eq!(
            digest_serializable(&vec![1u32, 2, 3]).unwrap(),
            digest_serializable(&vec![1u32, 2, 3]).unwrap()
        );
    }

    #[test]
    fn project_cache_root_is_target_fp() {
        let config = CacheConfig::for_project("/tmp/example");
        assert_eq!(
            config.root,
            std::path::PathBuf::from("/tmp/example/target/fp")
        );
    }

    #[test]
    fn current_project_cache_root_is_relative_to_working_directory() {
        let config = CacheConfig::for_current_project();
        assert!(config.root.ends_with(std::path::Path::new("target/fp")));
        assert!(
            !config
                .root
                .ends_with(std::path::Path::new("target/target/fp"))
        );
    }

    #[test]
    fn disabled_cache_does_not_touch_disk() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheProvider::new(CacheConfig {
            root: dir.path().join("target/fp"),
            schema_version: 1,
            enabled: false,
        });
        let key = CacheKey::new("disabled");
        cache.save(&key, &42u32).unwrap();
        assert!(cache.load::<u32>(&key).unwrap().is_none());
        assert!(!cache.config().root.exists());
    }

    #[test]
    fn hir_and_mir_artifacts_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheProvider::new(CacheConfig {
            root: dir.path().into(),
            schema_version: 1,
            enabled: true,
        });

        let hir = crate::hir::HirPackage::new(crate::package::PackageId::new("cache-test"));
        let hir_key = CacheKey::new("hir/cache-test");
        cache.save(&hir_key, &hir).unwrap();
        let restored_hir = cache.load::<crate::hir::HirPackage>(&hir_key).unwrap();
        assert_eq!(restored_hir.unwrap().id, hir.id);

        let mir = crate::mir::MirCodeUnit::new();
        let mir_key = CacheKey::new("mir/cache-test");
        cache.save(&mir_key, &mir).unwrap();
        assert_eq!(
            cache.load::<crate::mir::MirCodeUnit>(&mir_key).unwrap(),
            Some(mir)
        );
    }
}
