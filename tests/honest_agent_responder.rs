// Honest-agent Task 06 (Phase A subset) — mode selector + template renderer.
// Validates the two-mode contract plus the Phase B `coverage` gate: Answer
// only when query-to-memory content-word overlap is above the answer
// threshold; Unknown when path matches but words don't.

use trie_memory::mcp::context::ContextWindow;
use trie_memory::mcp::responder::{
    coverage_ppm, render, select_mode, shared_suffix_depth, ResponseMode,
};
use trie_memory::store::memory::{MemoryEntry, SourceType};

fn entry(path: Vec<Option<u64>>, topic: &str, content: &str, ts: u64) -> MemoryEntry {
    MemoryEntry {
        timestamp: ts,
        tick_range: (0, 100),
        content: content.to_string(),
        topic: Some(topic.to_string()),
        depth_profile: vec![(0, 1)],
        path_content_ids: path,
        source_type: SourceType::UserDirect,
        trust_level: 900,
        ..Default::default()
    }
}

#[test]
fn test_shared_suffix_depth() {
    assert_eq!(
        shared_suffix_depth(&[Some(1), Some(2), Some(9)], &[Some(3), Some(2), Some(9)]),
        2,
        "shared from root through depth 1"
    );
    assert_eq!(
        shared_suffix_depth(&[Some(1), Some(9)], &[Some(2), Some(9)]),
        1,
        "only root in common"
    );
    assert_eq!(
        shared_suffix_depth(&[Some(5)], &[Some(5)]),
        1,
        "both are just root"
    );
    assert_eq!(
        shared_suffix_depth(&[Some(1), Some(2)], &[Some(3), Some(4)]),
        0,
        "no common suffix"
    );
    assert_eq!(shared_suffix_depth(&[], &[]), 0);
}

#[test]
fn test_coverage_ignores_stopwords() {
    // "The" and "is" are stopwords; the real signal is the content word.
    // With stopword filtering, query {widget} overlaps memory {widget}.
    assert_eq!(
        coverage_ppm("the widget is", "A widget exists"),
        1000,
        "stopwords should drop out of both sides"
    );
}

#[test]
fn test_coverage_handles_plural_morphology() {
    // Single-s morphology: "streams" ↔ "stream" must match after stemming.
    assert_eq!(
        coverage_ppm("streams help", "stream flows quickly"),
        500,
        "trailing-s stemming should make streams == stream"
    );
    // Counts ↔ count (verb-s), also covered.
    let cov = coverage_ppm("counts together", "count together");
    assert!(cov >= 500);
}

#[test]
fn test_coverage_ppm_basic() {
    // All query words in memory → 1000.
    assert_eq!(
        coverage_ppm("rust content", "rust content here"),
        1000
    );
    // Zero overlap → 0.
    assert_eq!(
        coverage_ppm("cookie chocolate", "synchronization primitive"),
        0
    );
    // Half overlap.
    let half = coverage_ppm("rust synchronization", "rust primitive");
    assert!(half >= 400 && half <= 600, "half overlap should be ~500, got {}", half);
    // Case-insensitive.
    assert_eq!(coverage_ppm("RUST", "rust content"), 1000);
    // Empty query.
    assert_eq!(coverage_ppm("", "anything"), 0);
}

#[test]
fn test_select_mode_empty_memories_is_unknown() {
    let memories: Vec<&MemoryEntry> = Vec::new();
    let sel = select_mode("q", &[Some(1), Some(9)], &memories);
    assert_eq!(sel.mode, ResponseMode::Unknown);
    assert!(sel.supporting.is_empty());
}

#[test]
fn test_select_mode_root_only_match_is_unknown() {
    // Memory and query share only root — Phase A fall-through.
    let m = entry(vec![Some(10), Some(9)], "cooking", "recipe", 100);
    let mems: Vec<&MemoryEntry> = vec![&m];
    let sel = select_mode("recipe", &[Some(99), Some(9)], &mems);
    assert_eq!(sel.mode, ResponseMode::Unknown);
    assert_eq!(sel.top_shared_suffix, 1);
}

