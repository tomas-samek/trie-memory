use serde_json::{json, Value};

use trie_memory::mcp::{tools, transport};
use trie_memory::store::concept::ConceptStore;
use trie_memory::store::ContentStore;
use trie_memory::trie::Trie;

fn call(trie: &mut Trie, store: &mut ContentStore, name: &str, args: &Value) -> Value {
    let mut word_trie = Trie::new();
    let mut concepts = ConceptStore::new();
    let mut layers = trie_memory::store::layer::LayerStore::new();
    let mut context = trie_memory::mcp::context::ContextWindow::default();
    tools::call_tool(
        trie,
        &mut word_trie,
        store,
        &mut concepts,
        &mut layers,
        &mut context,
        name,
        args,
    )
}

fn parse_tool_text(result: &Value) -> Value {
    let content = result.get("content").unwrap().as_array().unwrap();
    serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn test_tool_list_has_all_tools() {
    let list = tools::tool_list();
    let tools_arr = list.get("tools").unwrap().as_array().unwrap();
    let names: Vec<&str> = tools_arr
        .iter()
        .map(|t| t.get("name").unwrap().as_str().unwrap())
        .collect();

    assert!(names.contains(&"trie_write"));
    assert!(names.contains(&"trie_read"));
    assert!(names.contains(&"trie_query"));
    assert!(names.contains(&"trie_stats"));
    assert!(names.contains(&"trie_perceive"));
    assert!(names.contains(&"trie_perceive_window"));
    assert!(names.contains(&"trie_remember"));
    assert!(names.contains(&"trie_recall"));
    assert!(names.contains(&"trie_snapshot"));
    assert!(names.contains(&"trie_restore"));
}

#[test]
fn test_trie_write_tool() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    let result = call(&mut trie, &mut store, "trie_write", &json!({"data": "Hello"}));
    let text = parse_tool_text(&result);
    assert_eq!(text["byte"]["tokens_processed"], 5);
}

#[test]
fn test_trie_stats_tool() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    trie.write(b"Test");
    let result = call(&mut trie, &mut store, "trie_stats", &json!({}));
    let text = parse_tool_text(&result);
    assert_eq!(text["node_id"], 0);
    assert!(text["total_nodes"].as_u64().unwrap() >= 1);
}

#[test]
fn test_trie_read_tool() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    trie.write(b"AB");
    let result = call(&mut trie, &mut store, "trie_read", &json!({"leaf_id": 0}));
    assert!(result.get("isError").is_none());
}

#[test]
fn test_trie_query_tool() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    trie.write(b"AAAA");
    let result = call(&mut trie, &mut store, "trie_query", &json!({"pattern": "AA"}));
    assert!(result.get("isError").is_none());
}

#[test]
fn test_unknown_tool_returns_error() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    let result = call(&mut trie, &mut store, "nonexistent", &json!({}));
    assert!(result.get("isError").unwrap().as_bool().unwrap());
}

#[test]
fn test_trie_write_missing_data() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    let result = call(&mut trie, &mut store, "trie_write", &json!({}));
    assert!(result.get("isError").unwrap().as_bool().unwrap());
}

#[test]
fn test_snapshot_restore_roundtrip() {
    let dir = std::env::temp_dir().join("trie-memory-test");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("test-snapshot.dat");
    let path_str = path.to_str().unwrap();

    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    trie.write(b"Hello World");
    let original_nodes = trie.nodes.len();

    let snap_result = call(&mut trie, &mut store, "trie_snapshot", &json!({"path": path_str}));
    assert!(snap_result.get("isError").is_none());

    let mut trie2 = Trie::new();
    let restore_result = call(&mut trie2, &mut store, "trie_restore", &json!({"path": path_str}));
    assert!(restore_result.get("isError").is_none());
    assert_eq!(trie2.nodes.len(), original_nodes);

    std::fs::remove_file(path).ok();
}

