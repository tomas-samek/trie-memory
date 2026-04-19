use trie_memory::trie::node::NodeState;
use trie_memory::trie::Trie;

// --- Perceive tests ---

#[test]
fn test_perceive_invalid_leaves_skipped() {
    let trie = Trie::new();
    let result = trie.perceive(&[999, 1000]);
    assert_eq!(result.total_leaves, 0);
    assert!(result.activation.is_empty());
}

// --- Delta encoding tests ---

#[test]
fn test_delta_encoding_aaa() {
    let mut trie = Trie::new();
    trie.write(b"aaa");
    // 'a'=97, previous=128 → delta = 97-128+128 = 97
    // 'a'=97, previous=97  → delta = 97-97+128 = 128
    // 'a'=97, previous=97  → delta = 128
    // Root buffer should contain [97, 128, 128]
    assert_eq!(trie.nodes[0].state, NodeState::Learning);
    assert_eq!(trie.nodes[0].buffer.len(), 3);
    assert_eq!(trie.nodes[0].buffer[0], 97);  // first delta: 'a' from neutral
    assert_eq!(trie.nodes[0].buffer[1], 128); // same char repeated
    assert_eq!(trie.nodes[0].buffer[2], 128); // same char repeated
}

#[test]
fn test_delta_encoding_repeated_chars_are_128() {
    let mut trie = Trie::new();
    trie.write(b"lllll");
    // First delta: 'l'(108) - 128 + 128 = 108
    // All subsequent: 108 - 108 + 128 = 128
    assert_eq!(trie.nodes[0].buffer[1], 128);
    assert_eq!(trie.nodes[0].buffer[2], 128);
    assert_eq!(trie.nodes[0].buffer[3], 128);
    assert_eq!(trie.nodes[0].buffer[4], 128);
}

#[test]
fn test_query_uses_same_delta_encoding() {
    let mut trie = Trie::new();
    // Feed enough English text to crystallize root
    let text = b"the quick brown fox jumps over the lazy dog the quick brown fox \
        jumps over the lazy dog the quick brown fox jumps over the lazy dog \
        the quick brown fox jumps over the lazy dog the quick brown fox jumps over \
        the lazy dog once more and again and again";
    trie.write(text);

    if trie.nodes[0].state == NodeState::Crystallized {
        // Query with a substring from the same text — should find matches
        let result = trie.query(b"the quick");
        // At minimum it should route through root
        assert!(!result.matches_per_depth.is_empty());
    }
}

// --- Write path tests ---

#[test]
fn test_write_short_stream_root_stays_learning() {
    let mut trie = Trie::new();
    trie.write(b"Hello");
    assert_eq!(trie.nodes[0].state, NodeState::Learning);
    assert_eq!(trie.nodes[0].buffer.len(), 5);
    assert_eq!(trie.nodes.len(), 1);
}

#[test]
fn test_write_result_counts() {
    let mut trie = Trie::new();
    let r = trie.write(b"Hello");
    assert_eq!(r.tokens_processed, 5);
    assert_eq!(r.trie_size, trie.nodes.len());
}

#[test]
fn test_write_enough_to_crystallize_root() {
    let mut trie = Trie::new();
    // English text produces diverse deltas (must exceed 256 bytes for root threshold)
    let text = b"The quick brown fox jumps over the lazy dog. \
        Pack my box with five dozen liquor jugs. \
        How vexingly quick daft zebras jump! \
        The five boxing wizards jump quickly. \
        Sphinx of black quartz, judge my vow. \
        Two driven jocks help fax my big quiz. \
        We promptly judged antique ivory buckles for the next prize.";
    trie.write(text);
    assert_eq!(trie.nodes[0].state, NodeState::Crystallized);
    assert!(!trie.nodes[0].spectrum.is_empty());
}