#[test]
fn test_select_mode_deep_match_with_coverage_is_answer() {
    // Suffix 3 AND high coverage — the honest happy path.
    let m = entry(
        vec![Some(10), Some(20), Some(30), Some(9)],
        "rust",
        "rust content matters",
        100,
    );
    let mems: Vec<&MemoryEntry> = vec![&m];
    let query_path = vec![Some(55), Some(20), Some(30), Some(9)];
    let sel = select_mode("rust content matters", &query_path, &mems);
    assert_eq!(sel.mode, ResponseMode::Answer);
    assert_eq!(sel.top_shared_suffix, 3);
    assert_eq!(sel.top_coverage, 1000);
    assert_eq!(sel.supporting.len(), 1);
}

#[test]
fn test_select_mode_deep_path_zero_coverage_is_unknown() {
    // This is the D-false-positive scenario: suffix 3 but query has NO words
    // in common with memory content. Phase B must demote this to Unknown.
    let m = entry(
        vec![Some(10), Some(20), Some(30), Some(9)],
        "semaphore",
        "A semaphore is a synchronization primitive that counts permits.",
        100,
    );
    let mems: Vec<&MemoryEntry> = vec![&m];
    let query_path = vec![Some(55), Some(20), Some(30), Some(9)];
    let sel = select_mode(
        "What temperature for chocolate chip cookies",
        &query_path,
        &mems,
    );
    assert_eq!(
        sel.mode,
        ResponseMode::Unknown,
        "zero-coverage must demote suffix match to Unknown"
    );
    assert!(sel.reasoning.contains("coverage"));
}

#[test]
fn test_select_mode_coverage_wins_over_recency_in_tie() {
    // Both memories tie on shared suffix. Phase A broke ties by timestamp
    // (newer). Phase B must prefer the higher-coverage memory.
    let sema = entry(
        vec![Some(1), Some(5), Some(9)],
        "semaphore",
        "A semaphore is a synchronization primitive that counts permits.",
        100, // older
    );
    let tides = entry(
        vec![Some(2), Some(5), Some(9)],
        "tides",
        "The bay of Fundy has tidal ranges up to fifteen meters at extreme spring tides.",
        50, // older still, but content matches the query
    );
    let mems: Vec<&MemoryEntry> = vec![&sema, &tides];
    let sel = select_mode(
        "The bay of Fundy has tidal ranges up to fifteen meters at extreme spring tides.",
        &[Some(7), Some(5), Some(9)],
        &mems,
    );
    assert_eq!(sel.mode, ResponseMode::Answer);
    assert_eq!(
        sel.supporting[0].topic.as_deref(),
        Some("tides"),
        "coverage must win the tie over the older insertion ordering"
    );
}

#[test]
fn test_select_mode_partial_mode_on_moderate_coverage() {
    // 3 query words match out of 10 → 300‰, between partial and answer.
    let m = entry(
        vec![Some(10), Some(20), Some(30), Some(9)],
        "t",
        "rust async runtime schedules futures on thread-pool workers",
        100,
    );
    let mems: Vec<&MemoryEntry> = vec![&m];
    let query_path = vec![Some(55), Some(20), Some(30), Some(9)];
    // 3 of these 10 query words appear in the memory: rust, async, futures.
    let sel = select_mode(
        "rust async futures in ecosystem now tell me everything please",
        &query_path,
        &mems,
    );
    assert_eq!(sel.mode, ResponseMode::Partial);
}

#[test]
fn test_select_mode_drops_shallower_siblings() {
    let deep = entry(
        vec![Some(1), Some(2), Some(3), Some(9)],
        "deep",
        "deep content goes here",
        100,
    );
    let shallow = entry(vec![Some(4), Some(9)], "shallow", "deep content", 100);
    let mems: Vec<&MemoryEntry> = vec![&deep, &shallow];
    let sel = select_mode(
        "deep content goes here",
        &[Some(77), Some(2), Some(3), Some(9)],
        &mems,
    );
    assert_eq!(sel.mode, ResponseMode::Answer);
    assert_eq!(sel.supporting.len(), 1);
    assert_eq!(sel.supporting[0].topic.as_deref(), Some("deep"));
}

