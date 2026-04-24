// Honest-agent Phase A integration — Scenarios 1, 2, 3 from Task 08.
//
//   Scenario 1 — No hallucination on unknown query.
//   Scenario 2 — Within-session learning (teach + recall in same session).
//   Scenario 3 — Cross-session learning (teach, snapshot, load, recall).
//
// Runs through the full MCP stack (call_tool + dispatch).

use serde_json::{json, Value};

use trie_memory::mcp::context::ContextWindow;
use trie_memory::mcp::tools;
use trie_memory::store::concept::ConceptStore;
use trie_memory::store::layer::LayerStore;
use trie_memory::store::ContentStore;
use trie_memory::trie::Trie;

struct Stack {
    trie: Trie,
    word_trie: Trie,
    store: ContentStore,
    concepts: ConceptStore,
    layers: LayerStore,
    context: ContextWindow,
}

impl Stack {
    fn new() -> Self {
        Self {
            trie: Trie::new(),
            word_trie: Trie::new(),
            store: ContentStore::new(),
            concepts: ConceptStore::new(),
            layers: LayerStore::new(),
            context: ContextWindow::default(),
        }
    }

    fn call(&mut self, name: &str, args: Value) -> Value {
        tools::call_tool(
            &mut self.trie,
            &mut self.word_trie,
            &mut self.store,
            &mut self.concepts,
            &mut self.layers,
            &mut self.context,
            name,
            &args,
        )
    }
}

fn text(result: &Value) -> Value {
    let content = result.get("content").expect("tool result has content").as_array().unwrap();
    serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap()
}

/// Enough byte volume across several scripts and styles to crystallize root
/// and give the trie a shape, so subsequent writes produce distinct path keys.
fn prime(stack: &mut Stack) {
    let priming = [
        "The corridors were long and empty, frost creeping along the ribbed walls. \
         The light came through slits cut high in the stone, thin as a blade.",
        "Opakování s postupně rostoucími intervaly využívá křivku zapomínání a \
         aktivní vybavování posiluje paměťovou stopu v dlouhodobé paměti.",
        "fn calculate(x: i32) -> i32 { x * 2 + 1 } \
         async fn fetch(url: &str) -> Result<String, Error> { Ok(String::new()) }",
        "日本語で自己紹介をするとき、まず名前を言います。「はじめまして」と \
         言えば丁寧です。東京から来ました。どうぞよろしくお願いします。",
        "SELECT id, name, email FROM users WHERE active = true \
         AND created_at > '2026-01-01' ORDER BY last_login DESC LIMIT 100;",
        "Die Wissenschaft hat festgestellt, dass Kaffee am Morgen wach macht, \
         weil Koffein Adenosinrezeptoren blockiert.",
    ];
    // Two passes so root crystallizes and depth-1 siblings appear.
    for _ in 0..2 {
        for p in &priming {
            stack.call("trie_write", json!({ "data": p }));
        }
    }
}

// ---------- Scenario 1 ----------

#[test]
fn scenario_1_no_hallucination_on_unknown_query() {
    let mut stack = Stack::new();

    // Empty trie — ask about something we have no content for.
    let raw = stack.call(
        "trie_ask",
        json!({ "query": "What is a foo-widget?" }),
    );
    let r = text(&raw);

    assert_eq!(
        r["mode"].as_str(),
        Some("unknown"),
        "empty-trie query must be UNKNOWN, got: {}",
        r
    );
    let supporting = r["supporting"].as_array().unwrap();
    assert!(supporting.is_empty(), "UNKNOWN must have no supporting memories");

    // Response must admit ignorance and must not fabricate a definition.
    let response = r["response"].as_str().unwrap().to_lowercase();
    assert!(
        response.contains("don't") || response.contains("do not"),
        "must admit ignorance, got: {:?}",
        response
    );
    assert!(
        !response.contains("foo-widget is"),
        "must not fabricate a definition, got: {:?}",
        response
    );
}

#[test]
fn scenario_1_unknown_query_on_primed_trie() {
    // Still UNKNOWN on an unseen topic, even once the trie has structure.
    let mut stack = Stack::new();
    prime(&mut stack);

    let raw = stack.call(
        "trie_ask",
        json!({ "query": "What is quantum chromodynamics confinement?" }),
    );
    let r = text(&raw);

    assert_eq!(
        r["mode"].as_str(),
        Some("unknown"),
        "primed-but-uncontented query must be UNKNOWN, got: {}",
        r
    );
}

// ---------- Scenario 2 ----------

