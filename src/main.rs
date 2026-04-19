use std::io;
use std::sync::Arc;

use tokio::sync::Mutex;

use trie_memory::mcp::context::ContextWindow;
use trie_memory::mcp::{dispatch, sse, transport};
use trie_memory::store::concept::ConceptStore;
use trie_memory::store::layer::LayerStore;
use trie_memory::store::ContentStore;
use trie_memory::trie::Trie;

const DEFAULT_SNAPSHOT_PATH: &str = "W:/data/trie-store/trie-memory.dat";
const DEFAULT_WORD_SNAPSHOT_PATH: &str = "W:/data/trie-store/word-trie-memory.dat";
const DEFAULT_CONTENT_PATH: &str = "W:/data/trie-store/content-store.json";
const DEFAULT_CONCEPT_PATH: &str = "W:/data/trie-store/concept-store.json";
const DEFAULT_LAYER_PATH: &str = "W:/data/trie-store/layer-store.json";
const DEFAULT_SSE_PORT: u16 = 3001;

fn load_trie_at(path: &str, label: &str) -> Trie {
    if Trie::snapshot_exists(path) {
        eprintln!("Restoring {} from {}", label, path);
        match Trie::restore_from(path) {
            Ok(t) => {
                eprintln!("Restored {} nodes ({})", t.nodes.len(), label);
                return t;
            }
            Err(e) => {
                eprintln!("Failed to restore {}: {}, starting fresh", label, e);
            }
        }
    } else {
        eprintln!("No {} snapshot found, starting fresh", label);
    }
    Trie::new()
}

fn load_trie() -> Trie {
    load_trie_at(DEFAULT_SNAPSHOT_PATH, "byte-trie")
}

fn load_word_trie() -> Trie {
    load_trie_at(DEFAULT_WORD_SNAPSHOT_PATH, "word-trie")
}

fn load_store() -> ContentStore {
    if ContentStore::exists(DEFAULT_CONTENT_PATH) {
        eprintln!("Restoring content store from {}", DEFAULT_CONTENT_PATH);
        match ContentStore::load(DEFAULT_CONTENT_PATH) {
            Ok(s) => {
                eprintln!("Restored {} memories", s.entry_count());
                return s;
            }
            Err(e) => {
                eprintln!("Failed to restore content store: {}, starting fresh", e);
            }
        }
    } else {
        eprintln!("No content store found, starting fresh");
    }
    ContentStore::new()
}

fn save_trie_at(trie: &Trie, path: &str, label: &str) {
    match trie.snapshot(path) {
        Ok(r) => eprintln!("Saved {}: {} bytes to {}", label, r.bytes_written, r.path),
        Err(e) => eprintln!("Failed to save {}: {}", label, e),
    }
}

fn save_trie(trie: &Trie) {
    save_trie_at(trie, DEFAULT_SNAPSHOT_PATH, "byte-trie")
}

fn save_word_trie(trie: &Trie) {
    save_trie_at(trie, DEFAULT_WORD_SNAPSHOT_PATH, "word-trie")
}

fn save_store(store: &ContentStore) {
    match store.save(DEFAULT_CONTENT_PATH) {
        Ok(n) => eprintln!("Saved content store: {} bytes to {}", n, DEFAULT_CONTENT_PATH),
        Err(e) => eprintln!("Failed to save content store: {}", e),
    }
}

fn load_concepts() -> ConceptStore {
    if ConceptStore::exists(DEFAULT_CONCEPT_PATH) {
        eprintln!("Restoring concept store from {}", DEFAULT_CONCEPT_PATH);
        match ConceptStore::restore(DEFAULT_CONCEPT_PATH) {
            Ok(c) => {
                eprintln!("Restored {} concepts", c.concept_count());
                return c;
            }
            Err(e) => {
                eprintln!("Failed to restore concept store: {}, starting fresh", e);
            }
        }
    } else {
        eprintln!("No concept store found, starting fresh");
    }
    ConceptStore::new()
}

