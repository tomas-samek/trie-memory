use super::node::{Classification, Node, NodeState};
use super::Trie;

#[derive(Debug, Clone, serde::Serialize)]
pub struct WriteResult {
    pub tokens_processed: usize,
    pub nodes_created: usize,
    pub trie_size: usize,
}

impl Trie {
    /// Feed a byte stream through the trie write path.
    /// Applies delta encoding: each token is the difference from the previous byte.
    pub fn write(&mut self, stream: &[u8]) -> WriteResult {
        let initial_len = self.nodes.len();
        let mut tokens_processed = 0;
        let mut previous: u8 = 128; // neutral starting point

        for &byte in stream {
            let delta = ((byte as i16 - previous as i16) + 128).clamp(0, 255) as u8;
            self.write_token(delta);
            tokens_processed += 1;
            previous = byte;
        }

        WriteResult {
            tokens_processed,
            nodes_created: self.nodes.len() - initial_len,
            trie_size: self.nodes.len(),
        }
    }

    fn write_token(&mut self, token: u8) {
        let tick = self.next_tick();
        self.route(0, token, tick);
    }

    /// Route a token through the trie. Returns true if handled (Same/Unknown),
    /// false if nothing in this subtree matched (all Different).
    fn route(&mut self, node_idx: usize, token: u8, tick: u64) -> bool {
        let classification = self.nodes[node_idx].classify(token);

        match classification {
            Classification::Same => {
                self.nodes[node_idx].consume(token, tick);
                true
            }
            Classification::Unknown => {
                self.handle_unknown(node_idx, token, tick);
                true
            }
            Classification::Different => {
                let children: Vec<u64> = self.nodes[node_idx].children.clone();

                // Phase 1a: Try ALL children for Same first
                for &child_id in &children {
                    let cidx = child_id as usize;
                    if self.nodes[cidx].classify(token) == Classification::Same {
                        self.nodes[cidx].consume(token, tick);
                        return true;
                    }
                }

                // Phase 1b: No Same found. Try ALL children for Unknown
                for &child_id in &children {
                    let cidx = child_id as usize;
                    if self.nodes[cidx].classify(token) == Classification::Unknown {
                        self.handle_unknown(cidx, token, tick);
                        return true;
                    }
                }

                // Phase 2: All children said Different. Recurse only into children
                // that are "ready" for subtree growth — already have children, OR
                // have been visited enough post-crystallization to count as mature.
                // Fresh childless crystallized children are skipped so Phase 3 at
                // this level can create new siblings (width growth). Once a child
                // has absorbed enough tokens, it matures and begins accepting depth.
                //
                // Skip Learning nodes — Phase 1b already handled their Unknowns.
                for &child_id in &children {
                    let cidx = child_id as usize;
                    let child = &self.nodes[cidx];
                    if child.state != NodeState::Crystallized {
                        continue;
                    }
                    let has_children = !child.children.is_empty();
                    // Maturity = absorbed Same-hits equal to ~25% of its own
                    // crystallization threshold. Empirically tuned to keep
                    // small-corpus tries flat-ish while letting heavily-visited
                    // children sprout descendants on real-sized input.
                    let mature = child.visit_count_val()
                        >= (child.crystallization_threshold as u64) / 4;
                    if has_children || mature {
                        if self.route(cidx, token, tick) {
                            return true; // subtree handled it
                        }
                        // subtree couldn't handle it — try next sibling's subtree
                    }
                }

                // Phase 3: Nobody in any subtree wanted this token.
                // Create new sibling at this level.
                self.create_child_and_observe(node_idx, token, tick);
                true
            }
        }
    }

    fn handle_unknown(&mut self, node_idx: usize, token: u8, _tick: u64) {
        match self.nodes[node_idx].state {
            NodeState::Learning => {
                self.nodes[node_idx].observe(token);
            }
            NodeState::Crystallized => {
                self.create_child_and_observe(node_idx, token, _tick);
            }
        }
    }

    fn create_child_and_observe(&mut self, parent_idx: usize, token: u8, _tick: u64) {
        let new_id = self.nodes.len() as u64;
        let parent_depth = self.nodes[parent_idx].depth;
        let parent_id = self.nodes[parent_idx].id;

        let mut child = Node::new(new_id, Some(parent_id), parent_depth + 1);
        child.observe(token);

        self.nodes.push(child);
        self.nodes[parent_idx].children.push(new_id);
    }
}
