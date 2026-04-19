pub mod grouping;
pub mod node;
pub mod perceive;
pub mod persistence;
pub mod query;
pub mod read;
pub mod tokenizer;
pub mod write;

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use node::Node;

pub use grouping::{GroupSuggestion, InsertResult};
pub use node::{Classification, NodeState, params_for_depth};
pub use perceive::PerceiveResult;
pub use query::QueryResult;
pub use read::NodeSummary;
pub use write::WriteResult;

/// Node stats returned by the stats() method.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeStats {
    pub node_id: u64,
    pub depth: u32,
    pub visit_count: u64,
    pub children_count: usize,
    pub spectrum: Vec<u8>,
    pub state: String,
    pub buffer_size: usize,
    pub consumption_log_size: usize,
    pub total_nodes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Trie {
    pub nodes: Vec<Node>,
    pub root: u64,
    #[serde(with = "node::atomic_u64_serde")]
    pub tick: AtomicU64,
}

impl Trie {
    pub fn new() -> Self {
        Self {
            nodes: vec![Node::new(0, None, 0)],
            root: 0,
            tick: AtomicU64::new(0),
        }
    }

    pub fn next_tick(&self) -> u64 {
        self.tick.fetch_add(1, Ordering::Relaxed)
    }

    /// Examine a node's children and find groups with overlapping spectra.
    pub fn suggest_groups(&self, node_id: u64, min_overlap: f64) -> Vec<GroupSuggestion> {
        let idx = node_id as usize;
        if idx >= self.nodes.len() {
            return vec![];
        }

        let children = &self.nodes[idx].children;
        let mut suggestions: Vec<GroupSuggestion> = Vec::new();

        for i in 0..children.len() {
            for j in (i + 1)..children.len() {
                let ci = children[i] as usize;
                let cj = children[j] as usize;

                if self.nodes[ci].state != NodeState::Crystallized {
                    continue;
                }
                if self.nodes[cj].state != NodeState::Crystallized {
                    continue;
                }

                let (shared, ratio) =
                    grouping::spectrum_overlap(&self.nodes[ci].spectrum, &self.nodes[cj].spectrum);

                if ratio >= min_overlap && !shared.is_empty() {
                    suggestions.push(GroupSuggestion {
                        child_ids: vec![children[i], children[j]],
                        shared_spectrum: shared,
                        overlap_ratio: ratio,
                    });
                }
            }
        }

        suggestions.sort_by(|a, b| b.overlap_ratio.partial_cmp(&a.overlap_ratio).unwrap());
        suggestions
    }

    /// Insert a new intermediate node between parent and specified children.
    /// The intermediate's spectrum is the union of the children's spectra.
    pub fn insert_intermediate(
        &mut self,
        parent_id: u64,
        child_ids: &[u64],
    ) -> Result<InsertResult, String> {
        let parent_idx = parent_id as usize;
        if parent_idx >= self.nodes.len() {
            return Err("Parent not found".to_string());
        }

        if child_ids.len() < 2 {
            return Err("Need at least 2 children to group".to_string());
        }

        // Validate all child_ids are direct children of parent
        let parent_children = &self.nodes[parent_idx].children;
        for &cid in child_ids {
            if !parent_children.contains(&cid) {
                return Err(format!("Node {} is not a child of {}", cid, parent_id));
            }
        }

        // Compute union spectrum (everything any child handles)
        let mut union_spectrum: Vec<u8> = Vec::new();
        for &cid in child_ids {
            for &val in &self.nodes[cid as usize].spectrum {
                if !union_spectrum.contains(&val) {
                    union_spectrum.push(val);
                }
            }
        }
        union_spectrum.sort();

        // Create intermediate node
        let new_id = self.nodes.len() as u64;
        let parent_depth = self.nodes[parent_idx].depth;
        let mut intermediate = Node::new(new_id, Some(parent_id), parent_depth + 1);

        // Set intermediate's spectrum directly (skip learning phase)
        intermediate.spectrum = union_spectrum.clone();
        intermediate.state = NodeState::Crystallized;

        // Reparent: children move from parent to intermediate
        for &cid in child_ids {
            self.nodes[cid as usize].parent = Some(new_id);
            intermediate.children.push(cid);
        }

        // Remove grouped children from parent's children list
        self.nodes[parent_idx]
            .children
            .retain(|c| !child_ids.contains(c));

        // Add intermediate to parent's children list
        self.nodes[parent_idx].children.push(new_id);

        let children_grouped = child_ids.len();
        self.nodes.push(intermediate);

        // Recalculate depths for all descendants of the intermediate
        self.recalculate_depths(new_id);

        Ok(InsertResult {
            new_node_id: new_id,
            spectrum: union_spectrum,
            children_grouped,
            depth: parent_depth + 1,
        })
    }

    /// Recursively recalculate depths for a node and all its descendants.
    fn recalculate_depths(&mut self, node_id: u64) {
        let idx = node_id as usize;
        let parent_depth = match self.nodes[idx].parent {
            Some(pid) => self.nodes[pid as usize].depth,
            None => return,
        };
        self.nodes[idx].depth = parent_depth + 1;

        let children: Vec<u64> = self.nodes[idx].children.clone();
        for child_id in children {
            self.recalculate_depths(child_id);
        }
    }

    /// Get stats for a node (or root if None).
    pub fn stats(&self, node_id: Option<u64>) -> Option<NodeStats> {
        let id = node_id.unwrap_or(0);
        let idx = id as usize;
        if idx >= self.nodes.len() {
            return None;
        }

        let node = &self.nodes[idx];
        Some(NodeStats {
            node_id: node.id,
            depth: node.depth,
            visit_count: node.visit_count_val(),
            children_count: node.children.len(),
            spectrum: node.spectrum.clone(),
            state: match node.state {
                NodeState::Learning => "learning".to_string(),
                NodeState::Crystallized => "crystallized".to_string(),
            },
            buffer_size: node.buffer.len(),
            consumption_log_size: node.consumption_log.len(),
            total_nodes: self.nodes.len(),
        })
    }

    /// Route an input byte stream through the trie (read-only) and return the
    /// path key of the deepest matching node. Convenience wrapper for tools.
    pub fn path_key_for_input(&self, input: &[u8]) -> Option<Vec<Option<u64>>> {
        let result = self.query(input);
        self.path_key(result.deepest_node)
    }

    /// Generate a content-addressable path key from a node to root.
    /// Each segment is the content_id (FNV-1a of spectrum) at that depth.
    /// Returns deepest-first: [leaf_cid, ..., root_cid].
    /// Segments for Learning nodes are None (no stable identity yet).
    pub fn path_key(&self, node_id: u64) -> Option<Vec<Option<u64>>> {
        let mut idx = node_id as usize;
        if idx >= self.nodes.len() {
            return None;
        }
        let mut key = Vec::new();
        loop {
            key.push(self.nodes[idx].content_id());
            match self.nodes[idx].parent {
                Some(pid) => idx = pid as usize,
                None => break,
            }
        }
        Some(key)
    }
}

impl Default for Trie {
    fn default() -> Self {
        Self::new()
    }
}
