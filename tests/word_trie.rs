use trie_memory::store::concept::ConceptStore;
use trie_memory::trie::tokenizer::{split_words, tokenize_to_bytes};
use trie_memory::trie::Trie;

#[test]
fn test_tokenizer_camel_case() {
    assert_eq!(split_words("CatalogQuery"), vec!["catalog", "query"]);
    assert_eq!(
        split_words("IDCSIdentityService"),
        vec!["idcs", "identity", "service"]
    );
    assert_eq!(
        split_words("getComponentVersionItem"),
        vec!["get", "component", "version", "item"]
    );
    assert_eq!(split_words("HTMLParser"), vec!["html", "parser"]);
}

#[test]
fn test_tokenizer_dotted_paths() {
    assert_eq!(
        split_words("oracle.jet.catalog.audit"),
        vec!["oracle", "jet", "catalog", "audit"]
    );
    assert_eq!(
        split_words("snake_case_name"),
        vec!["snake", "case", "name"]
    );
    assert_eq!(split_words("kebab-case-name"), vec!["kebab", "case", "name"]);
    assert_eq!(
        split_words("path/to/file.ext"),
        vec!["path", "to", "file", "ext"]
    );
}

#[test]
fn test_tokenizer_empty_and_mixed() {
    assert!(split_words("").is_empty());
    assert_eq!(
        split_words("  Foo..Bar__baz "),
        vec!["foo", "bar", "baz"]
    );
}

fn diverse_word_corpus() -> Vec<&'static str> {
    vec![
        "CatalogQuery", "CatalogEndpoint", "CatalogService", "CatalogHandler", "CatalogManager",
        "SearchResult", "SearchService", "SearchIndex", "SearchEngine", "SearchQuery",
        "AuditManager", "AuditLog", "AuditTrail", "AuditReport", "AuditEvent",
        "ComponentResolver", "ComponentRegistry", "ComponentFactory", "ComponentBuilder",
        "IdentityProvider", "IdentityStore", "IdentityService", "IdentityContext",
        "DependencyTree", "DependencyGraph", "DependencyResolver", "DependencyInjector",
        "HttpClient", "HttpServer", "HttpRequest", "HttpResponse", "HttpHeader",
        "JsonParser", "JsonWriter", "JsonSerializer", "JsonDeserializer",
        "XmlParser", "XmlWriter", "XmlNode", "XmlElement", "XmlAttribute",
        "DatabaseConnection", "DatabaseTransaction", "DatabaseSchema", "DatabaseMigration",
        "CacheManager", "CacheStore", "CacheInvalidator", "CacheProvider",
        "LoggerFactory", "LoggerConfig", "LoggerAdapter", "LoggerHandler",
        "ThreadPool", "ThreadLocal", "ThreadContext", "ThreadSafe",
        "FileReader", "FileWriter", "FileSystem", "FilePath", "FileHandle",
        "NetworkSocket", "NetworkBuffer", "NetworkStream", "NetworkAdapter",
        "oracle.jet.catalog", "oracle.jet.search", "oracle.jet.audit", "oracle.jet.component",
    ]
}

#[test]
fn test_word_trie_differentiates_within_language() {
    let mut trie = Trie::new();

    for _ in 0..10 {
        for input in diverse_word_corpus() {
            trie.write(&tokenize_to_bytes(input));
        }
    }

    let q1 = trie.query(&tokenize_to_bytes("CatalogQuery"));
    let q2 = trie.query(&tokenize_to_bytes("CatalogEndpoint"));
    let q3 = trie.query(&tokenize_to_bytes("SearchResult"));

    // With a sufficiently diverse corpus, the word-trie must grow beyond root,
    // and at least two distinct inputs must route to different nodes.
    assert!(
        trie.nodes.len() > 1,
        "word-trie never grew past root; corpus was too uniform",
    );
    assert!(
        q1.deepest_node != q2.deepest_node
            || q1.deepest_node != q3.deepest_node
            || q2.deepest_node != q3.deepest_node,
        "word-trie routed all three distinct inputs to the same node",
    );
}

#[test]
fn test_concept_lookup_uses_word_trie() {
    let mut byte_trie = Trie::new();
    let mut word_trie = Trie::new();
    let mut store = ConceptStore::new();

    for _ in 0..10 {
        for input in diverse_word_corpus() {
            byte_trie.write(input.as_bytes());
            word_trie.write(&tokenize_to_bytes(input));
        }
    }

    let make_keys = |input: &str| {
        let bk = byte_trie.path_key_for_input(input.as_bytes()).unwrap_or_default();
        let wk = word_trie
            .path_key_for_input(&tokenize_to_bytes(input))
            .unwrap_or_default();
        (bk, wk)
    };

    let lucene = store.create(Some("lucene-search".to_string()), 0);
    let rest = store.create(Some("rest-api".to_string()), 0);

    let (bk, wk) = make_keys("CatalogQuery");
    store.bind_dual(lucene, bk, wk, 1).unwrap();

    let (bk, wk) = make_keys("CatalogEndpoint");
    store.bind_dual(rest, bk, wk, 1).unwrap();

    // Word-trie key for CatalogQuery should pick lucene only
    let (_, wk) = make_keys("CatalogQuery");
    let hits: Vec<u64> = store.lookup_word(&wk).iter().map(|c| c.id).collect();
    assert!(hits.contains(&lucene));
    assert!(!hits.contains(&rest));

    let (_, wk) = make_keys("CatalogEndpoint");
    let hits: Vec<u64> = store.lookup_word(&wk).iter().map(|c| c.id).collect();
    assert!(hits.contains(&rest));
    assert!(!hits.contains(&lucene));
}
