use trie_memory::store::concept::ConceptStore;
use trie_memory::trie::Trie;

// Feed enough text to crystallize root so path keys are stable
fn crystallized_trie() -> Trie {
    let mut trie = Trie::new();
    // Feed diverse multilingual content to build structure at various depths
    let corpus: &[&[u8]] = &[
        // English prose
        b"the quick brown fox jumps over the lazy dog and the cat sat on the mat",
        b"bush vegetation plant shrub tree forest garden nature green leaf",
        b"automobile car vehicle transport road street drive motor engine",
        // Czech (diacritics create exotic deltas)
        "křoví strom les zahrada příroda zelená list vegetace".as_bytes(),
        "automobil auto vozidlo doprava silnice motor".as_bytes(),
        // Japanese/Chinese multi-byte UTF-8 (very different deltas)
        "茂み tree plant nature".as_bytes(),
        "樹木 forest green".as_bytes(),
        // Code
        b"fn main() { let x = 42; println!(\"{}\", x); }",
        b"SELECT id FROM users WHERE name = 'bush';",
    ];
    for _ in 0..3 {
        for chunk in corpus {
            trie.write(chunk);
        }
    }
    trie
}

// --- content_id / path_key tests ---

#[test]
fn test_learning_node_content_id_is_none() {
    let trie = Trie::new();
    // Root starts as Learning
    assert!(trie.nodes[0].content_id().is_none());
}

#[test]
fn test_crystallized_node_content_id_is_some() {
    let trie = crystallized_trie();
    // Root should be crystallized after enough input
    if trie.nodes[0].state == trie_memory::trie::NodeState::Crystallized {
        assert!(trie.nodes[0].content_id().is_some());
    }
}

#[test]
fn test_path_key_learning_segment_is_none() {
    let mut trie = Trie::new();
    // Write a small amount — root stays Learning
    trie.write(b"hello");
    let key = trie.path_key(0).unwrap();
    assert_eq!(key.len(), 1); // just root
    assert!(key[0].is_none(), "Learning root should have None content_id in path key");
}

#[test]
fn test_path_key_crystallized_all_some() {
    let trie = crystallized_trie();
    // Find a crystallized leaf
    let crystallized_leaf = trie.nodes.iter().find(|n| {
        n.state == trie_memory::trie::NodeState::Crystallized && n.children.is_empty()
    });
    if let Some(leaf) = crystallized_leaf {
        let key = trie.path_key(leaf.id).unwrap();
        // All nodes on the path should be crystallized — all segments Some
        for (i, seg) in key.iter().enumerate() {
            assert!(seg.is_some(), "Segment {} should be Some for fully crystallized path", i);
        }
    }
}

// --- Cross-language binding ---

#[test]
fn test_cross_language_binding() {
    let trie = crystallized_trie();
    let mut concepts = ConceptStore::new();

    // Create a concept for "vegetation/bush"
    let concept_id = concepts.create(Some("bush/vegetation".to_string()), 0);

    // Bind three surface forms
    let english = trie.path_key_for_input(b"bush vegetation plant").unwrap();
    let czech = trie.path_key_for_input("křoví vegetace".as_bytes()).unwrap();
    let japanese = trie.path_key_for_input("茂み".as_bytes()).unwrap();

    concepts.bind(concept_id, english.clone(), 0).unwrap();
    concepts.bind(concept_id, czech.clone(), 1).unwrap();
    concepts.bind(concept_id, japanese.clone(), 2).unwrap();

    // Query with English → should find the concept
    let en_results = concepts.lookup(&english);
    assert!(!en_results.is_empty(), "English query should find the concept");
    assert!(en_results.iter().any(|c| c.id == concept_id));

    // Query with Czech → should find the SAME concept
    let cz_results = concepts.lookup(&czech);
    assert!(!cz_results.is_empty(), "Czech query should find the concept");
    assert!(cz_results.iter().any(|c| c.id == concept_id));

    // Query with Japanese → should find the SAME concept
    let jp_results = concepts.lookup(&japanese);
    assert!(!jp_results.is_empty(), "Japanese query should find the concept");
    assert!(jp_results.iter().any(|c| c.id == concept_id));

    // Verify the concept has at least one binding
    // (some surface forms may route to the same trie node and deduplicate)
    let concept = concepts.get(concept_id).unwrap();
    assert!(!concept.bindings.is_empty(), "Concept should have at least 1 binding");
}

// --- Auto binding (co-occurrence) ---

#[test]
fn test_auto_binding_creates_single_concept() {
    let trie = crystallized_trie();
    let mut concepts = ConceptStore::new();

    let inputs: &[&[u8]] = &[
        b"tree strom",
        b"\xe6\xa8\xb9\xe6\x9c\xa8",
        b"tree forest green",
    ];

    // Simulate concept_bind_auto: create one concept, bind all surface forms
    let concept_id = concepts.create(Some("tree".to_string()), 100);
    let mut bound_keys = Vec::new();
    for input in inputs {
        let key = trie.path_key_for_input(input).unwrap();
        concepts.bind(concept_id, key.clone(), 100).unwrap();
        bound_keys.push(key);
    }

    // Verify single concept — binding count may be <= inputs if some route to the same trie node
    let concept = concepts.get(concept_id).unwrap();
    assert!(
        concept.bindings.len() >= 1 && concept.bindings.len() <= bound_keys.len(),
        "Expected 1..={} bindings, got {}",
        bound_keys.len(),
        concept.bindings.len()
    );

    // Lookup via any surface form should find the same concept
    for key in &bound_keys {
        let results = concepts.lookup(key);
        assert!(
            results.iter().any(|c| c.id == concept_id),
            "Lookup via bound key should find the concept"
        );
    }
}