#[test]
fn test_transport_roundtrip() {
    let msg = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let mut buf = Vec::new();
    transport::write_message(&mut buf, &msg).unwrap();

    let mut reader = std::io::BufReader::new(&buf[..]);
    let parsed = transport::read_message(&mut reader).unwrap().unwrap();
    assert_eq!(parsed["method"], "tools/list");
}

#[test]
fn test_success_response_format() {
    let resp = transport::success_response(&json!(1), json!({"ok": true}));
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["ok"], true);
}

#[test]
fn test_error_response_format() {
    let resp = transport::error_response(&json!(1), -32601, "Not found");
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["error"]["code"], -32601);
    assert_eq!(resp["error"]["message"], "Not found");
}

// --- Remember / Recall tests ---

#[test]
fn test_remember_and_recall() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();

    let result = call(
        &mut trie,
        &mut store,
        "trie_remember",
        &json!({"content": "The Bronze Age Collapse was caused by disease", "topic": "Bronze Age"}),
    );
    assert!(result.get("isError").is_none());
    let text = parse_tool_text(&result);
    assert_eq!(text["memories_stored"], 1);

    let recall = call(
        &mut trie,
        &mut store,
        "trie_recall",
        &json!({"query": "Bronze Age Collapse disease"}),
    );
    assert!(recall.get("isError").is_none());
    let recall_text = parse_tool_text(&recall);
    let memories = recall_text["memories"].as_array().unwrap();
    assert!(!memories.is_empty(), "Should recall at least one memory");
    assert!(memories[0]["content"]
        .as_str()
        .unwrap()
        .contains("Bronze Age"));
    assert_eq!(memories[0]["topic"], "Bronze Age");
}

#[test]
fn test_remember_different_topics() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();

    call(
        &mut trie,
        &mut store,
        "trie_remember",
        &json!({"content": "Rust is a systems programming language", "topic": "Rust"}),
    );
    call(
        &mut trie,
        &mut store,
        "trie_remember",
        &json!({"content": "Python is great for data science", "topic": "Python"}),
    );

    let text = parse_tool_text(&call(
        &mut trie,
        &mut store,
        "trie_recall",
        &json!({"query": "programming language"}),
    ));
    let memories = text["memories"].as_array().unwrap();
    assert!(memories.len() >= 1);
}

#[test]
fn test_recall_no_match() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();

    let result = call(
        &mut trie,
        &mut store,
        "trie_recall",
        &json!({"query": "something never remembered"}),
    );
    assert!(result.get("isError").is_none());
    let text = parse_tool_text(&result);
    assert_eq!(text["total_matches"], 0);
}

#[test]
fn test_remember_missing_content() {
    let mut trie = Trie::new();
    let mut store = ContentStore::new();
    let result = call(&mut trie, &mut store, "trie_remember", &json!({}));
    assert!(result.get("isError").unwrap().as_bool().unwrap());
}

#[test]
fn test_content_store_persistence() {
    let dir = std::env::temp_dir().join("trie-memory-test-store");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("test-content.json");
    let path_str = path.to_str().unwrap();

    let mut store = ContentStore::new();
    store.add_by_path(trie_memory::store::memory::MemoryEntry {
        timestamp: 1000,
        tick_range: (0, 100),
        content: "test memory".to_string(),
        topic: Some("test".to_string()),
        depth_profile: vec![(0, 5), (1, 2)],
        path_content_ids: vec![Some(123), Some(456)],
        ..Default::default()
    });

    store.save(path_str).unwrap();

    let restored = ContentStore::load(path_str).unwrap();
    assert_eq!(restored.entry_count(), 1);

    let results = restored.recall_by_path(&[Some(123), Some(456)], 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "test memory");

    // Broader match (shallower suffix of the stored path) should also find it.
    let shallower = restored.recall_by_path(&[Some(456)], 10);
    assert_eq!(shallower.len(), 1);

    std::fs::remove_file(path).ok();
}
