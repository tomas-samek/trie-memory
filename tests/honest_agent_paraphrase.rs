// Scenario 4 — paraphrase retrieval (Task 08).
//
// Teach once, then ask five questions with varying phrasing distance
// from the teaching. Phase B's coverage uses exact word match (case-
// insensitive); we expect some queries to honestly degrade to Partial
// or Unknown instead of Answer, and the point of this harness is to
// document exactly where the boundary falls.
//
// Run with: cargo test --test honest_agent_paraphrase -- --nocapture

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

    fn ask(&mut self, q: &str) -> Value {
        let raw = self.call("trie_ask", json!({ "query": q }));
        let content = raw.get("content").unwrap().as_array().unwrap();
        serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap()
    }
}

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
    for _ in 0..2 {
        for p in &priming {
            stack.call("trie_write", json!({ "data": p }));
        }
    }
}

fn report(label: &str, res: &Value) {
    let mode = res["mode"].as_str().unwrap_or("?");
    let suf = res["top_shared_suffix"].as_u64().unwrap_or(0);
    let cov = res["top_coverage_ppm"].as_u64().unwrap_or(0);
    let clarity = res["confidence"]["signal_clarity"].as_u64().unwrap_or(0);
    let strength = res["confidence"]["signal_strength"].as_u64().unwrap_or(0);
    let supporting_len = res["supporting"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let resp: String = res["response"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(160)
        .collect();
    println!(
        "  {:<9} | mode={:<12} suf={} cov={:>4}‰ clarity={:>4}‰ strength={:>4}‰ supp={} ",
        label, mode, suf, cov, clarity, strength, supporting_len
    );
    println!("              response: {}", resp);
}

#[test]
fn scenario_4_paraphrase_harness() {
    let mut s = Stack::new();
    prime(&mut s);

    // Teach exactly once — Task 08 Setup.
    s.call(
        "trie_remember",
        json!({
            "content": "A foo-widget connects two streams by emitting on coincident deltas.",
            "topic": "foo-widget",
            "source_type": "user_direct",
            "observer_id": "tester"
        }),
    );

    println!("\n===== SCENARIO 4: PARAPHRASE RETRIEVAL =====\n");
    println!(
        "Stored: 'A foo-widget connects two streams by emitting on coincident deltas.'\n"
    );

    // Spec queries.
    let qa = s.ask("How does foo-widget work?");
    report("A literal", &qa);

    let qb = s.ask("What links streams together?");
    report("B topical", &qb);

    let qc = s.ask("Tell me about coincident delta handling.");
    report("C keywords", &qc);

    let qd = s.ask("Explain the protocol for bridging streams.");
    report("D synonyms", &qd);

    let qe = s.ask("What color is a foo-widget?");
    report("E off-topic", &qe);

    println!("\nExpectations (Task 08):");
    println!("  A (direct): ANSWER");
    println!("  B (topical): ANSWER or PARTIAL");
    println!("  C (keyword-overlap): PARTIAL or ANSWER");
    println!("  D (synonym, bridge): PARTIAL");
    println!("  E (off-topic): UNKNOWN or PARTIAL (no color info stored)\n");

    // Collect scoring.
    let modes = [
        ("A", qa["mode"].as_str().unwrap_or("?")),
        ("B", qb["mode"].as_str().unwrap_or("?")),
        ("C", qc["mode"].as_str().unwrap_or("?")),
        ("D", qd["mode"].as_str().unwrap_or("?")),
        ("E", qe["mode"].as_str().unwrap_or("?")),
    ];

    // Grade: 2 pts for meeting the lower bar, 1 pt for adjacent mode,
    // 0 pts for complete miss (wrong direction — fabricating when
    // should be unknown, or unknown when memory should fire).
    fn grade(label: &str, got: &str) -> u32 {
        match (label, got) {
            // A "retrieves the teaching content" per Task 08. Partial still
            // includes the supporting memory; Answer is cleaner but Partial
            // is not a miss.
            ("A", "answer") | ("A", "partial") => 2,
            ("A", _) => 0,
            ("B", "answer") | ("B", "partial") => 2,
            ("B", "unknown") => 1,
            ("B", _) => 0,
            ("C", "answer") | ("C", "partial") => 2,
            ("C", "unknown") => 1,
            ("C", _) => 0,
            ("D", "partial") => 2,
            ("D", "answer") | ("D", "unknown") => 1,
            ("D", _) => 0,
            ("E", "unknown") | ("E", "partial") => 2,
            ("E", "answer") => 0, // fabrication risk
            ("E", _) => 1,
            _ => 0,
        }
    }

    let mut total = 0u32;
    println!("Grade:");
    for (label, got) in &modes {
        let g = grade(label, got);
        total += g;
        println!("  {} got={:<10} grade={}/2", label, got, g);
    }
    println!("  TOTAL: {}/10\n", total);

    // Hard asserts only on the non-negotiable contract:
    //   E must NOT be ANSWER (fabrication would be catastrophic).
    //   A must NOT be UNKNOWN (literal recall is Phase A's baseline).
    assert_ne!(qe["mode"].as_str(), Some("answer"),
        "E (off-topic) must not answer — that would be fabrication");
    assert_ne!(qa["mode"].as_str(), Some("unknown"),
        "A (literal) must find the memory; Phase A baseline was already passing this");
}