fn save_concepts(concepts: &ConceptStore) {
    match concepts.snapshot(DEFAULT_CONCEPT_PATH) {
        Ok(n) => eprintln!("Saved concept store: {} bytes to {}", n, DEFAULT_CONCEPT_PATH),
        Err(e) => eprintln!("Failed to save concept store: {}", e),
    }
}

fn load_layers() -> LayerStore {
    if LayerStore::exists(DEFAULT_LAYER_PATH) {
        eprintln!("Restoring layer store from {}", DEFAULT_LAYER_PATH);
        match LayerStore::load(DEFAULT_LAYER_PATH) {
            Ok(l) => {
                eprintln!("Restored {} layers", l.count());
                return l;
            }
            Err(e) => {
                eprintln!("Failed to restore layer store: {}, starting fresh", e);
            }
        }
    } else {
        eprintln!("No layer store found, starting fresh");
    }
    LayerStore::new()
}

fn save_layers(layers: &LayerStore) {
    match layers.save(DEFAULT_LAYER_PATH) {
        Ok(n) => eprintln!("Saved layer store: {} bytes to {}", n, DEFAULT_LAYER_PATH),
        Err(e) => eprintln!("Failed to save layer store: {}", e),
    }
}

fn save_all(
    trie: &Trie,
    word_trie: &Trie,
    store: &ContentStore,
    concepts: &ConceptStore,
    layers: &LayerStore,
) {
    eprintln!("Shutting down, saving...");
    save_trie(trie);
    save_word_trie(word_trie);
    save_store(store);
    save_concepts(concepts);
    save_layers(layers);
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let use_sse = args.iter().any(|a| a == "--sse");

    if use_sse {
        let port = parse_port(&args).unwrap_or(DEFAULT_SSE_PORT);
        let trie = Arc::new(Mutex::new(load_trie()));
        let word_trie = Arc::new(Mutex::new(load_word_trie()));
        let store = Arc::new(Mutex::new(load_store()));
        let concepts = Arc::new(Mutex::new(load_concepts()));
        let layers = Arc::new(Mutex::new(load_layers()));

        let trie_shutdown = Arc::clone(&trie);
        let word_trie_shutdown = Arc::clone(&word_trie);
        let store_shutdown = Arc::clone(&store);
        let concepts_shutdown = Arc::clone(&concepts);
        let layers_shutdown = Arc::clone(&layers);
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            let trie = trie_shutdown.lock().await;
            let word_trie = word_trie_shutdown.lock().await;
            let store = store_shutdown.lock().await;
            let concepts = concepts_shutdown.lock().await;
            let layers = layers_shutdown.lock().await;
            save_all(&trie, &word_trie, &store, &concepts, &layers);
            std::process::exit(0);
        });

        let context = Arc::new(Mutex::new(ContextWindow::default()));
        sse::run(trie, word_trie, store, concepts, layers, context, port).await;
    } else {
        run_stdio().await;
    }
}

async fn run_stdio() {
    let mut trie = load_trie();
    let mut word_trie = load_word_trie();
    let mut store = load_store();
    let mut concepts = load_concepts();
    let mut layers = load_layers();
    let mut context = ContextWindow::default();

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();

    eprintln!("trie-memory MCP server ready (stdio)");

    loop {
        match transport::read_message(&mut reader) {
            Ok(Some(msg)) => {
                if let Some(response) = dispatch::handle_message(
                    &mut trie,
                    &mut word_trie,
                    &mut store,
                    &mut concepts,
                    &mut layers,
                    &mut context,
                    &msg,
                ) {
                    if let Err(e) = transport::write_message(&mut writer, &response) {
                        eprintln!("Failed to write response: {}", e);
                        break;
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }

    save_all(&trie, &word_trie, &store, &concepts, &layers);
}

fn parse_port(args: &[String]) -> Option<u16> {
    let idx = args.iter().position(|a| a == "--port")?;
    args.get(idx + 1)?.parse().ok()
}
