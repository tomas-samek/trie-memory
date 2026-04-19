// Phase B step 2 — confidence vector components + expanded cascade.
//
// Covers ConfidenceVector computation (strength, clarity, density, trust,
// recency, legacy, coverage) and the new Disambiguate mode.

use trie_memory::mcp::responder::{
    compute_confidence, select_mode_with_tick, ResponseMode,
};
use trie_memory::store::memory::{MemoryEntry, SourceType};

fn entry(
    path: Vec<Option<u64>>,
    topic: &str,
    content: &str,
    ts: u64,
    trust: u32,
    src: SourceType,
) -> MemoryEntry {
    MemoryEntry {
        timestamp: ts,
        tick_range: (0, ts),
        content: content.to_string(),
        topic: Some(topic.to_string()),
        depth_profile: vec![(0, 1)],
        path_content_ids: path,
        source_type: src,
        trust_level: trust,
        ..Default::default()
    }
}

#[test]
fn test_signal_strength_scales_with_suffix_depth() {
    let shallow = entry(vec![Some(1), Some(9)], "s", "alpha beta", 0, 900, SourceType::UserDirect);
    let deep = entry(vec![Some(1), Some(2), Some(3), Some(9)], "d", "alpha beta", 0, 900, SourceType::UserDirect);
    let query_path = vec![Some(7), Some(2), Some(3), Some(9)];

    // Shallow: only 1 of 4 path segments shared → 250‰.
    let (c_shallow, _) =
        compute_confidence("alpha beta", &query_path, &[&shallow], 5, 0);
    // Deep: 3 of 4 path segments shared → 750‰.
    let (c_deep, _) = compute_confidence("alpha beta", &query_path, &[&deep], 5, 0);
    assert!(
        c_deep.signal_strength > c_shallow.signal_strength,
        "deeper suffix must give higher strength: deep={} shallow={}",
        c_deep.signal_strength,
        c_shallow.signal_strength
    );
    assert!(c_deep.signal_strength >= 500);
}

#[test]
fn test_signal_clarity_low_on_tied_candidates() {
    // Two memories same suffix + same coverage → ratio ~1 → clarity ~0.
    let a = entry(vec![Some(1), Some(2), Some(9)], "a", "rust async", 0, 900, SourceType::UserDirect);
    let b = entry(vec![Some(3), Some(2), Some(9)], "b", "rust async", 0, 900, SourceType::UserDirect);
    let mems: Vec<&MemoryEntry> = vec![&a, &b];
    let (conf, _) =
        compute_confidence("rust async", &[Some(7), Some(2), Some(9)], &mems, 5, 0);
    assert!(
        conf.signal_clarity < 250,
        "tied memories must produce low clarity, got {}",
        conf.signal_clarity
    );
}

#[test]
fn test_signal_clarity_high_when_top_dominates() {
    let winner = entry(vec![Some(1), Some(2), Some(9)], "w", "rust async runtime", 0, 900, SourceType::UserDirect);
    let loser = entry(vec![Some(3), Some(2), Some(9)], "l", "totally unrelated words", 0, 900, SourceType::UserDirect);
    let mems: Vec<&MemoryEntry> = vec![&winner, &loser];
    let (conf, _) =
        compute_confidence("rust async runtime", &[Some(7), Some(2), Some(9)], &mems, 5, 0);
    assert!(
        conf.signal_clarity >= 500,
        "one clear winner must give high clarity, got {}",
        conf.signal_clarity
    );
}

#[test]
fn test_source_trust_averages_across_supporting() {
    let high = entry(vec![Some(1), Some(2), Some(9)], "a", "abc def", 0, 1000, SourceType::UserCorrection);
    let mid = entry(vec![Some(3), Some(2), Some(9)], "b", "abc def", 0, 400, SourceType::WebFetched);
    let mems: Vec<&MemoryEntry> = vec![&high, &mid];
    let (conf, _) =
        compute_confidence("abc def", &[Some(7), Some(2), Some(9)], &mems, 5, 0);
    // Mean of 1000 and 400 = 700.
    assert_eq!(conf.source_trust, 700);
}

