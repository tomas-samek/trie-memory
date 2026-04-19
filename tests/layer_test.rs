use trie_memory::store::layer::{LayerStore, MemoryLayer};

#[test]
fn test_layer_lifecycle() {
    let mut layers = LayerStore::new();

    let id = layers.begin("test layer".into(), Some("math".into()), 100);
    assert_eq!(id, 0);
    assert!(layers.active().is_some());

    layers.record_write(50, 200);
    layers.record_concept(0);

    let layer = layers.commit().unwrap();
    assert_eq!(layer.tick_start, 100);
    assert_eq!(layer.tick_end, 200);
    assert_eq!(layer.word_count, 50);
    assert_eq!(layer.concept_ids, vec![0]);
    assert!(layers.active().is_none());

    assert_eq!(layers.count(), 1);
    let by_domain = layers.layers_by_domain("math");
    assert_eq!(by_domain.len(), 1);
}

#[test]
fn test_relative_ticks() {
    let layer = MemoryLayer {
        id: 0,
        label: "test".into(),
        domain: None,
        origin_timestamp: 0,
        tick_start: 1000,
        tick_end: 2000,
        word_count: 100,
        concept_ids: vec![],
    };

    assert_eq!(layer.relative_tick(1500), Some(500));
    assert_eq!(layer.relative_tick(999), None);
    assert_eq!(layer.relative_tick(2001), None);
    assert_eq!(layer.tick_duration(), 1000);
}

#[test]
fn test_layer_persistence() {
    let dir = std::env::temp_dir().join("trie-memory-layer-test");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("test-layers.json");
    let path_str = path.to_str().unwrap();

    let mut layers = LayerStore::new();
    layers.begin("test".into(), Some("code:rust".into()), 0);
    layers.record_write(10, 100);
    layers.record_concept(42);
    layers.commit();

    layers.save(path_str).unwrap();
    let loaded = LayerStore::load(path_str).unwrap();
    assert_eq!(loaded.count(), 1);
    let l = loaded.get(0).unwrap();
    assert_eq!(l.label, "test");
    assert_eq!(l.domain.as_deref(), Some("code:rust"));
    assert_eq!(l.word_count, 10);
    assert_eq!(l.concept_ids, vec![42]);

    std::fs::remove_file(path_str).ok();
}

#[test]
fn test_multiple_layers() {
    let mut layers = LayerStore::new();

    layers.begin("java".into(), Some("code:java".into()), 0);
    layers.record_write(100, 500);
    layers.commit();

    layers.begin("novel".into(), Some("prose".into()), 600);
    layers.record_write(200, 1200);
    layers.commit();

    assert_eq!(layers.count(), 2);
    assert_eq!(layers.layers_by_domain("code:java").len(), 1);
    assert_eq!(layers.layers_by_domain("prose").len(), 1);
    assert_eq!(layers.layers_by_domain("math").len(), 0);

    let at_tick = layers.layers_at_tick(300);
    assert_eq!(at_tick.len(), 1);
    assert_eq!(at_tick[0].label, "java");
}

#[test]
fn test_no_active_layer_is_harmless() {
    let mut layers = LayerStore::new();
    // These should be no-ops when no layer is active
    layers.record_write(10, 50);
    layers.record_concept(0);
    assert!(layers.active().is_none());
    assert_eq!(layers.count(), 0);
}
