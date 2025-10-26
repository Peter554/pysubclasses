//! Caching module for parsed Python files.
//!
//! This module implements a file-based cache to avoid re-parsing unchanged Python files.
//! The cache stores serialized `ParsedFile` results along with file metadata (mtime, size)
//! to detect changes.

use crate::{
    error::Result,
    parser::{self, ParsedFile},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// Metadata for a cached file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// Last modified time of the source file
    mtime: SystemTime,
    /// Size of the source file in bytes
    size: u64,
    /// The parsed result
    parsed: ParsedFile,
}

/// The cache structure storing all cached parse results.
#[derive(Debug, Serialize, Deserialize)]
struct Cache {
    /// Version of the cache format (for future compatibility)
    version: u32,
    /// Map from file path to cache entry
    entries: HashMap<PathBuf, CacheEntry>,
}

impl Cache {
    const VERSION: u32 = 1;

    fn new() -> Self {
        Self {
            version: Self::VERSION,
            entries: HashMap::new(),
        }
    }

    fn load(cache_path: &Path) -> Option<Self> {
        let data = fs::read(cache_path).ok()?;
        bincode::deserialize(&data).ok()
    }

    fn save(&self, cache_path: &Path) -> Result<()> {
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = bincode::serialize(self).map_err(std::io::Error::other)?;
        fs::write(cache_path, data)?;
        Ok(())
    }
}

/// Gets the cache file path for a given root directory.
fn get_cache_path(root_dir: &Path) -> PathBuf {
    root_dir.join(".pysubclasses-cache")
}

/// Gets file metadata for cache validation.
fn get_file_metadata(path: &Path) -> Option<(SystemTime, u64)> {
    let metadata = fs::metadata(path).ok()?;
    let mtime = metadata.modified().ok()?;
    let size = metadata.len();
    Some((mtime, size))
}

/// Parses Python files with caching enabled.
///
/// This function checks the cache for each file and only parses files that have changed
/// or are not in the cache.
///
/// # Arguments
///
/// * `root_dir` - The root directory (used for cache location and module path computation)
/// * `python_files` - List of Python files to parse
///
/// # Returns
///
/// A vector of parse results, same as `parse_files`.
pub fn parse_with_cache(
    root_dir: &Path,
    python_files: &[PathBuf],
) -> Result<Vec<Result<ParsedFile>>> {
    let cache_path = get_cache_path(root_dir);

    // Load existing cache
    let mut cache = Cache::load(&cache_path).unwrap_or_else(Cache::new);

    // Check if cache version matches
    if cache.version != Cache::VERSION {
        cache = Cache::new();
    }

    let mut results = Vec::with_capacity(python_files.len());
    let mut files_to_parse = Vec::new();
    let mut cache_hits = 0;
    let mut cache_misses = 0;

    // First pass: check cache
    for file_path in python_files {
        if let Some((mtime, size)) = get_file_metadata(file_path) {
            if let Some(entry) = cache.entries.get(file_path) {
                // Check if file has changed
                if entry.mtime == mtime && entry.size == size {
                    // Cache hit
                    results.push(Ok(entry.parsed.clone()));
                    cache_hits += 1;
                    continue;
                }
            }
        }

        // Cache miss - need to parse
        files_to_parse.push(file_path.clone());
        cache_misses += 1;
    }

    // Parse files that weren't in cache or have changed
    if !files_to_parse.is_empty() {
        let parse_results = parser::parse_files(root_dir, &files_to_parse)?;

        // Update cache and collect results
        for (file_path, result) in files_to_parse.iter().zip(parse_results.into_iter()) {
            match &result {
                Ok(parsed) => {
                    if let Some((mtime, size)) = get_file_metadata(file_path) {
                        cache.entries.insert(
                            file_path.clone(),
                            CacheEntry {
                                mtime,
                                size,
                                parsed: parsed.clone(),
                            },
                        );
                    }
                }
                Err(_) => {
                    // Remove from cache if parse failed
                    cache.entries.remove(file_path);
                }
            }
            results.push(result);
        }
    }

    // Save updated cache
    if cache_misses > 0 {
        if let Err(e) = cache.save(&cache_path) {
            eprintln!("Warning: Failed to save cache: {e}");
        }
    }

    if cache_hits > 0 || cache_misses > 0 {
        eprintln!("Cache: {cache_hits} hits, {cache_misses} misses");
    }

    Ok(results)
}
