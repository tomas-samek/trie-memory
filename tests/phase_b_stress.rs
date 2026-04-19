// Phase B stress test — larger corpus, 20+ queries across categories,
// honest pass/fail per query. Meant to shake out failure modes the
// tidy unit tests miss.
//
// Categories:
//   L  literal / near-literal recall
//   P  paraphrase (word overlap but different phrasing)
//   X  cross-topic distinctness (must not leak across taught topics)
//   D  disambiguate (two memories about the same concept)
//   U  unknown (off-topic queries)
//   T  trust / provenance (low-trust source must surface)
//   Z  stopword-heavy / empty-content queries
//   C  cross-language (Czech)
//   S  cross-session persistence under stress
//
// Run with: cargo test --test phase_b_stress -- --nocapture

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

fn prime(s: &mut Stack) {
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
            s.call("trie_write", json!({ "data": p }));
        }
    }
}

fn teach(s: &mut Stack, content: &str, topic: &str, source: &str) {
    let r = s.call(
        "trie_remember",
        json!({
            "content": content,
            "topic": topic,
            "source_type": source,
            "observer_id": "stress-test",
        }),
    );
    let text = r.get("content").and_then(|c| c.as_array()).and_then(|a| a.first())
        .and_then(|v| v["text"].as_str()).unwrap_or("");
    let parsed: Value = serde_json::from_str(text).unwrap_or(json!({}));
    eprintln!(
        "  teach[{}] path={}",
        topic,
        parsed["path_key"]
    );
}

fn teach_corpus(s: &mut Stack) {
    // 11 memories across 10 topics; one topic has two entries to exercise
    // Disambiguate.
    teach(s,
        "A semaphore is a synchronization primitive that counts permits.",
        "semaphore", "user_direct");
    teach(s,
        "Semaphores can be binary or counting; a binary semaphore acts like a mutex.",
        "semaphore-variants", "user_direct");
    teach(s,
        "A mutex is a mutual exclusion lock held by exactly one owner at a time.",
        "mutex", "user_direct");
    teach(s,
        "Rust async runtime schedules futures onto thread-pool workers via a reactor loop.",
        "async-runtime", "user_direct");
    teach(s,
        "The bay of Fundy has tidal ranges up to fifteen meters at extreme spring tides.",
        "tides", "user_direct");
    teach(s,
        "Coffee contains caffeine which blocks adenosine receptors in the brain.",
        "coffee", "user_direct");
    teach(s,
        "Spaced repetition uses widening intervals to strengthen memory traces over time.",
        "spaced-repetition", "user_direct");
    teach(s,
        "SQL INNER JOIN returns rows where the key matches in both joined tables.",
        "sql-join", "user_direct");
    teach(s,
        "Rust lifetime annotations mark how long a reference is borrowed within a scope.",
        "rust-lifetimes", "user_direct");
    teach(s,
        "Pečené kuře s rozmarýnem a bramborami se peče 45 minut na 180 stupňů.",
        "kuře", "user_direct");
    // Low-trust chaff.
    teach(s,
        "Drinking coffee reverses aging and grants eternal youth, studies suggest.",
        "coffee-claim-web", "web_fetched");
}

#[derive(Debug)]
struct Case {
    cat: &'static str,
    label: &'static str,
    query: &'static str,
    expect_modes: &'static [&'static str], // any of these passes
    expect_topic_contains: Option<&'static str>, // substring of top topic, if checkable
    // Hard fail: these modes are never acceptable
    forbid_modes: &'static [&'static str],
}