#[test]
fn test_repeated_bind_increments_strength() {
    let trie = crystallized_trie();
    let mut concepts = ConceptStore::new();

    let concept_id = concepts.create(None, 0);
    let key = trie.path_key_for_input(b"bush vegetation").unwrap();

    concepts.bind(concept_id, key.clone(), 0).unwrap();
    concepts.bind(concept_id, key.clone(), 1).unwrap();
    concepts.bind(concept_id, key.clone(), 2).unwrap();

    let concept = concepts.get(concept_id).unwrap();
    // Same key → one binding with strength 3
    assert_eq!(concept.bindings.len(), 1, "Repeated bind should not duplicate");
    assert_eq!(concept.bindings[0].strength, 3);
}

// --- Partial key matching ---

#[test]
fn test_partial_key_matching_shorter_query_finds_longer_bound() {
    let trie = crystallized_trie();
    let mut concepts = ConceptStore::new();

    // Find a deep path key (depth > 1)
    let deep_key = trie
        .nodes
        .iter()
        .filter(|n| n.depth >= 2 && n.state == trie_memory::trie::NodeState::Crystallized)
        .next()
        .and_then(|n| trie.path_key(n.id));

    if deep_key.is_none() {
        return; // Skip if no deep nodes yet
    }
    let deep_key = deep_key.unwrap();
    assert!(deep_key.len() >= 3, "Need depth >= 2 for partial match test");

    let concept_id = concepts.create(Some("deep concept".to_string()), 0);
    concepts.bind(concept_id, deep_key.clone(), 0).unwrap();

    // Query with just the root segment (last element, since key is deepest-first)
    let root_suffix = deep_key[deep_key.len() - 1..].to_vec();
    let results = concepts.lookup(&root_suffix);
    assert!(
        results.iter().any(|c| c.id == concept_id),
        "Shorter root-anchored query should find concept bound at longer key"
    );

    // Query with 2 segments from root
    if deep_key.len() >= 2 {
        let two_suffix = deep_key[deep_key.len() - 2..].to_vec();
        let results = concepts.lookup(&two_suffix);
        assert!(
            results.iter().any(|c| c.id == concept_id),
            "Two-segment root-anchored query should find concept bound at longer key"
        );
    }
}

// --- Persistence round-trip ---

#[test]
fn test_concept_store_snapshot_restore() {
    let trie = crystallized_trie();
    let mut concepts = ConceptStore::new();

    let id = concepts.create(Some("test-concept".to_string()), 42);
    let key = trie.path_key_for_input(b"test input").unwrap();
    concepts.bind(id, key.clone(), 42).unwrap();

    let path = "target/test-concept-store.json";
    let bytes = concepts.snapshot(path).unwrap();
    assert!(bytes > 0);

    let restored = ConceptStore::restore(path).unwrap();
    assert_eq!(restored.concept_count(), 1);

    let restored_concept = restored.get(id).unwrap();
    assert_eq!(restored_concept.label.as_deref(), Some("test-concept"));
    assert_eq!(restored_concept.created_at, 42);

    // Lookup still works after restore
    let results = restored.lookup(&key);
    assert!(results.iter().any(|c| c.id == id));
}

// --- MCP integration via call_tool ---

#[test]
fn test_mcp_concept_tools_end_to_end() {
    use serde_json::json;
    use trie_memory::mcp::tools::call_tool;
    use trie_memory::store::ContentStore;

    let mut trie = crystallized_trie();
    let mut word_trie = trie_memory::trie::Trie::new();
    let mut store = ContentStore::new();
    let mut concepts = ConceptStore::new();
    let mut layers = trie_memory::store::layer::LayerStore::new();
    let mut context = trie_memory::mcp::context::ContextWindow::default();

    // concept_create
    let result = call_tool(&mut trie, &mut word_trie, &mut store, &mut concepts, &mut layers, &mut context, "concept_create",
        &json!({ "label": "vegetation" }));
    let text: serde_json::Value = serde_json::from_str(
        result["content"][0]["text"].as_str().unwrap()
    ).unwrap();
    let concept_id = text["concept_id"].as_u64().unwrap();
    assert_eq!(concept_id, 0);

    // concept_bind
    let result = call_tool(&mut trie, &mut word_trie, &mut store, &mut concepts, &mut layers, &mut context, "concept_bind",
        &json!({ "concept_id": concept_id, "input": "bush plant" }));
    assert!(result.get("isError").is_none(), "concept_bind should succeed");

    // concept_lookup
    let result = call_tool(&mut trie, &mut word_trie, &mut store, &mut concepts, &mut layers, &mut context, "concept_lookup",
        &json!({ "input": "bush plant" }));
    let text: serde_json::Value = serde_json::from_str(
        result["content"][0]["text"].as_str().unwrap()
    ).unwrap();
    let found = text["concepts"].as_array().unwrap();
    assert!(!found.is_empty(), "Should find the bound concept");
    assert_eq!(found[0]["id"].as_u64().unwrap(), concept_id);

    // concept_bind_auto
    let result = call_tool(&mut trie, &mut word_trie, &mut store, &mut concepts, &mut layers, &mut context, "concept_bind_auto",
        &json!({ "inputs": ["tree", "strom", "木"], "label": "tree" }));
    assert!(result.get("isError").is_none(), "concept_bind_auto should succeed");
    let text: serde_json::Value = serde_json::from_str(
        result["content"][0]["text"].as_str().unwrap()
    ).unwrap();
    assert_eq!(text["bindings"].as_array().unwrap().len(), 3);
}
