use super::Trie;

#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeSummary {
    pub node_id: u64,
    pub depth: u32,
    pub spectrum: Vec<u8>,
    pub visit_count: u64,
}

impl Trie {
    /// Walk from a leaf node to root, returning summaries (leaf first, root last).
    pub fn read(&self, leaf_id: u64) -> Option<Vec<NodeSummary>> {
        let mut idx = leaf_id as usize;
        if idx >= self.nodes.len() {
            return None;
        }

        let mut path = Vec::new();
        loop {
            let node = &self.nodes[idx];
            path.push(NodeSummary {
                node_id: node.id,
                depth: node.depth,
                spectrum: node.spectrum.clone(),
                visit_count: node.visit_count_val(),
            });
            match node.parent {
                Some(parent_id) => idx = parent_id as usize,
                None => break,
            }
        }

        Some(path)
    }
}