fn cases() -> Vec<Case> {
    vec![
        // L - literal / near-literal
        Case { cat: "L", label: "semaphore literal",
            query: "A semaphore is a synchronization primitive that counts permits.",
            expect_modes: &["answer"], expect_topic_contains: Some("semaphore"),
            forbid_modes: &["unknown"] },
        Case { cat: "L", label: "mutex near-literal",
            query: "What is a mutex? A mutex is a mutual exclusion lock.",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("mutex"),
            forbid_modes: &["unknown"] },
        Case { cat: "L", label: "sql-join literal",
            query: "SQL INNER JOIN returns rows where the key matches in both tables.",
            expect_modes: &["answer"], expect_topic_contains: Some("sql"),
            forbid_modes: &["unknown"] },
        // P - paraphrase
        Case { cat: "P", label: "how do threads coordinate",
            query: "How do multiple threads coordinate on shared resources?",
            // Deep-semantic paraphrase; honest outcome is Partial or Unknown.
            expect_modes: &["partial", "unknown"], expect_topic_contains: None,
            forbid_modes: &[] },
        Case { cat: "P", label: "thread-pool futures",
            query: "Who schedules Rust futures onto worker threads?",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("async"),
            forbid_modes: &[] },
        Case { cat: "P", label: "caffeine sleepy",
            query: "Why does caffeine keep me alert?",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("coffee"),
            forbid_modes: &[] },
        // X - cross-topic distinctness (must not leak)
        Case { cat: "X", label: "tides vs semaphore",
            query: "Why do tides have such large ranges in some bays?",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("tide"),
            forbid_modes: &[] },
        Case { cat: "X", label: "coffee vs mutex",
            query: "Tell me about caffeine chemistry in the morning.",
            expect_modes: &["answer", "partial", "unknown"], expect_topic_contains: None,
            forbid_modes: &[] },
        // D - disambiguate (two semaphore memories)
        Case { cat: "D", label: "semaphore ambiguous",
            query: "Give me details about semaphores please.",
            expect_modes: &["disambiguate", "answer", "partial"], expect_topic_contains: Some("semaphore"),
            forbid_modes: &[] },
        // U - unknown (no related memory)
        Case { cat: "U", label: "cookies",
            query: "What temperature for chocolate chip cookies?",
            expect_modes: &["unknown"], expect_topic_contains: None,
            forbid_modes: &["answer"] },
        Case { cat: "U", label: "quantum chromodynamics",
            query: "Explain quantum chromodynamics gauge theory.",
            expect_modes: &["unknown"], expect_topic_contains: None,
            forbid_modes: &["answer"] },
        Case { cat: "U", label: "untaught concept",
            query: "What are the rules of bridge card game?",
            expect_modes: &["unknown"], expect_topic_contains: None,
            forbid_modes: &["answer"] },
        // T - trust
        Case { cat: "T", label: "web-claim recall",
            query: "Does drinking coffee reverse aging?",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("web"),
            forbid_modes: &["unknown"] },
        // Z - stopword-heavy / empty content
        Case { cat: "Z", label: "empty-ish question",
            query: "What is it about?",
            expect_modes: &["unknown"], expect_topic_contains: None,
            forbid_modes: &["answer"] },
        Case { cat: "Z", label: "short fragment",
            query: "and then",
            expect_modes: &["unknown"], expect_topic_contains: None,
            forbid_modes: &["answer"] },
        // C - cross-language
        Case { cat: "C", label: "Czech kuře literal",
            query: "Pečené kuře s rozmarýnem a bramborami",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("kuře"),
            forbid_modes: &["unknown"] },
        Case { cat: "C", label: "Czech kuře shortened",
            query: "pečené kuře rozmarýn",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("kuře"),
            forbid_modes: &[] },
        // S - context pollution: ask three turns on unrelated topics back-to-back
        Case { cat: "S", label: "context pollution turn 1",
            query: "What is a semaphore?",
            // Two stored semaphore memories (semaphore / semaphore-variants)
            // both tie at cov=500‰ so Disambiguate is honest here.
            expect_modes: &["answer", "partial", "disambiguate"],
            expect_topic_contains: Some("semaphore"),
            forbid_modes: &[] },
        Case { cat: "S", label: "context pollution turn 2",
            query: "Now tell me about tides ranges",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("tide"),
            forbid_modes: &[] },
        Case { cat: "S", label: "context pollution turn 3",
            query: "What about SQL INNER JOIN behavior",
            expect_modes: &["answer", "partial"], expect_topic_contains: Some("sql"),
            forbid_modes: &[] },
    ]
}

#[test]
fn phase_b_stress() {
    let mut s = Stack::new();
    prime(&mut s);
    teach_corpus(&mut s);

    println!("\n======== PHASE B STRESS TEST ========\n");

    let mut pass = 0u32;
    let mut topic_match = 0u32;
    let mut hard_fail = 0u32;
    let mut per_cat: std::collections::BTreeMap<&str, (u32, u32, u32)> =
        std::collections::BTreeMap::new();
    let mut rows: Vec<String> = Vec::new();

    let cases = cases();
    for c in &cases {
        let r = s.ask(c.query);
        let mode = r["mode"].as_str().unwrap_or("?");
        let cov = r["top_coverage_ppm"].as_u64().unwrap_or(0);
        let top_topic = r["supporting"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|m| m["topic"].as_str())
            .unwrap_or("");
        let path_key_dbg = r["recognition"]["path_key"].to_string();

        let mode_pass = c.expect_modes.iter().any(|m| *m == mode);
        let mode_hard = c.forbid_modes.iter().any(|m| *m == mode);

        let topic_pass = match c.expect_topic_contains {
            None => true,
            Some(needle) => top_topic.contains(needle),
        };

        let outcome = if mode_hard {
            hard_fail += 1;
            "HARD FAIL"
        } else if mode_pass && topic_pass {
            pass += 1;
            if c.expect_topic_contains.is_some() {
                topic_match += 1;
            }
            "pass"
        } else if mode_pass {
            pass += 1; // mode correct but topic didn't match expected
            "pass (topic off)"
        } else {
            "soft fail"
        };

        let row = format!(
            "  [{}] {:<30} mode={:<12} top={:<24} cov={:>4}‰ path={} {}",
            c.cat, c.label, mode, top_topic, cov, path_key_dbg, outcome
        );
        rows.push(row.clone());
        println!("{}", row);

        let entry = per_cat.entry(c.cat).or_insert((0, 0, 0));
        entry.0 += 1;
        if !mode_hard && mode_pass {
            entry.1 += 1;
        }
        if mode_hard {
            entry.2 += 1;
        }
    }

    println!("\n---- per category ----");
    for (cat, (total, ok, hard)) in &per_cat {
        println!(
            "  {} : {:>2}/{:>2} pass, {} hard fail",
            cat, ok, total, hard
        );
    }

    println!(
        "\nTOTAL: {}/{} pass, {} topic-precise, {} hard fail",
        pass,
        cases.len(),
        topic_match,
        hard_fail
    );

    // Hard-fail gate: we will NOT accept Answer-on-unknown (fabrication) or
    // Unknown-on-literal (broken recall). Soft fails are diagnostic.
    assert_eq!(
        hard_fail, 0,
        "hard failures detected — fabrication risk or broken literal recall"
    );

    // Baseline floor: at least 70% mode-pass rate across the stress set.
    // Calibrated on the current Phase B implementation so regressions bark.
    let total = cases.len() as u32;
    assert!(
        pass * 100 / total >= 70,
        "mode-pass rate {}/{} below 70% floor",
        pass,
        total
    );
}
