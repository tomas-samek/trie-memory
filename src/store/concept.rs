use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type ConceptId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub path_key: Vec<Option<u64>>,
    #[serde(default)]
    pub word_path_key: Vec<Option<u64>>,
    pub bound_at: u64,
    pub strength: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: ConceptId,
    pub label: Option<String>,
    pub bindings: Vec<Binding>,
    pub created_at: u64,
    pub access_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConceptStore {
    concepts: Vec<Concept>,
    /// Index: hash of path key suffix → concept IDs.
    path_index: HashMap<u64, Vec<ConceptId>>,
    /// Same shape as `path_index`, but keyed on the word-level trie path.
    #[serde(default)]
    word_path_index: HashMap<u64, Vec<ConceptId>>,
    /// Direct word-hash → concept IDs index.
    /// Key is u16 word hash (from tokenizer::word_token), stored as u64 for serde compat.
    #[serde(default)]
    word_hash_index: HashMap<u64, Vec<ConceptId>>,
}

/// FNV-1a hash for a path key segment slice. Public for use in tool handlers.
pub fn hash_path_key_pub(key: &[Option<u64>]) -> u64 {
    hash_path_key(key)
}

fn index_prefixes(
    index: &mut HashMap<u64, Vec<ConceptId>>,
    path_key: &[Option<u64>],
    concept_id: ConceptId,
) {
    let n = path_key.len();
    for end in 0..n {
        let prefix = &path_key[..=end];
        let h = hash_path_key(prefix);
        index.entry(h).or_default().push(concept_id);
    }
}

