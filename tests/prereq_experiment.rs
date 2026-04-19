// Prerequisite experiment for honest_agent Phase A.
//
// The 2026-04 baseline (on a depth-1 trie) showed that short queries resonated
// with everything: "spaced repetition" did not recall the memory tagged
// "opakování, spaced repetition, aktivní vybavování"; structurally richer but
// off-topic memories dominated. Long, context-dense queries succeeded.
//
// After TASK.md landed, the trie builds real depth. This test replays the
// short-vs-long scenario across several topics and prints the current
// behavior so Phase A thresholds can be calibrated against post-fix reality,
// not pre-fix memory.
//
// Run with:   cargo test --test prereq_experiment -- --nocapture

use trie_memory::store::memory::MemoryEntry;
use trie_memory::store::ContentStore;
use trie_memory::trie::tokenizer::{silence_ticks, split_words, tokenize_with_silence, word_token, WordOrSilence};
use trie_memory::trie::Trie;

fn load_corpus() -> Vec<(String, String)> {
    let raw = std::fs::read_to_string("tests/fixtures/corpus.md")
        .expect("corpus fixture missing; expected tests/fixtures/corpus.md");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current_topic: Option<String> = None;
    let mut current_body = String::new();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("## topic:") {
            if let Some(t) = current_topic.take() {
                out.push((t, std::mem::take(&mut current_body).trim().to_string()));
            }
            current_topic = Some(rest.trim().to_string());
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    if let Some(t) = current_topic {
        out.push((t, current_body.trim().to_string()));
    }
    out
}

fn depth_histogram(t: &Trie) -> Vec<(u32, usize)> {
    let total = t.stats(None).map(|s| s.total_nodes).unwrap_or(0);
    let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for id in 0..total as u64 {
        if let Some(s) = t.stats(Some(id)) {
            *counts.entry(s.depth).or_insert(0) += 1;
        }
    }
    counts.into_iter().collect()
}

fn distinct_store_keys(store: &ContentStore) -> usize {
    store.distinct_path_keys()
}

fn feed_word_trie(word_trie: &mut Trie, content: &str) {
    for item in tokenize_with_silence(content) {
        match item {
            WordOrSilence::Word(w) => {
                let token = word_token(&w);
                word_trie.write(&token);
            }
            WordOrSilence::Silence(gap) => {
                for _ in 0..gap {
                    word_trie.next_tick();
                }
            }
        }
    }
    // Sentence-boundary tick at the end of each memory.
    for _ in 0..silence_ticks('.') {
        word_trie.next_tick();
    }
}

fn query_word_trie(word_trie: &Trie, pattern: &str) -> Vec<(String, u64, u32)> {
    split_words(pattern)
        .into_iter()
        .map(|w| {
            let token = word_token(&w);
            let qr = word_trie.query(&token);
            (w, qr.deepest_node, qr.match_depth)
        })
        .collect()
}

fn feed_memory(trie: &mut Trie, store: &mut ContentStore, content: &str, topic: &str) {
    let tick_start = trie.tick.load(std::sync::atomic::Ordering::Relaxed);
    trie.write(content.as_bytes());
    let tick_end = trie.tick.load(std::sync::atomic::Ordering::Relaxed);

    let qr = trie.query(content.as_bytes());
    let path_key = trie.path_key(qr.deepest_node).unwrap_or_default();
    // Timestamp = tick_end so memories are ordered by feed sequence; otherwise
    // stable sort with zero timestamps masks recall ordering.
    let entry = MemoryEntry {
        timestamp: tick_end,
        tick_range: (tick_start, tick_end),
        content: content.to_string(),
        topic: Some(topic.to_string()),
        depth_profile: qr.matches_per_depth.clone(),
        path_content_ids: path_key,
        ..Default::default()
    };
    store.add_by_path(entry);
}

fn report_word_trie(word_trie: &Trie, label: &str, query: &str) {
    let per_word = query_word_trie(word_trie, query);
    let mut depths: Vec<u32> = per_word.iter().map(|(_, _, d)| *d).collect();
    depths.sort_unstable();
    let distinct_nodes: std::collections::HashSet<u64> =
        per_word.iter().map(|(_, n, _)| *n).collect();

    println!();
    println!("WORD-TRIE QUERY [{}]", label);
    println!("  input     : {:?}", query);
    println!("  words     : {}", per_word.len());
    println!("  distinct  : {} node(s)", distinct_nodes.len());
    println!("  depths    : {:?}", depths);
    for (w, node, d) in &per_word {
        println!("    {:?} -> node{} depth{}", w, node, d);
    }
}

fn report(trie: &Trie, store: &ContentStore, label: &str, query: &str) {
    let qr = trie.query(query.as_bytes());
    let path_key = trie.path_key(qr.deepest_node).unwrap_or_default();
    let memories = store.recall_by_path(&path_key, 3);

    println!();
    println!("QUERY [{}]  bytes={}  deepest=node{}  depth={}",
        label, query.len(), qr.deepest_node, qr.match_depth);
    println!("  input      : {:?}", query);
    println!("  depth_prof : {:?}", qr.matches_per_depth);
    println!("  recalled   : {} memory(ies)", memories.len());
    for (i, m) in memories.iter().enumerate() {
        let preview: String = m.content.chars().take(70).collect();
        println!("    [{}] topic={:?}", i, m.topic);
        println!("        {:?}", preview);
    }
}

#[test]
fn prereq_short_vs_long_queries() {
    let mut trie = Trie::new();
    let mut word_trie = Trie::new();
    let mut store = ContentStore::new();

    // Load corpus from tests/fixtures/corpus.md (~9KB, 12 topics,
    // three writing systems: Latin/English, Latin/Czech, CJK/Japanese,
    // plus code and structured data).
    let corpus = load_corpus();
    assert!(!corpus.is_empty(), "corpus fixture loaded empty");

    let total_bytes: usize = corpus.iter().map(|(_, c)| c.len()).sum();
    println!("=== Corpus ===");
    println!("  topics      : {}", corpus.len());
    println!("  total bytes : {}", total_bytes);

    // Feed each topic twice so crystallized nodes settle.
    for _ in 0..2 {
        for (topic, content) in &corpus {
            feed_memory(&mut trie, &mut store, content, topic);
            feed_word_trie(&mut word_trie, content);
        }
    }

    // Byte-trie shape summary.
    let total_nodes = trie.stats(None).map(|s| s.total_nodes).unwrap_or(0);
    let mut max_depth = 0u32;
    for id in 0..total_nodes as u64 {
        if let Some(s) = trie.stats(Some(id)) {
            if s.depth > max_depth {
                max_depth = s.depth;
            }
        }
    }

    // Word-trie shape summary.
    let word_nodes = word_trie.stats(None).map(|s| s.total_nodes).unwrap_or(0);
    let mut word_max_depth = 0u32;
    for id in 0..word_nodes as u64 {
        if let Some(s) = word_trie.stats(Some(id)) {
            if s.depth > word_max_depth {
                word_max_depth = s.depth;
            }
        }
    }

    let byte_depth_hist = depth_histogram(&trie);
    let word_depth_hist = depth_histogram(&word_trie);

    println!("=== Byte-trie shape after feeding ===");
    println!("  total nodes : {}", total_nodes);
    println!("  max depth   : {}", max_depth);
    println!("  per depth   : {:?}", byte_depth_hist);
    println!("=== Word-trie shape after feeding ===");
    println!("  total nodes : {}", word_nodes);
    println!("  max depth   : {}", word_max_depth);
    println!("  per depth   : {:?}", word_depth_hist);
    println!("=== ContentStore ===");
    println!("  memories    : {}", store.entry_count());
    let distinct_keys = distinct_store_keys(&store);
    println!("  distinct keys (path indexing surface) : {}", distinct_keys);

    // --- Byte-trie: short vs long ---
    println!("\n========== BYTE-TRIE RECALL ==========");
    report(&trie, &store, "short:spaced",  "spaced repetition");
    report(&trie, &store, "long:spaced",   "opakování, spaced repetition, aktivní vybavování");
    report(&trie, &store, "short:agent",   "komunikace agentů");
    report(&trie, &store, "long:agent",    "komunikace mezi agenty v časově kódovaných zprávách");
    report(&trie, &store, "short:cooking", "kuře");
    report(&trie, &store, "long:cooking",  "pečené kuře s bramborami a rozmarýnem");
    report(&trie, &store, "short:code",    "fn main");
    report(&trie, &store, "long:code",     "fn main() { let x: Vec<u8> = vec![1,2,3]; }");
    report(&trie, &store, "short:unknown", "quantum chromodynamics");
    report(&trie, &store, "long:unknown",  "quantum chromodynamics gauge theory color charge confinement");

    // --- Word-trie: per-word depth on same queries ---
    println!("\n========== WORD-TRIE RECALL ==========");
    report_word_trie(&word_trie, "short:spaced",  "spaced repetition");
    report_word_trie(&word_trie, "long:spaced",   "opakování, spaced repetition, aktivní vybavování");
    report_word_trie(&word_trie, "short:agent",   "komunikace agentů");
    report_word_trie(&word_trie, "long:agent",    "komunikace mezi agenty v časově kódovaných zprávách");
    report_word_trie(&word_trie, "short:cooking", "kuře");
    report_word_trie(&word_trie, "long:cooking",  "pečené kuře s bramborami a rozmarýnem");
    report_word_trie(&word_trie, "short:code",    "fn main");
    report_word_trie(&word_trie, "long:code",     "fn main Vec u8 vec println");
    report_word_trie(&word_trie, "short:unknown", "quantum chromodynamics");
    report_word_trie(&word_trie, "long:unknown",  "quantum chromodynamics gauge theory color charge confinement");

    // This test is diagnostic; it always passes. Results are captured from
    // --nocapture output into docs/session_2026_04_19.md for analysis.
    assert!(total_nodes > 1, "trie should have grown beyond root");
}
