// Scenario 5 — correction handling (Task 08).
//
// Teach D1 → teach D2 as correction of D1 → query → expect the agent to
// surface BOTH versions so the user sees a correction occurred. Append-only:
// D1 is not deleted; it's marked `revised_by = D2`.

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
    fn parse(v: &Value) -> Value {
        let c = v.get("content").unwrap().as_array().unwrap();
        serde_json::from_str(c[0]["text"].as_str().unwrap()).unwrap()
    }
}

fn prime(s: &mut Stack) {
    let priming = [
        "The corridors were long and empty, frost creeping along the ribbed walls.",
        "Opakování s postupně rostoucími intervaly využívá křivku zapomínání.",
        "fn calc(x: i32) -> i32 { x * 2 + 1 }",
        "日本語で自己紹介をするとき。",
        "SELECT id FROM users WHERE active = true;",
        "Die Wissenschaft hat festgestellt.",
    ];
    for _ in 0..2 {
        for p in &priming {
            s.call("trie_write", json!({ "data": p }));
        }
    }
}

#[test]
fn scenario_5_correction_surfaces_both() {
    let mut s = Stack::new();
    prime(&mut s);

    // D1: initial teaching.
    let r1 = Stack::parse(&s.call(
        "trie_remember",
        json!({
            "content": "A foo-widget emits on coincident deltas.",
            "topic": "foo-widget",
            "source_type": "user_direct",
            "observer_id": "jerry"
        }),
    ));
    let d1_id = r1["deposit_id"].as_u64().unwrap();

    // D2: correction, with corrects=D1.
    let r2 = Stack::parse(&s.call(
        "trie_remember",
        json!({
            "content": "Actually, a foo-widget emits on delta divergence, not coincidence.",
            "topic": "foo-widget",
            "source_type": "user_correction",
            "observer_id": "jerry",
            "corrects": d1_id
        }),
    ));
    let d2_id = r2["deposit_id"].as_u64().unwrap();
    assert_eq!(d1_id, 0);
    assert_eq!(d2_id, 1);

    // D1 should now carry revised_by = D2. Test via the tool output of a
    // subsequent recall — but we can also assert directly by inspecting
    // the store once we expose get_by_id.
    let d1_after = s.store.get_by_id(d1_id).cloned();
    assert_eq!(
        d1_after.and_then(|e| e.revised_by),
        Some(d2_id),
        "correcting D1 must stamp its revised_by field"
    );

    // Query about foo-widget emission.
    let a = Stack::parse(&s.call(
        "trie_ask",
        json!({ "query": "A foo-widget emits on coincident deltas." }),
    ));
    println!("=== scenario 5 ask result ===");
    println!("{}", serde_json::to_string_pretty(&a).unwrap());

    // The cascade should detect the revision chain and route to Conflicted.
    assert_eq!(
        a["mode"].as_str(),
        Some("conflicted"),
        "correction + original in top bucket must route to Conflicted"
    );

    // Response must show both versions.
    let response = a["response"].as_str().unwrap();
    assert!(
        response.contains("coincident") && response.contains("divergence"),
        "response must surface both versions: {:?}",
        response
    );

    // Supporting must include both D1 and D2.
    let supp = a["supporting"].as_array().unwrap();
    let ids: Vec<u64> = supp
        .iter()
        .filter_map(|m| m["id"].as_u64())
        .collect();
    assert!(ids.contains(&d1_id) && ids.contains(&d2_id));
}

#[test]
fn scenario_5_append_only_d1_preserved() {
    let mut s = Stack::new();
    prime(&mut s);

    let r1 = Stack::parse(&s.call(
        "trie_remember",
        json!({
            "content": "A foo-widget emits on coincident deltas.",
            "topic": "foo-widget",
            "source_type": "user_direct",
            "observer_id": "jerry"
        }),
    ));
    let d1_id = r1["deposit_id"].as_u64().unwrap();

    s.call(
        "trie_remember",
        json!({
            "content": "Actually, a foo-widget emits on delta divergence.",
            "topic": "foo-widget",
            "source_type": "user_correction",
            "corrects": d1_id,
            "observer_id": "jerry"
        }),
    );

    // D1's content must still be in the store — append-only, never mutated.
    let d1 = s.store.get_by_id(d1_id).expect("D1 must still exist");
    assert_eq!(
        d1.content,
        "A foo-widget emits on coincident deltas.",
        "D1's content must not be overwritten by a correction"
    );
}

#[test]
fn scenario_5_no_correction_stays_answer_or_partial() {
    // Sanity: without any correction metadata, a second teach on the same
    // topic should NOT trigger Conflicted; both memories tie at cov and
    // surface as Disambiguate or Partial but never Conflicted.
    let mut s = Stack::new();
    prime(&mut s);

    s.call(
        "trie_remember",
        json!({
            "content": "A foo-widget emits on coincident deltas.",
            "topic": "foo-widget",
            "source_type": "user_direct",
            "observer_id": "jerry"
        }),
    );
    s.call(
        "trie_remember",
        json!({
            "content": "A foo-widget also respects backpressure signals.",
            "topic": "foo-widget",
            "source_type": "user_direct",
            "observer_id": "jerry"
        }),
    );

    let a = Stack::parse(&s.call(
        "trie_ask",
        json!({ "query": "A foo-widget emits on coincident deltas." }),
    ));
    let mode = a["mode"].as_str().unwrap_or("?");
    assert_ne!(
        mode, "conflicted",
        "without correction metadata, mode must not be Conflicted, got: {}",
        mode
    );
}
