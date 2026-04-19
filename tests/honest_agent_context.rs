// Honest-agent Task 03 — context window tests.
//
// Covers enrichment determinism, cross-session independence, and ordering
// invariants for the rolling exchange buffer + hot_concepts.

use trie_memory::mcp::context::ContextWindow;

#[test]
fn test_first_query_has_empty_context() {
    let ctx = ContextWindow::default();
    let e = ctx.enrich("what is a semaphore");
    // Enriched equals raw when context is empty.
    assert_eq!(e.raw, "what is a semaphore");
    assert_eq!(e.enriched, "what is a semaphore");
    assert!(e.topics_used.is_empty());
}

#[test]
fn test_enrichment_appends_recent_topics() {
    let mut ctx = ContextWindow::default();
    ctx.record_exchange(
        "tell me about tides",
        vec!["tides".to_string(), "gravity".to_string()],
        100,
    );
    let e = ctx.enrich("what about today");
    assert!(e.enriched.starts_with("what about today"));
    assert!(e.enriched.contains("tides"));
    assert!(e.enriched.contains("gravity"));
    assert!(e.enriched.len() > "what about today".len());
    assert_eq!(e.topics_used, vec!["tides".to_string(), "gravity".to_string()]);
}

#[test]
fn test_same_query_different_context_differs() {
    let mut a = ContextWindow::default();
    let mut b = ContextWindow::default();
    a.record_exchange("prior a", vec!["rust".to_string()], 1);
    b.record_exchange("prior b", vec!["cooking".to_string()], 1);

    let ea = a.enrich("tell me more");
    let eb = b.enrich("tell me more");
    assert_ne!(ea.enriched, eb.enriched);
    assert!(ea.enriched.contains("rust"));
    assert!(eb.enriched.contains("cooking"));
}

#[test]
fn test_hot_concepts_are_mru() {
    let mut ctx = ContextWindow::default();
    ctx.record_exchange("q1", vec!["alpha".to_string()], 1);
    ctx.record_exchange("q2", vec!["beta".to_string()], 2);
    ctx.record_exchange("q3", vec!["alpha".to_string()], 3);
    // alpha was just reactivated; it should be first.
    assert_eq!(ctx.hot_concepts().first().map(|s| s.as_str()), Some("alpha"));
    // beta is still in the buffer.
    assert!(ctx.hot_concepts().iter().any(|s| s == "beta"));
}

#[test]
fn test_rolling_buffer_trims_to_cap() {
    let mut ctx = ContextWindow::default();
    for i in 0..30 {
        ctx.record_exchange(&format!("q{}", i), vec![format!("topic{}", i)], i);
    }
    // MAX_EXCHANGES = 10 inside the module.
    assert!(ctx.conversation_depth() <= 10);
}

#[test]
fn test_reset_clears_state() {
    let mut ctx = ContextWindow::default();
    ctx.record_exchange("q1", vec!["alpha".to_string()], 1);
    ctx.record_exchange("q2", vec!["beta".to_string()], 2);
    assert!(ctx.conversation_depth() > 0);
    assert!(!ctx.hot_concepts().is_empty());

    ctx.reset(Some("new-session".into()));
    assert_eq!(ctx.conversation_depth(), 0);
    assert!(ctx.hot_concepts().is_empty());
    assert_eq!(ctx.session_id, "new-session");

    // Post-reset enrichment is a no-op.
    let e = ctx.enrich("hello");
    assert_eq!(e.enriched, "hello");
}

#[test]
fn test_observer_and_session_propagate() {
    let ctx = ContextWindow::new(Some("jerry".into()), "sess-42".into());
    assert_eq!(ctx.observer_id.as_deref(), Some("jerry"));
    assert_eq!(ctx.session_id, "sess-42");
}

#[test]
fn test_enrichment_dedups_topics() {
    let mut ctx = ContextWindow::default();
    ctx.record_exchange("q1", vec!["rust".to_string(), "rust".to_string()], 1);
    ctx.record_exchange("q2", vec!["rust".to_string()], 2);
    let e = ctx.enrich("more");
    // "rust" should appear only once in the enriched suffix.
    let count = e.enriched.matches("rust").count();
    assert_eq!(count, 1, "enriched={:?}", e.enriched);
}