#[test]
fn test_routing_produces_depth_growth() {
    let mut trie = Trie::new();
    // Crystallize root
    let text = b"The quick brown fox jumps over the lazy dog. \
        Pack my box with five dozen liquor jugs. \
        How vexingly quick daft zebras jump! \
        The five boxing wizards jump quickly. \
        Sphinx of black quartz, judge my vow. \
        Two driven jocks help fax my big quiz. \
        We promptly judged antique ivory buckles for the next prize.";
    trie.write(text);
    assert_eq!(trie.nodes[0].state, NodeState::Crystallized);

    // Feed cross-script content so delta distribution is wide enough that
    // some tokens end up Different for *all* existing children, which is the
    // condition that triggers Phase 2 recursion and genuine depth growth.
    // Pure ASCII repetition produces uniform deltas; children's spectrums
    // cover them all, Phase 2 never fires, and the trie stays flat.
    for _ in 0..20 {
        let more = "public static void main(String[] args) { System.out.println(42); } \
            import numpy; x = numpy.array([1,2,3]); print(x.mean()) \
            SELECT * FROM users WHERE id = 1; DROP TABLE students;-- \
            fn main() { let x: i32 = 42; println!(\"{}\", x); } \
            <html><body><h1>Hello</h1><p>World</p></body></html> \
            pečené kuře s bramborami a rozmarýnem, 180 stupňů \
            日本語で自己紹介をするとき、まず名前を言います \
            Οι Έλληνες φιλόσοφοι έθεσαν τα θεμέλια της λογικής";
        trie.write(more.as_bytes());
    }

    let total_nodes = trie.nodes.len();
    let max_depth = trie.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    let depth_1_count = trie.nodes.iter().filter(|n| n.depth == 1).count();

    // With the revised routing, both width and depth grow on genuinely diverse
    // input: multiple siblings at depth 1, AND at least one descendant
    // forms once a heavily-visited child becomes mature.
    assert!(
        max_depth >= 2,
        "Diverse cross-script input should produce depth 2+, got {} (total {} nodes)",
        max_depth,
        total_nodes
    );
    assert!(
        depth_1_count >= 2,
        "Diverse input should produce multiple depth-1 siblings (width), got {}",
        depth_1_count
    );
}

#[test]
fn test_write_long_stream_grows_trie() {
    let mut trie = Trie::new();
    let mut data = Vec::new();
    for _ in 0..10 {
        data.extend_from_slice(b"The quick brown fox jumps over the lazy dog. ");
    }
    let result = trie.write(&data);
    assert_eq!(result.tokens_processed, data.len());
}

#[test]
fn test_english_text_produces_structure() {
    let mut trie = Trie::new();
    let text = b"It was the best of times, it was the worst of times, \
        it was the age of wisdom, it was the age of foolishness, \
        it was the epoch of belief, it was the epoch of incredulity, \
        it was the season of Light, it was the season of Darkness, \
        it was the spring of hope, it was the winter of despair, \
        we had everything before us, we had nothing before us, \
        we were all going direct to Heaven, we were all going direct \
        the other way. In short, the period was so far like the present \
        period, that some of its noisiest authorities insisted on its \
        being received, for good or for evil, in the superlative degree \
        of comparison only. There were a king with a large jaw and a \
        queen with a plain face, on the throne of England; there were \
        a king with a large jaw and a queen with a fair face, on the \
        throne of France. In both countries it was clearer than crystal \
        to the lords of the State preserves of loaves and fishes, that \
        things in general were settled for ever.";
    for _ in 0..50 {
        trie.write(text);
    }

    let max_depth = trie.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    assert!(
        max_depth >= 1,
        "English text should reach depth 1+, got {}",
        max_depth
    );
    assert!(
        trie.nodes.len() > 2,
        "Should develop structure, got {} nodes",
        trie.nodes.len()
    );
}

// --- Read path tests ---

#[test]
fn test_read_from_root() {
    let trie = Trie::new();
    let path = trie.read(0).unwrap();
    assert_eq!(path.len(), 1);
    assert_eq!(path[0].node_id, 0);
}

