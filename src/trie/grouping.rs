use serde::Serialize;

/// Compute overlap ratio between two spectra.
/// Returns (shared_values, ratio) where ratio = shared / min(len_a, len_b).
/// Uses f64 for ratio — this is a comparison metric, not stored in the trie.
pub fn spectrum_overlap(a: &[u8], b: &[u8]) -> (Vec<u8>, f64) {
    let shared: Vec<u8> = a.iter().filter(|v| b.contains(v)).copied().collect();
    let min_len = a.len().min(b.len());
    let ratio = if min_len == 0 {
        0.0
    } else {
        shared.len() as f64 / min_len as f64
    };
    (shared, ratio)
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupSuggestion {
    pub child_ids: Vec<u64>,
    pub shared_spectrum: Vec<u8>,
    pub overlap_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsertResult {
    pub new_node_id: u64,
    pub spectrum: Vec<u8>,
    pub children_grouped: usize,
    pub depth: u32,
}
