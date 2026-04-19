use std::collections::HashMap;
use std::sync::atomic::Ordering;

use super::Trie;

#[derive(Debug, Clone, serde::Serialize)]
pub struct PerceiveResult {
    pub activation: Vec<DepthActivation>,
    pub total_leaves: usize,
    pub max_depth_reached: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DepthActivation {
    pub depth: u32,
    pub nodes: Vec<NodeActivation>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeActivation {
    pub node_id: u64,
    pub hits: usize,
    pub visit_count: u64,
    pub spectrum: Vec<u8>,
}

impl Trie {
    /// Batch multi-leaf read: walk each leaf to root, aggregate hits per node, group by depth.
    /// Read-only — does not modify the trie.
    pub fn perceive(&self, leaf_ids: &[u64]) -> PerceiveResult {
        let mut hit_counts: HashMap<u64, usize> = HashMap::new();
        let mut max_depth: u32 = 0;
        let mut valid_leaves = 0;

        for &leaf_id in leaf_ids {
            let mut idx = leaf_id as usize;
            if idx >= self.nodes.len() {
                continue;
            }
            valid_leaves += 1;

            loop {
                let node = &self.nodes[idx];
                *hit_counts.entry(node.id).or_insert(0) += 1;
                if node.depth > max_depth {
                    max_depth = node.depth;
                }
                match node.parent {
                    Some(parent_id) => idx = parent_id as usize,
                    None => break,
                }
            }
        }

        // Group by depth, sorted by depth ascending
        let mut depth_map: HashMap<u32, Vec<NodeActivation>> = HashMap::new();
        for (&node_id, &hits) in &hit_counts {
            let node = &self.nodes[node_id as usize];
            depth_map
                .entry(node.depth)
                .or_default()
                .push(NodeActivation {
                    node_id,
                    hits,
                    visit_count: node.visit_count_val(),
                    spectrum: node.spectrum.clone(),
                });
        }

        // Sort nodes within each depth by hits descending
        let mut activation: Vec<DepthActivation> = depth_map
            .into_iter()
            .map(|(depth, mut nodes)| {
                nodes.sort_by(|a, b| b.hits.cmp(&a.hits).then_with(|| a.node_id.cmp(&b.node_id)));
                DepthActivation { depth, nodes }
            })
            .collect();
        activation.sort_by_key(|d| d.depth);

        PerceiveResult {
            activation,
            total_leaves: valid_leaves,
            max_depth_reached: max_depth,
        }
    }

    /// Perceive recent activity: find all nodes that consumed within the last N ticks,
    /// then aggregate their paths like perceive().
    pub fn perceive_window(&self, last_n_ticks: u64) -> PerceiveResult {
        let current_tick = self.tick.load(Ordering::Relaxed);
        let window_start = current_tick.saturating_sub(last_n_ticks);

        let mut active_nodes: Vec<u64> = Vec::new();
        for node in &self.nodes {
            let has_recent = node
                .consumption_log
                .iter()
                .any(|(tick, _)| *tick >= window_start);
            if has_recent {
                active_nodes.push(node.id);
            }
        }

        self.perceive(&active_nodes)
    }
}