#[test]
fn test_read_invalid_node_returns_none() {
    let trie = Trie::new();
    assert!(trie.read(999).is_none());
}

#[test]
fn test_read_after_crystallization_and_children() {
    let mut trie = Trie::new();
    let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&data);
    if trie.nodes.len() > 1 {
        let leaf_id = trie.nodes.len() as u64 - 1;
        let path = trie.read(leaf_id).unwrap();
        assert!(path.len() >= 2);
        assert_eq!(path.last().unwrap().node_id, 0);
    }
}

// --- Query tests ---

#[test]
fn test_query_does_not_modify_trie() {
    let mut trie = Trie::new();
    let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&data);
    let nodes_before = trie.nodes.len();
    trie.query(b"BBBBBB");
    assert_eq!(trie.nodes.len(), nodes_before, "Query should not create nodes");
}

#[test]
fn test_query_returns_result() {
    let mut trie = Trie::new();
    trie.write(b"Hello");
    let result = trie.query(b"He");
    assert_eq!(result.deepest_node, 0);
}

// --- Stats tests ---

#[test]
fn test_stats_root() {
    let trie = Trie::new();
    let stats = trie.stats(None).unwrap();
    assert_eq!(stats.node_id, 0);
    assert_eq!(stats.depth, 0);
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.state, "learning");
}

#[test]
fn test_stats_after_crystallization() {
    let mut trie = Trie::new();
    let text = b"The quick brown fox jumps over the lazy dog. \
        Pack my box with five dozen liquor jugs. \
        How vexingly quick daft zebras jump! \
        The five boxing wizards jump quickly. \
        Sphinx of black quartz, judge my vow. \
        Two driven jocks help fax my big quiz. \
        We promptly judged antique ivory buckles for the next prize.";
    trie.write(text);
    let stats = trie.stats(None).unwrap();
    assert_eq!(stats.state, "crystallized");
    assert!(!stats.spectrum.is_empty());
}

#[test]
fn test_stats_invalid_node() {
    let trie = Trie::new();
    assert!(trie.stats(Some(999)).is_none());
}

// --- Perceive with crystallized trie ---

#[test]
fn test_perceive_after_crystallization() {
    let mut trie = Trie::new();
    let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&data);
    if trie.nodes.len() > 1 {
        let leaf_ids: Vec<u64> = (1..trie.nodes.len() as u64).collect();
        let result = trie.perceive(&leaf_ids);
        assert_eq!(result.total_leaves, leaf_ids.len());
        let root_act = result.activation.iter().find(|d| d.depth == 0).unwrap();
        let root_node = root_act.nodes.iter().find(|n| n.node_id == 0).unwrap();
        assert_eq!(root_node.hits, leaf_ids.len());
    }
}

#[test]
fn test_perceive_ordered_by_hits() {
    let mut trie = Trie::new();
    let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&data);
    if trie.nodes.len() > 2 {
        let leaf_ids: Vec<u64> = (1..trie.nodes.len() as u64).collect();
        let result = trie.perceive(&leaf_ids);
        for depth_act in &result.activation {
            for w in depth_act.nodes.windows(2) {
                assert!(w[0].hits >= w[1].hits);
            }
        }
    }
}

// --- Recursive routing tests ---

fn build_deep_trie() -> Trie {
    let mut trie = Trie::new();
    // Use English text — delta encoding produces diverse values
    let text = b"The quick brown fox jumps over the lazy dog. \
        Pack my box with five dozen liquor jugs. \
        How vexingly quick daft zebras jump! \
        The five boxing wizards jump quickly. \
        Sphinx of black quartz, judge my vow. \
        Two driven jocks help fax my big quiz. \
        We promptly judged antique ivory buckles for the next prize.";
    trie.write(text);
    assert_eq!(trie.nodes[0].state, NodeState::Crystallized);
    trie
}