#[test]
fn scenario_2_within_session_learning() {
    let mut stack = Stack::new();
    prime(&mut stack);

    // Turn 1: ask about a concept we have not stored yet.
    let t1 = text(&stack.call(
        "trie_ask",
        json!({ "query": "A foo-widget emits on coincident stream deltas." }),
    ));
    assert_eq!(
        t1["mode"].as_str(),
        Some("unknown"),
        "turn 1 must be UNKNOWN before teaching, got: {}",
        t1
    );

    // Turn 2: teach — store the memory explicitly.
    let teach = stack.call(
        "trie_remember",
        json!({
            "content": "A foo-widget emits on coincident stream deltas, bridging two inputs.",
            "topic": "foo-widget",
            "source_type": "user_direct",
            "observer_id": "jerry"
        }),
    );
    assert!(
        teach.get("isError").is_none(),
        "teach must succeed: {}",
        teach
    );

    // Turn 3: ask again with the SAME phrasing (Phase A is literal recall).
    let t3 = text(&stack.call(
        "trie_ask",
        json!({ "query": "A foo-widget emits on coincident stream deltas." }),
    ));

    // This is the actual assertion of Phase A's central claim. If this fails
    // with UNKNOWN, the architecture cannot cover even literal recall given
    // the current trie shape, and that result is itself informative.
    assert_eq!(
        t3["mode"].as_str(),
        Some("answer"),
        "turn 3 after teaching must be ANSWER, got: {}",
        t3
    );

    let response = t3["response"].as_str().unwrap();
    assert!(
        response.contains("foo-widget"),
        "answer must contain the stored content, got: {:?}",
        response
    );
    assert!(
        response.contains("coincident") || response.contains("stream"),
        "answer must quote the stored content, got: {:?}",
        response
    );
}

// ---------- Scenario 3 ----------

fn snapshot_paths(suffix: &str) -> (String, String, String, String) {
    let dir = std::env::temp_dir().join(format!("trie-memory-scenario-{}", suffix));
    std::fs::create_dir_all(&dir).ok();
    (
        dir.join("trie.dat").to_string_lossy().into_owned(),
        dir.join("word-trie.dat").to_string_lossy().into_owned(),
        dir.join("content.json").to_string_lossy().into_owned(),
        dir.join("layer.json").to_string_lossy().into_owned(),
    )
}

#[test]
fn scenario_3_cross_session_learning() {
    let (trie_path, word_path, content_path, layer_path) = snapshot_paths("cross-session");
    // Ensure clean start.
    std::fs::remove_file(&trie_path).ok();
    std::fs::remove_file(&word_path).ok();
    std::fs::remove_file(&content_path).ok();
    std::fs::remove_file(&layer_path).ok();

    // ---- Session 1: prime, teach, snapshot ----
    {
        let mut stack = Stack::new();
        prime(&mut stack);

        stack.call(
            "trie_remember",
            json!({
                "content": "A foo-widget emits on coincident stream deltas, bridging two inputs.",
                "topic": "foo-widget",
                "source_type": "user_direct",
                "observer_id": "jerry"
            }),
        );

        let snap = stack.call(
            "trie_snapshot",
            json!({
                "path": trie_path,
                "word_path": word_path,
                "content_path": content_path,
                "layer_path": layer_path,
            }),
        );
        assert!(
            snap.get("isError").is_none(),
            "snapshot must succeed: {}",
            snap
        );
    }

    // ---- Session 2: fresh stack, restore from disk, ask ----
    {
        let mut stack = Stack::new();
        let restore = stack.call(
            "trie_restore",
            json!({
                "path": trie_path.clone(),
                "word_path": word_path.clone(),
                "content_path": content_path.clone(),
                "layer_path": layer_path.clone(),
            }),
        );
        assert!(
            restore.get("isError").is_none(),
            "restore must succeed: {}",
            restore
        );

        // Context is fresh (default), so this is genuinely a new session.
        assert_eq!(
            stack.context.conversation_depth(),
            0,
            "new session must start with empty context"
        );

        let t = text(&stack.call(
            "trie_ask",
            json!({ "query": "A foo-widget emits on coincident stream deltas." }),
        ));

        assert_eq!(
            t["mode"].as_str(),
            Some("answer"),
            "cross-session recall must still ANSWER, got: {}",
            t
        );
        let response = t["response"].as_str().unwrap();
        assert!(
            response.contains("foo-widget"),
            "cross-session answer must surface the stored content: {:?}",
            response
        );
    }

    std::fs::remove_file(&trie_path).ok();
    std::fs::remove_file(&word_path).ok();
    std::fs::remove_file(&content_path).ok();
}