fn hash_path_key(key: &[Option<u64>]) -> u64 {
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

impl ConceptStore {
    pub fn new() -> Self {
        Self {
            concepts: Vec::new(),
            path_index: HashMap::new(),
            word_path_index: HashMap::new(),
            word_hash_index: HashMap::new(),
        }
    }

    /// Create a new concept, optionally with a label. Returns its ID.
    pub fn create(&mut self, label: Option<String>, tick: u64) -> ConceptId {
        let id = self.concepts.len() as ConceptId;
        self.concepts.push(Concept {
            id,
            label,
            bindings: Vec::new(),
            created_at: tick,
            access_count: 0,
        });
        id
    }

    /// Bind a single (byte-level) trie path key to a concept.
    /// Equivalent to `bind_dual` with an empty word key.
    pub fn bind(
        &mut self,
        concept_id: ConceptId,
        path_key: Vec<Option<u64>>,
        tick: u64,
    ) -> Result<(), String> {
        self.bind_dual(concept_id, path_key, Vec::new(), tick)
    }

    /// Bind both byte- and word-level path keys to an existing concept.
    /// If a binding with the same byte key already exists, increment its strength
    /// and update the word key (if newly provided).
    /// Indexes both keys' leaf-anchored prefixes for lookup.
    pub fn bind_dual(
        &mut self,
        concept_id: ConceptId,
        path_key: Vec<Option<u64>>,
        word_path_key: Vec<Option<u64>>,
        tick: u64,
    ) -> Result<(), String> {
        let idx = concept_id as usize;
        if idx >= self.concepts.len() {
            return Err(format!("Concept {} not found", concept_id));
        }

        let full_hash = hash_path_key(&path_key);

        let existing = self.concepts[idx]
            .bindings
            .iter_mut()
            .find(|b| hash_path_key(&b.path_key) == full_hash);

        if let Some(binding) = existing {
            binding.strength += 1;
            if !word_path_key.is_empty() && binding.word_path_key != word_path_key {
                index_prefixes(&mut self.word_path_index, &word_path_key, concept_id);
                binding.word_path_key = word_path_key;
            }
        } else {
            index_prefixes(&mut self.path_index, &path_key, concept_id);
            if !word_path_key.is_empty() {
                index_prefixes(&mut self.word_path_index, &word_path_key, concept_id);
            }

            self.concepts[idx].bindings.push(Binding {
                path_key,
                word_path_key,
                bound_at: tick,
                strength: 1,
            });
        }

        Ok(())
    }

    /// Bind ONLY a word-level path key (one per word). Creates a binding with
    /// empty byte `path_key` and the given `word_path_key`. Indexes the word key.
    /// If a binding with the same word key already exists on this concept,
    /// increments its strength instead.
    pub fn bind_word_only(
        &mut self,
        concept_id: ConceptId,
        word_path_key: Vec<Option<u64>>,
        tick: u64,
    ) -> Result<(), String> {
        let idx = concept_id as usize;
        if idx >= self.concepts.len() {
            return Err(format!("Concept {} not found", concept_id));
        }
        let full_hash = hash_path_key(&word_path_key);
        let existing = self.concepts[idx]
            .bindings
            .iter_mut()
            .find(|b| !b.word_path_key.is_empty()
                && hash_path_key(&b.word_path_key) == full_hash);
        if let Some(b) = existing {
            b.strength += 1;
            return Ok(());
        }

        index_prefixes(&mut self.word_path_index, &word_path_key, concept_id);
        self.concepts[idx].bindings.push(Binding {
            path_key: Vec::new(),
            word_path_key,
            bound_at: tick,
            strength: 1,
        });
        Ok(())
    }

    /// Look up concepts whose bindings match the given byte-level path key.
    pub fn lookup(&self, path_key: &[Option<u64>]) -> Vec<&Concept> {
        self.lookup_in(&self.path_index, path_key)
    }

    /// Bind a word hash (u16) directly to a concept. This is the primary
    /// mechanism for word-level concept matching — bypasses the trie path key
    /// and uses the word's identity hash directly.
    pub fn bind_word_hash(
        &mut self,
        concept_id: ConceptId,
        word_hash: u16,
    ) -> Result<(), String> {
        let idx = concept_id as usize;
        if idx >= self.concepts.len() {
            return Err(format!("Concept {} not found", concept_id));
        }
        let key = word_hash as u64;
        let ids = self.word_hash_index.entry(key).or_default();
        if !ids.contains(&concept_id) {
            ids.push(concept_id);
        }
        Ok(())
    }

    /// Look up concepts by word hashes. Returns (concept_id, match_count)
    /// sorted by match count descending. This matches by direct word identity,
    /// not trie path keys — so "query" only matches concepts that actually
    /// contain the word "query", not concepts that happen to share a trie node.
    pub fn lookup_by_word_hashes(
        &self,
        word_hashes: &[u16],
    ) -> Vec<(ConceptId, usize)> {
        let mut scores: HashMap<ConceptId, usize> = HashMap::new();
        for &wh in word_hashes {
            let key = wh as u64;
            if let Some(ids) = self.word_hash_index.get(&key) {
                // Each word contributes +1 to each concept it matches
                let mut seen_for_word: Vec<ConceptId> = Vec::new();
                for &id in ids {
                    if !seen_for_word.contains(&id) {
                        seen_for_word.push(id);
                        *scores.entry(id).or_insert(0) += 1;
                    }
                }
            }
        }
        let mut ranked: Vec<(ConceptId, usize)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked
    }

    /// Multi-word lookup: given per-word path keys, rank concepts by how many
    /// of the query words match any binding on that concept. Returns
    /// (concept_id, match_count) sorted by count descending.
    pub fn lookup_multi_word(
        &self,
        word_keys: &[(String, Vec<Option<u64>>)],
    ) -> Vec<(ConceptId, usize)> {
        let mut scores: HashMap<ConceptId, usize> = HashMap::new();
        for (_w, key) in word_keys {
            if key.is_empty() {
                continue;
            }
            let mut per_word_hits: Vec<ConceptId> = Vec::new();
            // Use same specificity logic as lookup_in but collect unique concept ids for this word
            let n = key.len();
            for end in (0..n).rev() {
                let prefix = &key[..=end];
                let h = hash_path_key(prefix);
                if let Some(ids) = self.word_path_index.get(&h) {
                    for &id in ids {
                        if !per_word_hits.contains(&id) {
                            per_word_hits.push(id);
                        }
                    }
                    if !per_word_hits.is_empty() {
                        break;
                    }
                }
            }
            for id in per_word_hits {
                *scores.entry(id).or_insert(0) += 1;
            }
        }
        let mut ranked: Vec<(ConceptId, usize)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked
    }

    /// Look up concepts whose bindings match the given word-level path key.
    pub fn lookup_word(&self, path_key: &[Option<u64>]) -> Vec<&Concept> {
        self.lookup_in(&self.word_path_index, path_key)
    }

    fn lookup_in(
        &self,
        index: &HashMap<u64, Vec<ConceptId>>,
        path_key: &[Option<u64>],
    ) -> Vec<&Concept> {
        let mut seen: Vec<ConceptId> = Vec::new();
        let mut results: Vec<&Concept> = Vec::new();

        let n = path_key.len();
        for end in (0..n).rev() {
            let prefix = &path_key[..=end];
            let hash = hash_path_key(prefix);

            if let Some(ids) = index.get(&hash) {
                for &id in ids {
                    if !seen.contains(&id) {
                        seen.push(id);
                        if let Some(c) = self.concepts.get(id as usize) {
                            results.push(c);
                        }
                    }
                }
            }

            if !results.is_empty() {
                break;
            }
        }

        results
    }

    /// Increment access_count for a concept and return a reference to it.
    pub fn get_accessed(&mut self, concept_id: ConceptId) -> Option<&Concept> {
        let idx = concept_id as usize;
        if idx >= self.concepts.len() {
            return None;
        }
        self.concepts[idx].access_count += 1;
        Some(&self.concepts[idx])
    }

    /// Find groups of concepts whose bindings were all created within the same tick window.
    pub fn co_occurring(&self, tick: u64, window: u64) -> Vec<Vec<ConceptId>> {
        let lo = tick.saturating_sub(window);
        let in_window: Vec<ConceptId> = self
            .concepts
            .iter()
            .filter(|c| c.created_at >= lo && c.created_at <= tick + window)
            .map(|c| c.id)
            .collect();

        if in_window.is_empty() {
            vec![]
        } else {
            vec![in_window]
        }
    }

    pub fn concept_count(&self) -> usize {
        self.concepts.len()
    }

    pub fn get(&self, concept_id: ConceptId) -> Option<&Concept> {
        self.concepts.get(concept_id as usize)
    }

    pub fn snapshot(&self, path: &str) -> Result<usize, String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let len = json.len();
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(path, &json).map_err(|e| e.to_string())?;
        Ok(len)
    }

    pub fn restore(path: &str) -> Result<Self, String> {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }

    pub fn exists(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}

impl Default for ConceptStore {
    fn default() -> Self {
        Self::new()
    }
}