#[test]
fn test_render_unknown_never_fabricates() {
    let ctx = ContextWindow::default();
    let empty: Vec<&MemoryEntry> = Vec::new();
    let sel = select_mode("", &[Some(1), Some(9)], &empty);
    let out = render(&sel, "what is a foo-widget", &ctx);
    let lower = out.to_lowercase();
    assert!(
        lower.contains("don't") || lower.contains("do not") || lower.contains("i don't"),
        "Unknown render must admit ignorance: {:?}",
        out
    );
    assert!(
        out.contains("foo-widget") || out.contains("foo"),
        "Unknown should echo the question: {:?}",
        out
    );
}

#[test]
fn test_render_answer_uses_stored_content_verbatim() {
    let ctx = ContextWindow::default();
    let m = entry(
        vec![Some(1), Some(2), Some(9)],
        "rust",
        "Rust's async runtime schedules futures on thread-pool workers.",
        100,
    );
    let mems: Vec<&MemoryEntry> = vec![&m];
    // Query phrased with high word overlap so coverage is past the Answer gate.
    let sel = select_mode(
        "Rust's async runtime schedules futures on thread-pool workers.",
        &[Some(7), Some(2), Some(9)],
        &mems,
    );
    assert_eq!(sel.mode, ResponseMode::Answer);
    let out = render(&sel, "how does async work", &ctx);
    assert!(
        out.contains("Rust's async runtime"),
        "renderer must reproduce stored content: {:?}",
        out
    );
    assert!(
        out.contains("[rust]"),
        "renderer should surface the topic tag: {:?}",
        out
    );
}

#[test]
fn test_render_answer_surfaces_low_trust_tag() {
    let ctx = ContextWindow::default();
    let mut m = entry(
        vec![Some(1), Some(2), Some(9)],
        "web",
        "The sky is green because of atmospheric scraping (scraped).",
        100,
    );
    m.source_type = SourceType::WebFetched;
    m.trust_level = 400;
    let mems: Vec<&MemoryEntry> = vec![&m];
    let sel = select_mode(
        "The sky is green because of atmospheric scraping",
        &[Some(7), Some(2), Some(9)],
        &mems,
    );
    assert_eq!(sel.mode, ResponseMode::Answer);
    let out = render(&sel, "sky color", &ctx);
    let lower = out.to_lowercase();
    assert!(
        lower.contains("web") || lower.contains("lower trust"),
        "low-trust sources must be surfaced: {:?}",
        out
    );
}

#[test]
fn test_render_answer_flags_legacy_entries() {
    let ctx = ContextWindow::default();
    let mut m = entry(
        vec![Some(1), Some(2), Some(9)],
        "old",
        "ancient text buried in archives",
        100,
    );
    m.legacy_origin = true;
    let mems: Vec<&MemoryEntry> = vec![&m];
    let sel = select_mode(
        "ancient text buried in archives",
        &[Some(7), Some(2), Some(9)],
        &mems,
    );
    let out = render(&sel, "old stuff", &ctx);
    assert!(
        out.to_lowercase().contains("legacy"),
        "legacy-origin entries must be flagged: {:?}",
        out
    );
}

#[test]
fn test_render_partial_flags_uncertainty() {
    let ctx = ContextWindow::default();
    let m = entry(
        vec![Some(1), Some(2), Some(9)],
        "topic",
        "rust async runtime schedules futures on thread-pool workers",
        100,
    );
    let mems: Vec<&MemoryEntry> = vec![&m];
    let sel = select_mode(
        "rust async futures in ecosystem now tell me everything please",
        &[Some(7), Some(2), Some(9)],
        &mems,
    );
    assert_eq!(sel.mode, ResponseMode::Partial);
    let out = render(&sel, "q", &ctx);
    let lower = out.to_lowercase();
    assert!(
        lower.contains("partial") || lower.contains("not a confident"),
        "Partial must signal uncertainty: {:?}",
        out
    );
}
