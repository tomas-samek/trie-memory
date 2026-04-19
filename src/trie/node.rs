use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Returns (crystallization_threshold, spectrum_max_size) for a given depth.
///
/// Root is broadest (slow to form, large spectrum).
/// Deeper nodes are more specific (fast to form, small spectrum).
pub fn params_for_depth(depth: u32) -> (usize, usize) {
    match depth {
        0 => (256, 64),
        1 => (128, 32),
        2 => (64, 16),
        3 => (32, 8),
        _ => (16, 4),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Learning,
    Crystallized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    Same,
    Different,
    Unknown,
}

/// Custom serde for AtomicU64: serialize as plain u64.
pub mod atomic_u64_serde {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(val: &AtomicU64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(val.load(Ordering::Relaxed))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<AtomicU64, D::Error> {
        Ok(AtomicU64::new(u64::deserialize(d)?))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    pub spectrum: Vec<u8>,
    #[serde(with = "atomic_u64_serde")]
    pub visit_count: AtomicU64,
    pub depth: u32,
    pub state: NodeState,
    pub buffer: Vec<u8>,
    pub crystallization_threshold: usize,
    pub spectrum_max_size: usize,
    pub consumption_log: Vec<(u64, u8)>,
}

impl Node {
    pub fn new(id: u64, parent: Option<u64>, depth: u32) -> Self {
        let (threshold, spectrum_max) = params_for_depth(depth);
        Self {
            id,
            parent,
            children: Vec::new(),
            spectrum: Vec::new(),
            visit_count: AtomicU64::new(0),
            depth,
            state: NodeState::Learning,
            buffer: Vec::new(),
            crystallization_threshold: threshold,
            spectrum_max_size: spectrum_max,
            consumption_log: Vec::new(),
        }
    }

    pub fn classify(&self, token: u8) -> Classification {
        if self.state == NodeState::Learning {
            return Classification::Unknown;
        }
        if self.spectrum.contains(&token) {
            return Classification::Same;
        }
        Classification::Different
    }

    /// Add token to learning buffer. Crystallizes if threshold reached.
    pub fn observe(&mut self, token: u8) {
        self.buffer.push(token);
        if self.buffer.len() >= self.crystallization_threshold {
            self.crystallize();
        }
    }

    /// Record a Same match: increment visit_count and log consumption.
    pub fn consume(&mut self, token: u8, tick: u64) {
        self.visit_count.fetch_add(1, Ordering::Relaxed);
        self.consumption_log.push((tick, token));
    }

    /// Compute spectrum from buffer and transition to Crystallized.
    fn crystallize(&mut self) {
        let mut counts: HashMap<u8, usize> = HashMap::new();
        for &b in &self.buffer {
            *counts.entry(b).or_insert(0) += 1;
        }

        let mut entries: Vec<(u8, usize)> = counts.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        self.spectrum = entries
            .into_iter()
            .take(self.spectrum_max_size)
            .map(|(tok, _)| tok)
            .collect();
        self.spectrum.sort();

        self.state = NodeState::Crystallized;
        self.buffer.clear();
        self.buffer.shrink_to_fit();
    }

    pub fn visit_count_val(&self) -> u64 {
        self.visit_count.load(Ordering::Relaxed)
    }

    /// Content-addressable ID derived purely from the delta spectrum.
    /// Returns None for Learning nodes — no stable identity until crystallized.
    /// Returns Some(hash) for Crystallized nodes. Same spectrum → same ID. Always.
    pub fn content_id(&self) -> Option<u64> {
        if self.state == NodeState::Learning || self.spectrum.is_empty() {
            return None;
        }
        Some(fnv1a_hash(&self.spectrum))
    }
}

/// FNV-1a hash for byte slices. Deterministic, fast, no deps.
pub fn fnv1a_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut hash = FNV_OFFSET;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