#[test]
fn test_domain_density_counts_tied_cluster() {
    let a = entry(vec![Some(1), Some(2), Some(9)], "a", "x y", 0, 900, SourceType::UserDirect);
    let b = entry(vec![Some(3), Some(2), Some(9)], "b", "x y", 0, 900, SourceType::UserDirect);
    let c = entry(vec![Some(5), Some(9)], "c", "x y", 0, 900, SourceType::UserDirect); // shallower
    let mems: Vec<&MemoryEntry> = vec![&a, &b, &c];
    let (conf, _) =
        compute_confidence("x y", &[Some(7), Some(2), Some(9)], &mems, 5, 0);
    // 2 memories tied at top suffix, max_results=5 → 2/5 = 400‰.
    assert_eq!(conf.domain_density, 400);
}

#[test]
fn test_recency_decays_with_age() {
    let m = entry(vec![Some(1), Some(2), Some(9)], "t", "alpha beta", 100, 900, SourceType::UserDirect);
    let query_path = vec![Some(7), Some(2), Some(9)];
    // current_tick=100 (same as entry) → max recency.
    let (c_now, _) = compute_confidence("alpha beta", &query_path, &[&m], 5, 100);
    // current_tick way later → decayed.
    let (c_later, _) = compute_confidence("alpha beta", &query_path, &[&m], 5, 500_000);
    assert_eq!(c_now.recency, 1000);
    assert!(c_later.recency < c_now.recency);
}

#[test]
fn test_legacy_flag_propagates() {
    let mut m = entry(vec![Some(1), Some(2), Some(9)], "t", "alpha beta", 0, 200, SourceType::Unknown);
    m.legacy_origin = true;
    let mems: Vec<&MemoryEntry> = vec![&m];
    let (conf, _) =
        compute_confidence("alpha beta", &[Some(7), Some(2), Some(9)], &mems, 5, 0);
    assert!(conf.legacy_origin);
}

// ----- cascade behavior -----

#[test]
fn test_cascade_disambiguate_on_tied_candidates() {
    // Two memories at suffix 3 with high coverage → clarity ≈ 0 → DISAMBIGUATE.
    let a = entry(vec![Some(1), Some(2), Some(3), Some(9)], "topic-a", "alpha beta gamma", 0, 900, SourceType::UserDirect);
    let b = entry(vec![Some(5), Some(2), Some(3), Some(9)], "topic-b", "alpha beta gamma", 0, 900, SourceType::UserDirect);
    let mems: Vec<&MemoryEntry> = vec![&a, &b];
    let sel = select_mode_with_tick(
        "alpha beta gamma",
        &[Some(9), Some(2), Some(3), Some(9)],
        &mems,
        5,
        0,
    );
    assert_eq!(sel.mode, ResponseMode::Disambiguate);
    assert!(sel.supporting.len() >= 2);
    assert!(sel.reasoning.contains("disambiguate") || sel.reasoning.contains("clarity"));
}

#[test]
fn test_cascade_legacy_low_trust_downgraded_to_partial() {
    let mut m = entry(vec![Some(1), Some(2), Some(3), Some(9)], "t", "alpha beta gamma", 0, 200, SourceType::Unknown);
    m.legacy_origin = true;
    let mems: Vec<&MemoryEntry> = vec![&m];
    let sel = select_mode_with_tick(
        "alpha beta gamma",
        &[Some(9), Some(2), Some(3), Some(9)],
        &mems,
        5,
        0,
    );
    // Without the legacy downgrade this would be Answer (full coverage).
    assert_eq!(sel.mode, ResponseMode::Partial);
}

#[test]
fn test_cascade_fresh_high_trust_still_answer() {
    let m = entry(vec![Some(1), Some(2), Some(3), Some(9)], "t", "alpha beta gamma", 100, 900, SourceType::UserDirect);
    let mems: Vec<&MemoryEntry> = vec![&m];
    let sel = select_mode_with_tick(
        "alpha beta gamma",
        &[Some(9), Some(2), Some(3), Some(9)],
        &mems,
        5,
        100,
    );
    assert_eq!(sel.mode, ResponseMode::Answer);
    assert!(sel.confidence.signal_strength >= 500);
    assert_eq!(sel.confidence.coverage, 1000);
}
