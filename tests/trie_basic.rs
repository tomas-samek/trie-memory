use trie_memory::trie::node::{Classification, Node, NodeState, params_for_depth};
use trie_memory::trie::Trie;

#[test]
fn test_classify_learning_returns_unknown() {
    let node = Node::new(1, Some(0), 1);
    assert_eq!(node.state, NodeState::Learning);
    assert_eq!(node.classify(42), Classification::Unknown);
    assert_eq!(node.classify(0), Classification::Unknown);
    assert_eq!(node.classify(255), Classification::Unknown);
}

#[test]
fn test_depth_params_differ() {
    let (t0, s0) = params_for_depth(0);
    let (t3, s3) = params_for_depth(3);
    assert!(t0 > t3, "Root threshold should be larger than depth 3");
    assert!(s0 > s3, "Root spectrum should be larger than depth 3");
}

#[test]
fn test_depth_params_values() {
    assert_eq!(params_for_depth(0), (256, 64));
    assert_eq!(params_for_depth(1), (128, 32));
    assert_eq!(params_for_depth(2), (64, 16));
    assert_eq!(params_for_depth(3), (32, 8));
    assert_eq!(params_for_depth(4), (16, 4));
    assert_eq!(params_for_depth(100), (16, 4));
}

#[test]
fn test_node_uses_depth_params() {
    let root = Node::new(0, None, 0);
    assert_eq!(root.crystallization_threshold, 256);
    assert_eq!(root.spectrum_max_size, 64);

    let d3 = Node::new(5, Some(1), 3);
    assert_eq!(d3.crystallization_threshold, 32);
    assert_eq!(d3.spectrum_max_size, 8);
}

/// Helper: create a crystallized node at a given depth by feeding enough observations.
fn crystallize_node(depth: u32) -> Node {
    let (threshold, _) = params_for_depth(depth);
    let mut node = Node::new(1, Some(0), depth);
    for i in 0..threshold {
        node.observe(i as u8);
    }
    assert_eq!(node.state, NodeState::Crystallized);
    node
}

#[test]
fn test_classify_crystallized_same() {
    let node = crystallize_node(3); // threshold=32, spectrum_size=8
    // spectrum contains the top-8 most frequent bytes from 0..32
    // since all are frequency 1, it picks the first 8 sorted: 0..7
    for &tok in &node.spectrum {
        assert_eq!(node.classify(tok), Classification::Same);
    }
}

#[test]
fn test_classify_crystallized_different_no_children() {
    let node = crystallize_node(3);
    // Crystallized + token not in spectrum → Different (regardless of children)
    let absent = (0..=255u8).find(|b| !node.spectrum.contains(b)).unwrap();
    assert_eq!(node.classify(absent), Classification::Different);
}

#[test]
fn test_classify_crystallized_different_with_children() {
    let mut node = crystallize_node(3);
    node.children.push(99); // fake child
    let absent = (0..=255u8).find(|b| !node.spectrum.contains(b)).unwrap();
    assert_eq!(node.classify(absent), Classification::Different);
}

#[test]
fn test_consume_increments_visit_count() {
    let mut node = crystallize_node(3);
    let tok = node.spectrum[0];
    assert_eq!(node.visit_count_val(), 0);
    node.consume(tok, 0);
    assert_eq!(node.visit_count_val(), 1);
    node.consume(tok, 1);
    assert_eq!(node.visit_count_val(), 2);
}

#[test]
fn test_consume_logs_to_consumption_log() {
    let mut node = crystallize_node(3);
    let tok = node.spectrum[0];
    node.consume(tok, 10);
    node.consume(tok, 20);
    assert_eq!(node.consumption_log.len(), 2);
    assert_eq!(node.consumption_log[0], (10, tok));
    assert_eq!(node.consumption_log[1], (20, tok));
}

#[test]
fn test_node_crystallization_threshold_depth1() {
    let (threshold, _) = params_for_depth(1);
    let mut node = Node::new(1, Some(0), 1);
    assert_eq!(node.state, NodeState::Learning);

    for i in 0..(threshold - 1) {
        node.observe(i as u8);
    }
    assert_eq!(node.state, NodeState::Learning);

    node.observe(42);
    assert_eq!(node.state, NodeState::Crystallized);
    assert!(!node.spectrum.is_empty());
    assert!(node.buffer.is_empty());
}

#[test]
fn test_trie_new_has_root() {
    let trie = Trie::new();
    assert_eq!(trie.nodes.len(), 1);
    assert_eq!(trie.nodes[0].id, 0);
    assert_eq!(trie.nodes[0].depth, 0);
    assert!(trie.nodes[0].parent.is_none());
    assert_eq!(trie.nodes[0].state, NodeState::Learning);
    assert_eq!(trie.nodes[0].crystallization_threshold, 256);
}

#[test]
fn test_spectrum_picks_most_frequent() {
    let mut node = Node::new(1, Some(0), 3); // spectrum_max_size = 8
    node.crystallization_threshold = 10;

    // Feed 7 copies of 'A' and 3 copies of 'B'
    for _ in 0..7 {
        node.observe(65);
    }
    for _ in 0..3 {
        node.observe(66);
    }

    assert_eq!(node.state, NodeState::Crystallized);
    assert!(node.spectrum.contains(&65));
    assert!(node.spectrum.contains(&66));
}

#[test]
fn test_root_crystallizes_after_256_observations() {
    let mut root = Node::new(0, None, 0);
    assert_eq!(root.crystallization_threshold, 256);

    // Feed 255 observations — still learning
    for i in 0..255u8 {
        root.observe(i);
    }
    assert_eq!(root.state, NodeState::Learning);

    // 256th observation triggers crystallization
    root.observe(0);
    assert_eq!(root.state, NodeState::Crystallized);
    // Root's spectrum should be large (up to 64 values)
    assert!(root.spectrum.len() > 8, "Root spectrum should be broad, got {}", root.spectrum.len());
}

#[test]
fn test_root_large_spectrum_after_crystallization() {
    let mut root = Node::new(0, None, 0);
    // Feed all 256 byte values — root should know most of them
    for i in 0..=255u8 {
        root.observe(i);
    }
    assert_eq!(root.state, NodeState::Crystallized);
    assert_eq!(root.spectrum.len(), 64); // spectrum_max_size for depth 0
}
