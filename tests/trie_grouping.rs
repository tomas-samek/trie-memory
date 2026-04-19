use trie_memory::trie::grouping::spectrum_overlap;
use trie_memory::trie::node::{Node, NodeState};
use trie_memory::trie::Trie;

// --- spectrum_overlap tests ---

#[test]
fn test_spectrum_overlap_partial() {
    let (shared, ratio) = spectrum_overlap(&[1, 2, 3, 4], &[3, 4, 5, 6]);
    assert_eq!(shared, vec![3, 4]);
    assert!((ratio - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_spectrum_overlap_full() {
    let (shared, ratio) = spectrum_overlap(&[1, 2, 3], &[1, 2, 3]);
    assert_eq!(shared, vec![1, 2, 3]);
    assert!((ratio - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_spectrum_overlap_none() {
    let (shared, ratio) = spectrum_overlap(&[1, 2], &[3, 4]);
    assert!(shared.is_empty());
    assert!((ratio - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_spectrum_overlap_empty() {
    let (shared, ratio) = spectrum_overlap(&[], &[1, 2]);
    assert!(shared.is_empty());
    assert!((ratio - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_spectrum_overlap_both_empty() {
    let (shared, ratio) = spectrum_overlap(&[], &[]);
    assert!(shared.is_empty());
    assert!((ratio - 0.0).abs() < f64::EPSILON);
}

// --- Helper to build a trie with crystallized children ---

fn build_trie_with_crystallized_children() -> Trie {
    // Manually construct a trie with root + 3 crystallized children.
    // We bypass the write path so we can control spectra exactly.
    let mut trie = Trie::new();

    // Force root to crystallized state
    trie.nodes[0].state = NodeState::Crystallized;
    trie.nodes[0].spectrum = vec![10, 20, 30];

    // Child 1: spectrum [1, 2, 3, 4]
    let mut c1 = Node::new(1, Some(0), 1);
    c1.state = NodeState::Crystallized;
    c1.spectrum = vec![1, 2, 3, 4];
    trie.nodes.push(c1);

    // Child 2: spectrum [3, 4, 5, 6] — overlaps with c1 on [3, 4]
    let mut c2 = Node::new(2, Some(0), 1);
    c2.state = NodeState::Crystallized;
    c2.spectrum = vec![3, 4, 5, 6];
    trie.nodes.push(c2);

    // Child 3: spectrum [100, 200] — no overlap with c1 or c2
    let mut c3 = Node::new(3, Some(0), 1);
    c3.state = NodeState::Crystallized;
    c3.spectrum = vec![100, 200];
    trie.nodes.push(c3);

    trie.nodes[0].children = vec![1, 2, 3];

    trie
}

// --- suggest_groups tests ---

#[test]
fn test_suggest_groups_finds_overlap() {
    let trie = build_trie_with_crystallized_children();
    let suggestions = trie.suggest_groups(0, 0.3);

    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].child_ids, vec![1, 2]);
    assert_eq!(suggestions[0].shared_spectrum, vec![3, 4]);
    assert!((suggestions[0].overlap_ratio - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_suggest_groups_respects_min_overlap() {
    let trie = build_trie_with_crystallized_children();

    // With high threshold, no suggestions
    let suggestions = trie.suggest_groups(0, 0.9);
    assert!(suggestions.is_empty());

    // With low threshold, gets the pair
    let suggestions = trie.suggest_groups(0, 0.1);
    assert_eq!(suggestions.len(), 1);
}

#[test]
fn test_suggest_groups_invalid_node() {
    let trie = Trie::new();
    let suggestions = trie.suggest_groups(999, 0.3);
    assert!(suggestions.is_empty());
}

#[test]
fn test_suggest_groups_skips_learning_nodes() {
    let mut trie = Trie::new();
    trie.nodes[0].state = NodeState::Crystallized;
    trie.nodes[0].spectrum = vec![10];

    // Add a learning child
    let c1 = Node::new(1, Some(0), 1); // Learning state by default
    trie.nodes.push(c1);

    // Add a crystallized child
    let mut c2 = Node::new(2, Some(0), 1);
    c2.state = NodeState::Crystallized;
    c2.spectrum = vec![1, 2];
    trie.nodes.push(c2);

    trie.nodes[0].children = vec![1, 2];

    let suggestions = trie.suggest_groups(0, 0.1);
    assert!(suggestions.is_empty());
}

// --- insert_intermediate tests ---

#[test]
fn test_insert_intermediate_basic() {
    let mut trie = build_trie_with_crystallized_children();

    let result = trie.insert_intermediate(0, &[1, 2]).unwrap();

    assert_eq!(result.new_node_id, 4); // next after 0,1,2,3
    assert_eq!(result.spectrum, vec![1, 2, 3, 4, 5, 6]); // union
    assert_eq!(result.children_grouped, 2);
    assert_eq!(result.depth, 1);

    // Intermediate node is crystallized with union spectrum
    let intermediate = &trie.nodes[4];
    assert_eq!(intermediate.state, NodeState::Crystallized);
    assert_eq!(intermediate.spectrum, vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(intermediate.parent, Some(0));
    assert_eq!(intermediate.depth, 1);
    assert_eq!(intermediate.children, vec![1, 2]);

    // Grouped children now point to intermediate
    assert_eq!(trie.nodes[1].parent, Some(4));
    assert_eq!(trie.nodes[2].parent, Some(4));

    // Ungrouped child unchanged
    assert_eq!(trie.nodes[3].parent, Some(0));

    // Parent's children updated: removed [1,2], added [4]
    assert!(trie.nodes[0].children.contains(&3));
    assert!(trie.nodes[0].children.contains(&4));
    assert!(!trie.nodes[0].children.contains(&1));
    assert!(!trie.nodes[0].children.contains(&2));
}

#[test]
fn test_insert_intermediate_depth_recalculation() {
    let mut trie = build_trie_with_crystallized_children();

    // Add a grandchild to child 1
    let mut gc = Node::new(4, Some(1), 2);
    gc.state = NodeState::Crystallized;
    gc.spectrum = vec![10];
    trie.nodes.push(gc);
    trie.nodes[1].children.push(4);

    // Now insert intermediate between root and [1, 2]
    let result = trie.insert_intermediate(0, &[1, 2]).unwrap();
    let intermediate_id = result.new_node_id; // 5

    // Intermediate is at depth 1
    assert_eq!(trie.nodes[intermediate_id as usize].depth, 1);

    // Children 1 and 2 are now at depth 2 (was 1)
    assert_eq!(trie.nodes[1].depth, 2);
    assert_eq!(trie.nodes[2].depth, 2);

    // Grandchild is now at depth 3 (was 2)
    assert_eq!(trie.nodes[4].depth, 3);
}

#[test]
fn test_insert_intermediate_validation_wrong_parent() {
    let mut trie = build_trie_with_crystallized_children();

    // Node 3 is a child of root (0), not of node 1
    let err = trie.insert_intermediate(1, &[3, 2]).unwrap_err();
    assert!(err.contains("not a child of"));
}

#[test]
fn test_insert_intermediate_validation_too_few() {
    let mut trie = build_trie_with_crystallized_children();

    let err = trie.insert_intermediate(0, &[1]).unwrap_err();
    assert!(err.contains("at least 2"));
}

#[test]
fn test_insert_intermediate_validation_nonexistent_parent() {
    let mut trie = build_trie_with_crystallized_children();

    let err = trie.insert_intermediate(999, &[1, 2]).unwrap_err();
    assert!(err.contains("Parent not found"));
}

// --- read path after insertion ---

#[test]
fn test_read_path_includes_intermediate() {
    let mut trie = build_trie_with_crystallized_children();

    trie.insert_intermediate(0, &[1, 2]).unwrap();

    // Read from child 1 → should go through intermediate → root
    let path = trie.read(1).unwrap();
    let ids: Vec<u64> = path.iter().map(|s| s.node_id).collect();
    // path is [leaf, ..., root] = [1, intermediate(4), root(0)]
    assert_eq!(ids, vec![1, 4, 0]);
}

// --- suggest + insert round trip ---

#[test]
fn test_suggest_then_group_roundtrip() {
    let mut trie = build_trie_with_crystallized_children();

    let suggestions = trie.suggest_groups(0, 0.3);
    assert!(!suggestions.is_empty());

    let suggestion = &suggestions[0];
    let result = trie
        .insert_intermediate(0, &suggestion.child_ids)
        .unwrap();

    // The intermediate has union spectrum (superset of shared)
    for val in &suggestion.shared_spectrum {
        assert!(result.spectrum.contains(val));
    }

    // Stats should reflect the new node
    let stats = trie.stats(Some(result.new_node_id)).unwrap();
    assert_eq!(stats.children_count, suggestion.child_ids.len());
    assert_eq!(stats.state, "crystallized");
    assert_eq!(stats.spectrum, result.spectrum);
}

// --- union spectrum tests ---

#[test]
fn test_union_spectrum_disjoint_children() {
    // Two children with completely disjoint spectra — union should contain all values
    let mut trie = Trie::new();
    trie.nodes[0].state = NodeState::Crystallized;
    trie.nodes[0].spectrum = vec![10];

    let mut c1 = Node::new(1, Some(0), 1);
    c1.state = NodeState::Crystallized;
    c1.spectrum = vec![1, 2, 3];
    trie.nodes.push(c1);

    let mut c2 = Node::new(2, Some(0), 1);
    c2.state = NodeState::Crystallized;
    c2.spectrum = vec![4, 5, 6];
    trie.nodes.push(c2);

    trie.nodes[0].children = vec![1, 2];

    let result = trie.insert_intermediate(0, &[1, 2]).unwrap();
    assert_eq!(result.spectrum, vec![1, 2, 3, 4, 5, 6]);
    assert!(!result.spectrum.is_empty());
}

#[test]
fn test_union_spectrum_non_empty_after_grouping() {
    let mut trie = build_trie_with_crystallized_children();
    let result = trie.insert_intermediate(0, &[1, 2]).unwrap();

    // Intermediate must have a non-empty spectrum
    assert!(!result.spectrum.is_empty());
    assert!(!trie.nodes[result.new_node_id as usize].spectrum.is_empty());
}

#[test]
fn test_read_path_shows_nonempty_intermediate_spectrum() {
    let mut trie = build_trie_with_crystallized_children();
    trie.insert_intermediate(0, &[1, 2]).unwrap();

    let path = trie.read(1).unwrap();
    // path[1] is the intermediate node
    assert_eq!(path[1].node_id, 4);
    assert!(!path[1].spectrum.is_empty());
}

// --- Same-before-Unknown routing priority tests ---

#[test]
fn test_same_beats_unknown_regardless_of_order() {
    // Learning child appears BEFORE crystallized child in children list.
    // Token matches crystallized child's spectrum.
    // Crystallized child must consume it, not the Learning child.
    let mut trie = Trie::new();
    trie.nodes[0].state = NodeState::Crystallized;
    trie.nodes[0].spectrum = vec![]; // root rejects everything → routes to children

    // Child 1: Learning (classifies everything as Unknown)
    let learning = Node::new(1, Some(0), 1);
    trie.nodes.push(learning);

    // Child 2: Crystallized with spectrum [128] (the delta for first byte 128)
    let mut crystallized = Node::new(2, Some(0), 1);
    crystallized.state = NodeState::Crystallized;
    crystallized.spectrum = vec![128];
    trie.nodes.push(crystallized);

    // Learning child listed FIRST — would steal tokens without the fix
    trie.nodes[0].children = vec![1, 2];

    let initial_visit_count = trie.nodes[2].visit_count_val();
    let initial_buffer_len = trie.nodes[1].buffer.len();

    // Write byte 128 → delta = (128 - 128) + 128 = 128 → matches crystallized child
    trie.write(&[128]);

    // Crystallized child should have consumed
    assert!(
        trie.nodes[2].visit_count_val() > initial_visit_count,
        "Crystallized child should consume token matching its spectrum"
    );
    // Learning child should NOT have absorbed it
    assert_eq!(
        trie.nodes[1].buffer.len(),
        initial_buffer_len,
        "Learning child should not absorb token when a Same match exists"
    );
}

#[test]
fn test_same_beats_unknown_reversed_order() {
    // Same test but crystallized child listed FIRST — should still work
    let mut trie = Trie::new();
    trie.nodes[0].state = NodeState::Crystallized;
    trie.nodes[0].spectrum = vec![];

    let mut crystallized = Node::new(1, Some(0), 1);
    crystallized.state = NodeState::Crystallized;
    crystallized.spectrum = vec![128];
    trie.nodes.push(crystallized);

    let learning = Node::new(2, Some(0), 1);
    trie.nodes.push(learning);

    // Crystallized child listed FIRST
    trie.nodes[0].children = vec![1, 2];

    trie.write(&[128]);

    assert!(trie.nodes[1].visit_count_val() > 0);
    assert_eq!(trie.nodes[2].buffer.len(), 0);
}
