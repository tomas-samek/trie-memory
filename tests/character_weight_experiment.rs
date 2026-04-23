// Character-level weight + silence experiment.
//
// Question: can one trie, fed at the character scale with a non-flat weight
// encoding and silence-as-tick-advance for whitespace / punctuation,
// develop structure that discriminates topic-sized patterns without any
// word-level pre-tokenization on top?
//
// Three encodings are run on the identical corpus:
//
//   A) byte-trie          raw UTF-8 bytes, tick advances per byte (current).
//   B) weight-inline      char_weight() for every char *including* spaces
//                         and punct, tick advances per char. No silence.
//   C) weight + silence   char_weight() only for content chars; whitespace
//                         and punctuation advance the tick counter but
//                         write nothing. Content runs are written per
//                         "burst", so delta encoding resets between runs.
//
// This is the user's proposed model: spaces have no content but shift time;
// letters carry weight; words emerge from bursts separated by short silences.
//
// Run with:  cargo test --test character_weight_experiment -- --nocapture

use trie_memory::trie::tokenizer::{char_weight, weight_encode};
use trie_memory::trie::Trie;

fn is_silence_char(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '.' | ',' | ';' | ':' | '!' | '?' | '_' | '-' | '/' | '\\'
                | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        )
}

/// Tick-gap tiered by punctuation class. Within-word = 0 (no gap), space = 1,
/// comma = 3, semicolon/colon = 5, sentence-end = 10, newline = 20.
fn silence_ticks_char(ch: char) -> u64 {
    match ch {
        ' ' | '\t' => 1,
        '-' | '_' | '/' | '\\' => 1,
        ',' => 3,
        ';' | ':' => 5,
        '.' | '!' | '?' => 10,
        '\n' | '\r' => 20,
        _ => 1,
    }
}

/// (C) Feed: content chars → weight-encoded tokens written as a run;
/// silence chars → advance the tick counter without writing. Content runs
/// are written separately so each starts from `previous = 128` in the
/// delta encoder, the same way a word boundary would.
fn feed_weight_with_silence(trie: &mut Trie, input: &str) -> (usize, u64) {
    let mut run: Vec<u8> = Vec::new();
    let mut tokens_written = 0usize;
    let mut total_silence = 0u64;

    for ch in input.chars() {
        if is_silence_char(ch) {
            if !run.is_empty() {
                let r = trie.write(&run);
                tokens_written += r.tokens_processed;
                run.clear();
            }
            let gap = silence_ticks_char(ch);
            for _ in 0..gap {
                trie.next_tick();
            }
            total_silence += gap;
        } else {
            run.push(char_weight(ch));
        }
    }
    if !run.is_empty() {
        let r = trie.write(&run);
        tokens_written += r.tokens_processed;
    }
    (tokens_written, total_silence)
}

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
    let mut counts: std::collections::BTreeMap<u32, usize> =
        std::collections::BTreeMap::new();
    for id in 0..total as u64 {
        if let Some(s) = t.stats(Some(id)) {
            *counts.entry(s.depth).or_insert(0) += 1;
        }
    }
    counts.into_iter().collect()
}

fn shape_line(label: &str, t: &Trie) {
    let total = t.stats(None).map(|s| s.total_nodes).unwrap_or(0);
    let mut max_d = 0u32;
    for id in 0..total as u64 {
        if let Some(s) = t.stats(Some(id)) {
            if s.depth > max_d {
                max_d = s.depth;
            }
        }
    }
    let hist = depth_histogram(t);
    println!(
        "  {:<18} total={:>3}  max_depth={:>2}  histogram={:?}",
        label, total, max_d, hist
    );
}

fn path_for(trie: &Trie, input: &str) -> Vec<Option<u64>> {
    let qr = trie.query(input.as_bytes());
    trie.path_key(qr.deepest_node).unwrap_or_default()
}

fn path_for_weight_inline(trie: &Trie, input: &str) -> Vec<Option<u64>> {
    let bytes = weight_encode(input);
    let qr = trie.query(&bytes);
    trie.path_key(qr.deepest_node).unwrap_or_default()
}

/// Query for the weight+silence encoding: replay runs, take the path_key
/// at the deepest (highest match_depth) point reached across runs.
fn path_for_weight_silence(trie: &Trie, input: &str) -> (Vec<Option<u64>>, u32) {
    let mut deepest_node: u64 = 0;
    let mut deepest_depth: u32 = 0;
    let mut run: Vec<u8> = Vec::new();

    let mut consider = |t: &Trie, run: &mut Vec<u8>, dn: &mut u64, dd: &mut u32| {
        if !run.is_empty() {
            let qr = t.query(run);
            if qr.match_depth >= *dd {
                *dd = qr.match_depth;
                *dn = qr.deepest_node;
            }
            run.clear();
        }
    };

    for ch in input.chars() {
        if is_silence_char(ch) {
            consider(trie, &mut run, &mut deepest_node, &mut deepest_depth);
        } else {
            run.push(char_weight(ch));
        }
    }
    consider(trie, &mut run, &mut deepest_node, &mut deepest_depth);

    let pk = trie.path_key(deepest_node).unwrap_or_default();
    (pk, deepest_depth)
}

