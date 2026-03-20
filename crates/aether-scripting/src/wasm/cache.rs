//! Filesystem-backed cache for precompiled Wasmtime modules.
//!
//! After a WASM module is compiled (JIT), the compiled artifact is
//! serialized to `<cache_dir>/<hex_hash>.cwasm`. On subsequent loads
//! the precompiled module is deserialized directly, skipping compilation.

use std::path::{Path, PathBuf};

/// Cache for precompiled Wasmtime modules on disk.
#[derive(Debug, Clone)]
pub struct ModuleCache {
    cache_dir: PathBuf,
}

impl ModuleCache {
    /// Creates a new module cache backed by the given directory.
    ///
    /// The directory is created if it does not exist.
    pub fn new(cache_dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let cache_dir = cache_dir.into();
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Returns the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Returns the path where a module with the given hash would be cached.
    pub fn cache_path(&self, hash: &[u8; 32]) -> PathBuf {
        self.cache_dir.join(format!("{}.cwasm", hex::encode(hash)))
    }

    /// Attempts to load a precompiled module from the cache.
    ///
    /// Returns `None` if no cached artifact exists for this hash.
    /// Returns an error if the cached file exists but deserialization fails
    /// (e.g., engine version mismatch).
    pub fn load(
        &self,
        engine: &wasmtime::Engine,
        hash: &[u8; 32],
    ) -> Result<Option<wasmtime::Module>, wasmtime::Error> {
        let path = self.cache_path(hash);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|e| {
            wasmtime::Error::msg(format!("failed to read cache file {}: {e}", path.display()))
        })?;
        // SAFETY: We trust the cache directory contents. In production this
        // should additionally verify a checksum of the cached artifact.
        let module = unsafe { wasmtime::Module::deserialize(engine, &bytes)? };
        Ok(Some(module))
    }

    /// Stores a compiled module in the cache.
    pub fn store(&self, module: &wasmtime::Module, hash: &[u8; 32]) -> Result<(), wasmtime::Error> {
        let path = self.cache_path(hash);
        let bytes = module.serialize()?;
        std::fs::write(&path, &bytes).map_err(|e| {
            wasmtime::Error::msg(format!(
                "failed to write cache file {}: {e}",
                path.display()
            ))
        })?;
        Ok(())
    }

    /// Removes a cached module for the given hash.
    ///
    /// Returns `true` if a cached file was removed, `false` if none existed.
    pub fn evict(&self, hash: &[u8; 32]) -> bool {
        let path = self.cache_path(hash);
        std::fs::remove_file(&path).is_ok()
    }

    /// Removes all cached modules.
    pub fn clear(&self) -> std::io::Result<()> {
        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "cwasm") {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::verify::sha256_hash;

    fn create_engine() -> wasmtime::Engine {
        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        wasmtime::Engine::new(&config).expect("engine creation")
    }

    fn sample_wasm() -> Vec<u8> {
        wat::parse_str(r#"(module (func (export "run") (nop)))"#).unwrap()
    }

    #[test]
    fn cache_dir_is_created() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("wasm_cache");
        assert!(!cache_dir.exists());

        let cache = ModuleCache::new(&cache_dir).unwrap();
        assert!(cache_dir.exists());
        assert_eq!(cache.cache_dir(), cache_dir);
    }

    #[test]
    fn cache_path_uses_hex_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ModuleCache::new(tmp.path()).unwrap();
        let hash = [0xABu8; 32];
        let path = cache.cache_path(&hash);
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".cwasm"));
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with(&hex::encode([0xABu8; 32])));
    }

    #[test]
    fn load_returns_none_when_no_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ModuleCache::new(tmp.path()).unwrap();
        let engine = create_engine();
        let hash = sha256_hash(b"nonexistent");

        let result = cache.load(&engine, &hash).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn store_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ModuleCache::new(tmp.path()).unwrap();
        let engine = create_engine();

        let wasm = sample_wasm();
        let hash = sha256_hash(&wasm);
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();

        cache.store(&module, &hash).unwrap();
        assert!(cache.cache_path(&hash).exists());

        let loaded = cache.load(&engine, &hash).unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn evict_removes_cached_module() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ModuleCache::new(tmp.path()).unwrap();
        let engine = create_engine();

        let wasm = sample_wasm();
        let hash = sha256_hash(&wasm);
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();

        cache.store(&module, &hash).unwrap();
        assert!(cache.cache_path(&hash).exists());

        assert!(cache.evict(&hash));
        assert!(!cache.cache_path(&hash).exists());
    }

    #[test]
    fn evict_nonexistent_returns_false() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ModuleCache::new(tmp.path()).unwrap();
        let hash = sha256_hash(b"nope");
        assert!(!cache.evict(&hash));
    }

    #[test]
    fn clear_removes_all_cached_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ModuleCache::new(tmp.path()).unwrap();
        let engine = create_engine();

        let wasm_a = wat::parse_str(r#"(module (func (export "a") (nop)))"#).unwrap();
        let wasm_b = wat::parse_str(r#"(module (func (export "b") (nop)))"#).unwrap();
        let hash_a = sha256_hash(&wasm_a);
        let hash_b = sha256_hash(&wasm_b);

        let mod_a = wasmtime::Module::new(&engine, &wasm_a).unwrap();
        let mod_b = wasmtime::Module::new(&engine, &wasm_b).unwrap();
        cache.store(&mod_a, &hash_a).unwrap();
        cache.store(&mod_b, &hash_b).unwrap();

        cache.clear().unwrap();
        assert!(!cache.cache_path(&hash_a).exists());
        assert!(!cache.cache_path(&hash_b).exists());
    }
}
