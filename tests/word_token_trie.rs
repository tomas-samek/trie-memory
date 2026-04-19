use trie_memory::store::concept::ConceptStore;
use trie_memory::trie::tokenizer::{
    silence_ticks, tokenize_with_silence, word_token, WordOrSilence,
};
use trie_memory::trie::Trie;

#[test]
fn test_word_token_determinism() {
    assert_eq!(word_token("catalog"), word_token("catalog"));
    assert_eq!(word_token("catalog"), word_token("CATALOG")); // lowercased internally
    assert_ne!(word_token("catalog"), word_token("query"));
    assert_ne!(word_token("catalog"), word_token("endpoint"));
    assert_ne!(word_token("query"), word_token("endpoint"));
}

fn broad_word_corpus() -> Vec<&'static str> {
    vec![
        "catalog", "query", "search", "result", "audit", "manager", "component",
        "endpoint", "identity", "service", "dependency", "resolver", "index",
        "writer", "storage", "provider", "configuration", "event", "dispatcher",
        "logger", "handler", "factory", "registry", "adapter", "context",
        "client", "server", "request", "response", "header", "parser", "node",
        "element", "attribute", "connection", "transaction", "schema", "migration",
        "cache", "store", "invalidator", "pool", "local", "reader", "system",
        "path", "handle", "socket", "buffer", "stream", "network", "thread",
        "oracle", "jet", "com", "example", "http", "json", "xml", "database",
        "redis", "token", "kebab", "snake", "case", "http", "rest", "api",
        "lucene", "session", "middleware", "auth", "user", "group", "role",
        "permission", "policy", "rule", "engine", "pipeline", "worker", "task",
        "job", "queue", "broker", "topic", "subscription", "publisher", "consumer",
        "metric", "gauge", "counter", "histogram", "timer", "tracer", "span",
        "log", "record", "entry", "payload", "envelope", "message", "signal",
    ]
}

#[test]
fn test_word_trie_builds_depth() {
    let mut trie = Trie::new();
    let words = broad_word_corpus();
    for _ in 0..30 {
        for w in &words {
            let t = word_token(w);
            trie.write(&t);
        }
    }
    let stats = trie.stats(None).unwrap();
    assert!(
        stats.total_nodes > 1,
        "word-trie should build past root; got {} nodes",
        stats.total_nodes
    );
}

#[test]
fn test_word_trie_differentiates_words() {
    let mut trie = Trie::new();
    for _ in 0..30 {
        for w in &broad_word_corpus() {
            trie.write(&word_token(w));
        }
    }
    // With a broad corpus, at least SOME pair of distinct words should end up
    // at distinct deepest nodes — otherwise the word-trie has collapsed to root.
    let corpus = broad_word_corpus();
    let keys: Vec<_> = corpus
        .iter()
        .map(|w| trie.path_key_for_input(&word_token(w)))
        .collect();
    let distinct: std::collections::HashSet<_> = keys.iter().cloned().collect();
    assert!(
        distinct.len() > 1,
        "word-trie routed every word to the same node",
    );
}

#[test]
fn test_silence_ticks_values() {
    assert_eq!(silence_ticks(' '), 1);
    assert_eq!(silence_ticks('_'), 1);
    assert_eq!(silence_ticks(','), 3);
    assert_eq!(silence_ticks(';'), 5);
    assert_eq!(silence_ticks('.'), 10);
    assert_eq!(silence_ticks('!'), 10);
    assert_eq!(silence_ticks('?'), 10);
    assert_eq!(silence_ticks('\n'), 20);
}

#[test]
fn test_silence_encoding_camel_and_period() {
    let tokens = tokenize_with_silence("CatalogQuery. SearchResult");
    // Expected: catalog | silence(1) | query | silence(10) | search | silence(1) | result
    let mut words = Vec::new();
    let mut silences = Vec::new();
    for t in &tokens {
        match t {
            WordOrSilence::Word(w) => words.push(w.clone()),
            WordOrSilence::Silence(g) => silences.push(*g),
        }
    }
    assert_eq!(words, vec!["catalog", "query", "search", "result"]);
    // Must contain a sentence-level gap of at least 10 between "query" and "search".
    assert!(silences.contains(&10), "expected period silence=10; got {:?}", silences);
}

#[test]
fn test_silence_collapses_to_max() {
    // Newline > period > space, so a run like ".\n" yields the newline gap (20).
    let tokens = tokenize_with_silence("one.\ntwo");
    let silences: Vec<u64> = tokens
        .iter()
        .filter_map(|t| match t {
            WordOrSilence::Silence(g) => Some(*g),
            _ => None,
        })
        .collect();
    assert!(silences.iter().any(|&g| g >= 20));
}

#[test]
fn test_concept_multi_word_lookup_ranks_by_overlap() {
    let mut word_trie = Trie::new();
    let mut store = ConceptStore::new();

    // Build some word-trie structure so keys differ between words.
    let warmup = [
        "catalog", "query", "endpoint", "search", "result", "audit", "manager",
        "component", "identity", "service", "dependency", "resolver", "index",
        "writer", "storage", "provider", "configuration", "event", "dispatcher",
        "logger",
    ];
    for _ in 0..30 {
        for w in &warmup {
            word_trie.write(&word_token(w));
        }
    }

    let keys_for = |wt: &Trie, input: &str| -> Vec<(String, Vec<Option<u64>>)> {
        trie_memory::trie::tokenizer::split_words(input)
            .into_iter()
            .map(|w| {
                let k = wt.path_key_for_input(&word_token(&w)).unwrap_or_default();
                (w, k)
            })
            .collect()
    };

    let lucene = store.create(Some("lucene".to_string()), 0);
    let rest = store.create(Some("rest-api".to_string()), 0);

    // Bind "CatalogQuery" → lucene: binds both "catalog" and "query".
    for (_w, k) in keys_for(&word_trie, "CatalogQuery") {
        store.bind_word_only(lucene, k, 1).unwrap();
    }
    // Bind "CatalogEndpoint" → rest-api: binds "catalog" and "endpoint".
    for (_w, k) in keys_for(&word_trie, "CatalogEndpoint") {
        store.bind_word_only(rest, k, 1).unwrap();
    }

    // Lookup "CatalogQuery": should match lucene on BOTH words, rest on ONE word.
    let query_keys = keys_for(&word_trie, "CatalogQuery");
    let ranked = store.lookup_multi_word(&query_keys);
    assert!(!ranked.is_empty(), "expected matches for CatalogQuery");
    assert_eq!(
        ranked[0].0, lucene,
        "lucene should rank first (both words match); got {:?}",
        ranked
    );
    // If rest also matches, its score must be <= lucene's score.
    if ranked.len() > 1 {
        assert!(ranked[0].1 >= ranked[1].1);
    }
}
