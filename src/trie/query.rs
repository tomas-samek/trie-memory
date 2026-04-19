use super::node::Classification;
use super::Trie;

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryResult {
    pub deepest_node: u64,
    pub match_depth: u32,
    pub matches_per_depth: Vec<(u32, usize)>,
}

impl Trie {
    /// Read-only traversal: feed pattern through the trie without modifying it.
    /// Applies the same delta encoding as write() so patterns match correctly.
    pub fn query(&self, pattern: &[u8]) -> QueryResult {
        let mut deepest_node: u64 = 0;
        let mut max_depth: u32 = 0;
        let mut depth_counts: std::collections::HashMap<u32, usize> =
            std::collections::HashMap::new();
        let mut previous: u8 = 128;

        for &byte in pattern {
            let delta = ((byte as i16 - previous as i16) + 128).clamp(0, 255) as u8;
            let (node_id, depth) = self.query_route(0, delta);
            previous = byte;
            if depth >= max_depth {
                max_depth = depth;
                deepest_node = node_id;
            }
            *depth_counts.entry(depth).or_insert(0) += 1;
        }

        let mut matches_per_depth: Vec<(u32, usize)> = depth_counts.into_iter().collect();
        matches_per_depth.sort_by_key(|&(d, _)| d);

        QueryResult {
            deepest_node,
            match_depth: max_depth,
            matches_per_depth,
        }
    }

    /// Route a single token read-only, returning (node_id, depth) where it matched.
    fn query_route(&self, node_idx: usize, token: u8) -> (u64, u32) {
        let node = &self.nodes[node_idx];
        match node.classify(token) {
            Classification::Same => (node.id, node.depth),
            Classification::Unknown => (node.id, node.depth),
            Classification::Different => {
                // Phase 1a: check all children for Same first
                for &child_id in &node.children {
                    let cidx = child_id as usize;
                    let child = &self.nodes[cidx];
                    if child.classify(token) == Classification::Same {
                        return (child.id, child.depth);
                    }
                }

                // Phase 1b: no Same found, check all children for Unknown
                for &child_id in &node.children {
                    let cidx = child_id as usize;
                    let child = &self.nodes[cidx];
                    if child.classify(token) == Classification::Unknown {
                        return (child.id, child.depth);
                    }
                }

                // Phase 2: recurse into crystallized children (even if childless).
                // Skip Learning nodes — they handle tokens via Phase 1 (Unknown).
                for &child_id in &node.children {
                    let cidx = child_id as usize;
                    if self.nodes[cidx].state == crate::trie::NodeState::Crystallized {
                        let result = self.query_route(cidx, token);
                        // If subtree found a deeper match, return it
                        if result.1 > self.nodes[cidx].depth {
                            return result;
                        }
                    }
                }

                // No match anywhere — report at this node
                (node.id, node.depth)
            }
        }
    }
}
