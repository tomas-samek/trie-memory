pub mod concept;
pub mod layer;
pub mod memory;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub use memory::{source_type_default_trust, MemoryEntry, Origin, SourceType};

/// FNV-1a hash over a path-key vector. Mirrors the hash used in `store::concept`.
/// None segments (Learning nodes) fold in as 0 — still deterministic but
/// collapses all Learning-only paths to the same bucket.
pub fn hash_path_key(key: &[Option<u64>]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut hash = FNV_OFFSET;
    for segment in key {
        let val = segment.unwrap_or(0);
        hash ^= val;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// All non-empty suffixes of a path key, deepest-first.
/// For `[leaf, d1, d0]` returns `[[leaf, d1, d0], [d1, d0], [d0]]`.
/// Used to index a memory at every ancestor level so queries that match at a
/// shallower depth can still retrieve the memory.
fn path_suffixes(key: &[Option<u64>]) -> Vec<Vec<Option<u64>>> {
    (0..key.len()).map(|i| key[i..].to_vec()).collect()
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ContentStore {
    /// Flat storage for all memory entries.
    #[serde(default)]
    entries: Vec<MemoryEntry>,
    /// path-key-hash → indices into `entries`. Multi-value because a single
    /// memory is indexed at every ancestor level of its path.
    #[serde(default)]
    path_index: HashMap<u64, Vec<usize>>,
    /// Next id to assign.
    #[serde(default)]
    next_id: u64,

    /// Legacy field: memories from pre-v2 snapshots keyed by raw node_id.
    /// Kept for backward-compat loading only; never written to.
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    entries_legacy: HashMap<u64, Vec<MemoryEntry>>,
}

impl ContentStore {
    pub fn new() -> Self {
        Default::default()
    }

    /// Store a memory at every ancestor-level suffix of its `path_content_ids`.
    /// Assigns a monotonic id and sets it on the entry before storing.
    /// If the entry has no path_content_ids, it is stored under a single
    /// `legacy_origin`-style bucket (hash of empty key).
    ///
    /// If the entry's `origin.corrects` points to a prior entry, that
    /// prior entry's `revised_by` is stamped to this new id — establishing
    /// the bidirectional correction link used by the Conflicted mode
    /// detector.
    pub fn add_by_path(&mut self, mut entry: MemoryEntry) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        entry.id = id;
        let corrects_target = entry.origin.as_ref().and_then(|o| o.corrects);
        let idx = self.entries.len();
        let suffixes = if entry.path_content_ids.is_empty() {
            vec![Vec::new()]
        } else {
            path_suffixes(&entry.path_content_ids)
        };
        self.entries.push(entry);
        for suffix in suffixes {
            let key = hash_path_key(&suffix);
            self.path_index.entry(key).or_default().push(idx);
        }
        if let Some(target_id) = corrects_target {
            self.stamp_revision(target_id, id);
        }
        id
    }

    /// Find the entry with the given id and mark it revised_by the given
    /// corrector id. Silently no-ops if the target is missing — the caller
    /// is responsible for supplying a real id.
    fn stamp_revision(&mut self, target_id: u64, corrector_id: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == target_id) {
            entry.revised_by = Some(corrector_id);
        }
    }

    /// Lookup by id for diagnostics / tests.
    pub fn get_by_id(&self, id: u64) -> Option<&MemoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Legacy API — kept so older call sites compile. Stores a memory under a
    /// single raw u64 key in the legacy map. New code should use `add_by_path`.
    pub fn add(&mut self, key: u64, mut entry: MemoryEntry) {
        if entry.id == 0 {
            entry.id = self.next_id;
            self.next_id += 1;
        }
        self.entries_legacy.entry(key).or_default().push(entry);
    }

    /// Recall by path key. Tries the exact path and every shallower ancestor
    /// suffix. Results are deduplicated by entry id and sorted newest-first.
    pub fn recall_by_path(
        &self,
        path_key: &[Option<u64>],
        max_results: usize,
    ) -> Vec<&MemoryEntry> {
        let mut seen: HashSet<u64> = HashSet::new();
        let mut results: Vec<&MemoryEntry> = Vec::new();

        // Shallower suffixes first? No — deepest (most specific) first so exact
        // matches win when we later sort stably.
        let suffixes = if path_key.is_empty() {
            vec![Vec::new()]
        } else {
            path_suffixes(path_key)
        };

        for suffix in &suffixes {
            let key = hash_path_key(suffix);
            if let Some(indices) = self.path_index.get(&key) {
                for &idx in indices {
                    if let Some(entry) = self.entries.get(idx) {
                        if seen.insert(entry.id) {
                            results.push(entry);
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        results.truncate(max_results);
        results
    }

    /// Legacy recall API — u64 key + parent u64 keys (raw node ids).
    /// Used only for entries stored via the legacy `add` path.
    pub fn recall(&self, key: u64, parent_keys: &[u64], max_results: usize) -> Vec<&MemoryEntry> {
        let mut results: Vec<&MemoryEntry> = Vec::new();

        if let Some(entries) = self.entries_legacy.get(&key) {
            results.extend(entries.iter());
        }

        for &parent_key in parent_keys {
            if parent_key == key {
                continue;
            }
            if let Some(entries) = self.entries_legacy.get(&parent_key) {
                results.extend(entries.iter());
            }
        }

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        results.truncate(max_results);
        results
    }

    pub fn save(&self, path: &str) -> std::io::Result<usize> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let len = json.len();
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, &json)?;
        Ok(len)
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut store: Self = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Ensure next_id is past any id we just loaded.
        let max_id = store
            .entries
            .iter()
            .chain(store.entries_legacy.values().flat_map(|v| v.iter()))
            .map(|e| e.id)
            .max()
            .unwrap_or(0);
        if store.next_id <= max_id {
            store.next_id = max_id + 1;
        }

        // Mark all entries that lack provenance as legacy_origin.
        for e in store.entries.iter_mut() {
            if e.observer_id.is_none() && e.origin.is_none() && e.trust_level == 0 {
                e.legacy_origin = true;
            }
        }
        for v in store.entries_legacy.values_mut() {
            for e in v.iter_mut() {
                e.legacy_origin = true;
            }
        }

        Ok(store)
    }

    pub fn exists(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
            + self.entries_legacy.values().map(|v| v.len()).sum::<usize>()
    }

    pub fn distinct_path_keys(&self) -> usize {
        self.path_index.len()
    }
}
