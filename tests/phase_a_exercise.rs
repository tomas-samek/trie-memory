// Phase A exercise — drive the honest-agent stack through realistic
// conversations and capture what actually happens.
//
// Unlike the unit-level scenario tests, this is a diagnostic pass. Each
// sub-scenario asserts what Phase A is contracted to do and prints the raw
// response so we can write up honest findings in
// `docs/session_2026_04_19_phase_a_exercise.md`.
//
// Run with:
//   cargo test --test phase_a_exercise -- --nocapture

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

    fn teach(&mut self, content: &str, topic: &str, source: &str) -> Value {
        self.call(
            "trie_remember",
            json!({
                "content": content,
                "topic": topic,
                "source_type": source,
                "observer_id": "tester",
            }),
        )
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

fn report(label: &str, result: &Value) {
    println!("--- {} ---", label);
    println!("  mode        : {}", result["mode"].as_str().unwrap_or("?"));
    println!(
        "  reasoning   : {}",
        result["reasoning"].as_str().unwrap_or("")
    );
    println!(
        "  shared      : {}",
        result["top_shared_suffix"].as_u64().unwrap_or(0)
    );
    let recognition = &result["recognition"];
    println!(
        "  match_depth : {}  deepest_node : {}",
        recognition["match_depth"].as_u64().unwrap_or(0),
        recognition["deepest_node"].as_u64().unwrap_or(0),
    );
    let topics: Vec<String> = result["query"]["topics_used"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    println!("  topics_used : {:?}", topics);
    let supporting = result["supporting"].as_array().cloned().unwrap_or_default();
    println!("  supporting  : {} memory(ies)", supporting.len());
    for m in supporting.iter().take(3) {
        let topic = m["topic"].as_str().unwrap_or("-");
        let content: String = m["content"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(90)
            .collect();
        println!("    [{}] {}", topic, content);
    }
    let resp = result["response"].as_str().unwrap_or("");
    let trimmed: String = resp.chars().take(220).collect();
    println!("  response    : {}", trimmed);
    println!();
}

// -------- one big multi-part exercise --------
#[test]
fn phase_a_exercise() {
    let mut s = Stack::new();
    prime(&mut s);

    println!("\n==================== PHASE A EXERCISE ====================\n");

    // ---- A. Literal recall (known-good baseline) ----
    println!(">>> A. Literal recall (baseline)");
    let before_teach = s.ask("A semaphore is a synchronization primitive that counts permits.");
    report("A.1 before teach (expect unknown)", &before_teach);
    assert_eq!(before_teach["mode"].as_str(), Some("unknown"));

    s.teach(
        "A semaphore is a synchronization primitive that counts permits.",
        "semaphore",
        "user_direct",
    );
    let after_teach =
        s.ask("A semaphore is a synchronization primitive that counts permits.");
    report("A.2 after teach, identical phrasing (expect answer)", &after_teach);
    assert_eq!(after_teach["mode"].as_str(), Some("answer"));

    // ---- B. Near-literal recall ----
    println!(">>> B. Near-literal recall (small phrasing change)");
    let near = s.ask("A semaphore is a synchronization primitive that counts permits today.");
    report("B.1 trailing word added", &near);
    // No assertion — diagnostic. Document whichever way it lands.

    let rephrased =
        s.ask("What is a semaphore? It's a synchronization primitive that counts permits.");
    report("B.2 prefix-reordered phrasing", &rephrased);

    // ---- C. Paraphrase (preview of Phase B acid test) ----
    println!(">>> C. Paraphrase (Phase B acid test preview)");
    let para = s.ask("How do threads coordinate with shared counters?");
    report("C.1 paraphrase of same concept", &para);
    // No assertion — we *expect* UNKNOWN here in Phase A. Logging is the point.

    // ---- D. Unrelated query (false-positive check — diagnostic only) ----
    // Phase A's mode selector uses a byte-trie path-suffix heuristic. At small
    // corpora it can false-positive: any English query may share enough trie
    // path with the taught memory to land in Answer mode even when the topic
    // is obviously unrelated. We report the outcome instead of asserting, so
    // the finding gets captured rather than masked.
    println!(">>> D. Unrelated query on primed+taught trie (diagnostic)");
    let unrelated = s.ask("What temperature for chocolate chip cookies?");
    report("D.1 unrelated topic", &unrelated);
    let d_false_positive = unrelated["mode"].as_str() == Some("answer");
    if d_false_positive {
        println!(
            "  ⚠ FALSE POSITIVE: cookies query returned semaphore memory. \
             Phase A path-suffix rule is too permissive at this trie scale."
        );
    }

    // ---- E. Multi-memory in same session ----
    println!(">>> E. Two taught topics, no cross-pollination");
    s.teach(
        "The bay of Fundy has tidal ranges up to fifteen meters at extreme spring tides.",
        "tides",
        "user_direct",
    );
    let sema_recall =
        s.ask("A semaphore is a synchronization primitive that counts permits.");
    report("E.1 semaphore query (should still be semaphore)", &sema_recall);
    // Check only the top supporting memory — "Also matched" entries are
    // fine, they just document siblings at the same suffix depth.
    let sema_top_topic = sema_recall["supporting"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["topic"].as_str())
        .unwrap_or("");
    let sema_ok = sema_recall["mode"].as_str() == Some("answer")
        && sema_top_topic == "semaphore";
    if !sema_ok {
        println!(
            "  ⚠ E.1 regression: semaphore query's top memory was {:?}, expected semaphore",
            sema_top_topic
        );
    }

    let tides_recall =
        s.ask("The bay of Fundy has tidal ranges up to fifteen meters at extreme spring tides.");
    report("E.2 tides query (should return tides)", &tides_recall);
    let tides_text = tides_recall["response"].as_str().unwrap_or("");
    let tides_ok = tides_recall["mode"].as_str() == Some("answer")
        && tides_text.contains("Fundy");
    if !tides_ok {
        println!(
            "  ⚠ E.2 regression: tides query did not return tides memory"
        );
    }

    // ---- F. Context enrichment effect ----
    println!(">>> F. Context enrichment — short follow-up after topic exposure");
    s.call("context_reset", json!({ "session_id": "ctx-exp" }));
    // First turn sets context.
    let t1 = s.ask("A semaphore is a synchronization primitive that counts permits.");
    report("F.1 first turn (primes context with topic=semaphore)", &t1);
    // Short ambiguous follow-up.
    let t2 = s.ask("what about today");
    report("F.2 short follow-up (context may inject topic=semaphore)", &t2);
    // The enriched query should carry the topic suffix.
    let enriched = t2["query"]["enriched"].as_str().unwrap_or("");
    let topics_used: Vec<String> = t2["query"]["topics_used"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let ctx_injected = topics_used.contains(&"semaphore".to_string())
        || enriched.contains("semaphore");
    if !ctx_injected {
        println!(
            "  ⚠ F.2 context did not inject prior topic: enriched={:?}, topics={:?}",
            enriched, topics_used
        );
    }

    // ---- G. Trust/provenance surfaces ----
    println!(">>> G. Trust surfaces in the rendered response");
    s.call("context_reset", json!({ "session_id": "trust-exp" }));
    s.teach(
        "Drinking coffee reverses aging, studies show (maybe).",
        "coffee-claim",
        "web_fetched",
    );
    let web = s.ask("Drinking coffee reverses aging, studies show (maybe).");
    report("G.1 web-sourced memory recall", &web);
    let web_resp = web["response"].as_str().unwrap_or("").to_lowercase();
    let trust_surfaced = web_resp.contains("web") || web_resp.contains("lower trust");
    if !trust_surfaced {
        println!(
            "  ⚠ G.1 web-source trust tag missing from rendered response: {:?}",
            web_resp
        );
    }

    // ---- H. Summary stats ----
    println!(">>> H. Final trie + store shape");
    let stats = s.call("trie_stats", json!({}));
    println!("{}", serde_json::to_string_pretty(&stats).unwrap_or_default());
}