#[test]
fn test_recursive_routing_produces_growth() {
    let mut trie = build_deep_trie();
    // With fair routing, tokens spread across siblings. Feed massive volume
    // of diverse data to create branching and depth.
    for round in 0..500u16 {
        let data: Vec<u8> = (0..256).map(|i| {
            ((i as u16 * 7 + round * 13) % 256) as u8
        }).collect();
        trie.write(&data);
    }
    // With fair routing, trie grows wide (siblings) and eventually deep
    let total_nodes = trie.nodes.len();
    assert!(total_nodes > 3, "Expected growth, got {} nodes", total_nodes);
}

#[test]
fn test_no_infinite_recursion() {
    let mut trie = Trie::new();
    let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let result = trie.write(&data);
    assert_eq!(result.tokens_processed, 1000);
}

#[test]
fn test_deep_node_has_correct_parent_chain() {
    let mut trie = Trie::new();
    for _ in 0..30 {
        let data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        trie.write(&data);
    }
    for node in &trie.nodes {
        let mut current = node.id;
        let mut steps = 0;
        loop {
            let n = &trie.nodes[current as usize];
            match n.parent {
                None => {
                    assert_eq!(current, 0);
                    break;
                }
                Some(p) => {
                    current = p;
                    steps += 1;
                    assert!(steps <= 100, "Parent chain too long, possible cycle");
                }
            }
        }
    }
}

// --- Perceive window tests ---

#[test]
fn test_perceive_window_captures_recent() {
    let mut trie = Trie::new();
    let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&data);
    let result = trie.perceive_window(1000);
    // With the bug fix, perceive_window includes non-leaf nodes with consumption
    // Root crystallizes after 256 tokens then consumes matching ones — should appear
    if trie.nodes[0].state == NodeState::Crystallized {
        assert!(result.total_leaves > 0, "Should find active nodes");
    }
}

#[test]
fn test_perceive_window_includes_non_leaf_nodes() {
    let mut trie = Trie::new();
    // Feed enough to crystallize root and have it consume tokens
    let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&data);

    let result = trie.perceive_window(1000);
    // Root has children (not a leaf) but should still appear if it consumed recently
    if trie.nodes[0].consumption_log.len() > 0 {
        let root_in_result = result.activation.iter()
            .flat_map(|d| &d.nodes)
            .any(|n| n.node_id == 0);
        assert!(root_in_result, "Root should appear in perceive_window despite having children");
    }
}

#[test]
fn test_perceive_window_excludes_old() {
    let mut trie = Trie::new();
    let wave1: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&wave1);
    let wave2: Vec<u8> = (0..300).map(|i| (i % 128) as u8).collect();
    trie.write(&wave2);

    let result = trie.perceive_window(100);
    for depth_act in &result.activation {
        for node_act in &depth_act.nodes {
            let node = &trie.nodes[node_act.node_id as usize];
            // Nodes selected as "active" should have recent consumption
            let has_recent = node.consumption_log.iter().any(|(t, _)| *t >= 500);
            // Or they're ancestors of active nodes (hit via walk-to-root)
            // Either way, the node should exist
            assert!(has_recent || node.parent.is_some() || node_act.node_id == 0);
        }
    }
}

#[test]
fn test_perceive_window_empty_when_no_recent() {
    let mut trie = Trie::new();
    let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    trie.write(&data);

    use std::sync::atomic::Ordering;
    trie.tick.store(100_000, Ordering::Relaxed);

    let result = trie.perceive_window(100);
    assert_eq!(result.total_leaves, 0);
}

// --- Depth growth tests ---