#[test]
fn character_weight_experiment() {
    let corpus = load_corpus();
    assert!(!corpus.is_empty());

    let mut trie_a = Trie::new(); // byte
    let mut trie_b = Trie::new(); // weight inline
    let mut trie_c = Trie::new(); // weight + silence

    let mut bytes_total = 0usize;
    let mut silence_total = 0u64;
    for _ in 0..2 {
        for (_topic, content) in &corpus {
            let r_a = trie_a.write(content.as_bytes());
            bytes_total += r_a.tokens_processed;
            let encoded = weight_encode(content);
            let _ = trie_b.write(&encoded);
            let (_w, s) = feed_weight_with_silence(&mut trie_c, content);
            silence_total += s;
        }
    }

    println!("\n===== CHARACTER-WEIGHT EXPERIMENT =====\n");
    println!(
        "Corpus: {} topics, {} bytes per pass, 2 passes.",
        corpus.len(),
        corpus.iter().map(|(_, c)| c.len()).sum::<usize>()
    );
    println!("Total silence ticks in encoding C: {}", silence_total);
    println!("Total bytes routed in A: {}", bytes_total);

    println!("\n-- trie shape --");
    shape_line("A byte-trie", &trie_a);
    shape_line("B weight-inline", &trie_b);
    shape_line("C weight+silence", &trie_c);

    // Topic-discrimination check: do queries on distinct topics produce
    // distinct path-keys?
    let queries: Vec<(&str, &str)> = vec![
        ("spaced-rep CZ", "opakování spaced repetition aktivní vybavování křivka"),
        ("agent-msg CZ", "komunikace mezi agenty v časově kódovaných zprávách"),
        ("rust-async", "async fn fetch url str Result String Error await"),
        ("cooking CZ", "pečené kuře s bramborami rozmarýnem česnek pepř"),
        ("music-theory", "circle of fifths dominant seventh tonic subdominant"),
        ("sql-queries", "SELECT user_id COUNT SUM FROM orders GROUP BY HAVING"),
        ("japanese", "日本語 自己紹介 名前 東京 よろしく"),
        ("linear-alg", "vector n-dimensional dot product matrix determinant"),
        ("unknown", "quantum chromodynamics gauge confinement"),
    ];

    println!("\n-- per-query path keys --");
    println!("{:<18} | {:<22} | {:<22} | {:<22}",
        "topic", "A byte", "B weight-inline", "C weight+silence");
    for (label, q) in &queries {
        let pa = path_for(&trie_a, q);
        let pb = path_for_weight_inline(&trie_b, q);
        let (pc, pc_depth) = path_for_weight_silence(&trie_c, q);
        let short = |v: &Vec<Option<u64>>| -> String {
            let segs: Vec<String> = v
                .iter()
                .map(|s| match s {
                    None => "-".to_string(),
                    Some(h) => format!("{:x}", h & 0xFFFF),
                })
                .collect();
            format!("d{} [{}]", v.len(), segs.join(","))
        };
        println!(
            "{:<18} | {:<22} | {:<22} | {:<22} (best_depth={})",
            label,
            short(&pa),
            short(&pb),
            short(&pc),
            pc_depth
        );
    }

    // Quantify: how many distinct path-key-hashes per encoding?
    fn distinct_hash<F>(queries: &[(&str, &str)], mut f: F) -> usize
    where
        F: FnMut(&str) -> Vec<Option<u64>>,
    {
        use std::collections::HashSet;
        let mut s: HashSet<u64> = HashSet::new();
        for (_, q) in queries {
            let p = f(q);
            // Hash the path-key vector.
            let mut h: u64 = 14695981039346656037;
            for seg in &p {
                h ^= seg.unwrap_or(0);
                h = h.wrapping_mul(1099511628211);
            }
            s.insert(h);
        }
        s.len()
    }
    let a_distinct = distinct_hash(&queries, |q| path_for(&trie_a, q));
    let b_distinct = distinct_hash(&queries, |q| path_for_weight_inline(&trie_b, q));
    let c_distinct = distinct_hash(&queries, |q| {
        let (p, _) = path_for_weight_silence(&trie_c, q);
        p
    });

    println!("\n-- distinct path-keys across {} topical queries --", queries.len());
    println!("  A byte           : {} / {}", a_distinct, queries.len());
    println!("  B weight-inline  : {} / {}", b_distinct, queries.len());
    println!("  C weight+silence : {} / {}", c_distinct, queries.len());

    // Diagnostic — always pass. Interpretation lives in a session note.
    assert!(trie_a.stats(None).is_some());
}