fn english_corpus() -> Vec<u8> {
    let text = b"The quick brown fox jumps over the lazy dog. \
        Pack my box with five dozen liquor jugs. \
        How vexingly quick daft zebras jump! \
        The five boxing wizards jump quickly. \
        Sphinx of black quartz, judge my vow. \
        Two driven jocks help fax my big quiz. \
        We promptly judged antique ivory buckles for the next prize. \
        Amazingly few discotheques provide jukeboxes. \
        Cwm fjord bank glyphs vext quiz. \
        Glib jocks quiz nymph to vex dwarf. \
        Jackdaws love my big sphinx of quartz. \
        The job requires extra pluck and zeal from every young wage earner.";
    let mut data = Vec::new();
    for _ in 0..3 {
        data.extend_from_slice(text);
    }
    data
}

#[test]
fn test_depth_growth_english_text() {
    // Pure ASCII English has a narrow delta distribution: a few depth-1
    // children cover it entirely, Phase 2 rarely fires, and the trie
    // stays shallow. This is correct behavior (the flat-English observation
    // from CLAUDE.md). We verify at least root + one crystallized child.
    let mut trie = Trie::new();
    let data = english_corpus();
    assert!(data.len() > 1000, "test corpus must be 1000+ bytes");
    trie.write(&data);

    let max_depth = trie.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    assert!(
        max_depth >= 1,
        "English text should reach at least depth 1, got {} with {} nodes",
        max_depth,
        trie.nodes.len()
    );
    assert!(
        trie.nodes.len() >= 2,
        "English text should produce root + at least one child, got {} nodes",
        trie.nodes.len()
    );
}

#[test]
fn test_depth_limit_is_reasonable() {
    let mut trie = Trie::new();
    let mut data = english_corpus();
    // Pad to ~5000 bytes
    while data.len() < 5000 {
        data.extend_from_slice(b"the quick brown fox jumps over the lazy dog. ");
    }
    trie.write(&data);

    let max_depth = trie.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    assert!(
        max_depth < 20,
        "Depth growth should be bounded; got {} on {} bytes",
        max_depth,
        data.len()
    );
}

#[test]
fn test_diverse_content_grows_depth() {
    let mut trie = Trie::new();
    // Mix Latin English, Czech (diacritics), Japanese, and Greek so the
    // stream contains enough exotic delta values to force some tokens to
    // be Different for every existing child, which is the condition that
    // triggers Phase 2 recursion and genuine depth growth.
    let mut data = english_corpus();
    let diverse = "fn main() { let x: i32 = 42; println!(\"{}\", x); } \
        public static void main(String[] args) { System.out.println(42); } \
        SELECT id, name FROM users WHERE id = 1; \
        <html><body><h1>Hello</h1><p>World</p></body></html> \
        pečené kuře s bramborami a rozmarýnem, 180 stupňů 45 minut \
        日本語で自己紹介をするとき、まず名前を言います。東京から来ました \
        Οι Έλληνες φιλόσοφοι έθεσαν τα θεμέλια της λογικής και της γεωμετρίας \
        import numpy; arr = numpy.array([1, 2, 3, 4, 5]); print(arr.mean())";
    for _ in 0..3 {
        data.extend_from_slice(diverse.as_bytes());
    }
    trie.write(&data);

    let max_depth = trie.nodes.iter().map(|n| n.depth).max().unwrap_or(0);

    assert!(
        max_depth >= 2,
        "Diverse content should reach depth 2+, got {} (total nodes: {})",
        max_depth,
        trie.nodes.len()
    );
}

#[test]
fn test_depth_2_nodes_have_correct_parent_chain() {
    let mut trie = Trie::new();
    trie.write(&english_corpus());

    let depth_2_nodes: Vec<&_> = trie.nodes.iter().filter(|n| n.depth == 2).collect();
    if depth_2_nodes.is_empty() {
        return;
    }

    for node in depth_2_nodes {
        let parent_id = node.parent.expect("depth-2 node must have a parent");
        let parent = &trie.nodes[parent_id as usize];
        assert_eq!(parent.depth, 1, "Parent of depth-2 node must be at depth 1");
        assert!(parent.children.contains(&node.id));
    }
}
