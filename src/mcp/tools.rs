use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::context::ContextWindow;
use super::responder;
use crate::store::concept::ConceptStore;
use crate::store::layer::LayerStore;
use crate::store::memory::{source_type_default_trust, MemoryEntry, Origin, SourceType};
use crate::store::ContentStore;
use crate::trie::tokenizer;
use crate::trie::Trie;

/// Return the list of available tools in MCP format.
pub fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "trie_write",
                "description": "Feed a UTF-8 string into BOTH the byte-level and word-level tries.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "data": { "type": "string", "description": "UTF-8 string to feed into the tries" }
                    },
                    "required": ["data"]
                }
            },
            {
                "name": "trie_read",
                "description": "Walk from a leaf node to root, returning the path.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "leaf_id": { "type": "number", "description": "Node ID of the leaf to start from" }
                    },
                    "required": ["leaf_id"]
                }
            },
            {
                "name": "trie_query",
                "description": "Query BOTH tries (byte + word) with a pattern, read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "UTF-8 string pattern to match" }
                    },
                    "required": ["pattern"]
                }
            },
            {
                "name": "trie_stats",
                "description": "Get stats for a specific node or the root. Select trie with 'byte' (default) or 'word'.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "number", "description": "Node ID (defaults to root = 0)" },
                        "trie": { "type": "string", "description": "'byte' (default) or 'word'" }
                    }
                }
            },
            {
                "name": "trie_tokenize",
                "description": "Diagnostic: show how an input string is split into words and hashed for the word-level trie.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "Text to tokenize" }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "trie_perceive",
                "description": "Batch multi-leaf read. Walk multiple leaves to root, aggregate activation hits per node grouped by depth.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "leaf_ids": {
                            "type": "array",
                            "items": { "type": "number" },
                            "description": "Array of leaf node IDs to read from"
                        }
                    },
                    "required": ["leaf_ids"]
                }
            },
            {
                "name": "trie_perceive_window",
                "description": "Perceive recent trie activity. Returns activation map of nodes that fired within the last N ticks.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "last_n_ticks": { "type": "number", "description": "How many ticks back to look (default: 1000)" }
                    }
                }
            },
            {
                "name": "trie_remember",
                "description": "Store content with trie recognition + provenance. Writes to trie and saves content keyed by content-addressable path for later recall. Optional provenance fields (observer_id, source_type, trust_level, origin) carry who/where/how for honest-agent recall.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "content": { "type": "string", "description": "Text content to remember" },
                        "topic": { "type": "string", "description": "Optional topic label" },
                        "observer_id": { "type": "string", "description": "Who perceived this (user/agent identifier)" },
                        "session_id": { "type": "string", "description": "Which session this belongs to" },
                        "stream_id": { "type": "string", "description": "Which input stream (text, sensor, etc.)" },
                        "source_type": { "type": "string", "description": "One of: user_direct, user_correction, agent_inference, sensor_direct, web_fetched, tool_result, memory_recall, unknown" },
                        "trust_level": { "type": "number", "description": "Integer 0-1000 (defaults by source_type)" },
                        "origin": {
                            "type": "object",
                            "description": "Provenance block",
                            "properties": {
                                "author": { "type": "string" },
                                "captured_at": { "type": "number" },
                                "via": { "type": "string" },
                                "chain": { "type": "array", "items": { "type": "number" } },
                                "corrects": { "type": "number", "description": "Deposit id this entry corrects. The target will be stamped with revised_by." }
                            }
                        },
                        "corrects": { "type": "number", "description": "Shortcut for origin.corrects. Paired with source_type=\"user_correction\"." },
                        "modality": { "type": "string", "description": "e.g. text, audio, vision" },
                        "language": { "type": "string" }
                    },
                    "required": ["content"]
                }
            },
            {
                "name": "trie_recall",
                "description": "Recall stored memories. Query is enriched with recent-topic context before routing through the trie, then indexed against content-addressable path keys. Set use_context=false to bypass enrichment.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Text to use as recognition query" },
                        "max_results": { "type": "number", "description": "Max memories to return (default 5)" },
                        "use_context": { "type": "boolean", "description": "Enrich query with context window (default true)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "trie_suggest_groups",
                "description": "Analyze a node's children and suggest which ones could be grouped based on spectrum overlap.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "number", "description": "Node to analyze (defaults to root = 0)" },
                        "min_overlap": { "type": "number", "description": "Minimum overlap ratio 0.0-1.0 (default 0.3)" }
                    }
                }
            },
            {
                "name": "trie_group",
                "description": "Insert an intermediate node between a parent and specified children, grouping them by shared spectrum.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "parent_id": { "type": "number", "description": "Parent node ID" },
                        "child_ids": { "type": "array", "items": { "type": "number" }, "description": "IDs of children to group (min 2)" }
                    },
                    "required": ["parent_id", "child_ids"]
                }
            },
            {
                "name": "trie_snapshot",
                "description": "Save both tries (byte + word), content store, and layer store to disk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Byte-trie file path (default: ./trie-memory.dat)" },
                        "word_path": { "type": "string", "description": "Word-trie file path (default: ./word-trie-memory.dat)" },
                        "content_path": { "type": "string", "description": "Content-store file path (default: ./content-store.json)" },
                        "layer_path": { "type": "string", "description": "Layer-store file path (default: ./layer-store.json)" }
                    }
                }
            },
            {
                "name": "trie_restore",
                "description": "Restore both tries (byte + word), content store, and layer store from disk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Byte-trie file path (default: ./trie-memory.dat)" },
                        "word_path": { "type": "string", "description": "Word-trie file path (default: ./word-trie-memory.dat)" },
                        "content_path": { "type": "string", "description": "Content-store file path (default: ./content-store.json)" },
                        "layer_path": { "type": "string", "description": "Layer-store file path (default: ./layer-store.json)" }
                    }
                }
            },
            {
                "name": "trie_path_key",
                "description": "Generate a content-addressable path key for a node. Select trie with 'byte' (default) or 'word'.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "number", "description": "Node ID to generate path key for" },
                        "trie": { "type": "string", "description": "'byte' (default) or 'word'" }
                    },
                    "required": ["node_id"]
                }
            },
            {
                "name": "concept_create",
                "description": "Create a new concept in the concept store, optionally with a human-readable label.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "label": { "type": "string", "description": "Optional human-readable label for the concept" }
                    }
                }
            },
            {
                "name": "concept_bind",
                "description": "Bind an input string to an existing concept by routing it through the trie to get a path key.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "concept_id": { "type": "number", "description": "Concept ID to bind to" },
                        "input": { "type": "string", "description": "Input string to route through the trie" }
                    },
                    "required": ["concept_id", "input"]
                }
            },
            {
                "name": "concept_lookup",
                "description": "Look up concepts matching an input string by routing it through the trie to get a path key, then finding bound concepts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "input": { "type": "string", "description": "Input string to route through the trie" }
                    },
                    "required": ["input"]
                }
            },
            {
                "name": "concept_bind_auto",
                "description": "Temporal co-occurrence binder: routes all inputs through the trie, creates one concept, and binds all path keys to it. Use to teach that multiple surface forms mean the same thing.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "inputs": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of surface forms to bind together (e.g. [\"bush\", \"křoví\", \"茂み\"])"
                        },
                        "label": { "type": "string", "description": "Optional label for the created concept" }
                    },
                    "required": ["inputs"]
                }
            },
            {
                "name": "concept_snapshot",
                "description": "Save the concept store to disk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path (default: ./concept-store.json)" }
                    }
                }
            },
            {
                "name": "concept_restore",
                "description": "Restore the concept store from disk.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path (default: ./concept-store.json)" }
                    }
                }
            },
            {
                "name": "layer_begin",
                "description": "Start a new memory layer. All writes between begin and commit belong to this layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "label": { "type": "string", "description": "Human-readable label (e.g. 'java codebase', 'novel ch1-5')" },
                        "domain": { "type": "string", "description": "Optional domain prefix (e.g. 'code:java', 'prose', 'math')" }
                    },
                    "required": ["label"]
                }
            },
            {
                "name": "layer_commit",
                "description": "Freeze the current active layer and add it to history.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            },
            {
                "name": "layer_list",
                "description": "Show all committed memory layers, optionally filtered by domain.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "domain": { "type": "string", "description": "Optional domain filter" }
                    }
                }
            },
            {
                "name": "layer_info",
                "description": "Get details about a specific memory layer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "layer_id": { "type": "number", "description": "Layer ID" }
                    },
                    "required": ["layer_id"]
                }
            },
            {
                "name": "context_show",
                "description": "Inspect the current context window (observer, session, recent exchanges, hot concepts).",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": "context_reset",
                "description": "Clear the context window (start a new session). Optional session_id renames.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string" }
                    }
                }
            },
            {
                "name": "trie_ask",
                "description": "Honest-agent entry point: enriches the query via context, recalls memories by content-addressable path, selects response mode (answer / unknown), and renders a template response that never fabricates.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "What you want to know" },
                        "max_results": { "type": "number", "description": "Max supporting memories (default 5)" }
                    },
                    "required": ["query"]
                }
            }
        ]
    })
}

const DEFAULT_SNAPSHOT_PATH: &str = "W:/data/trie-store/trie-memory.dat";
const DEFAULT_WORD_SNAPSHOT_PATH: &str = "W:/data/trie-store/word-trie-memory.dat";

const DEFAULT_CONCEPT_PATH: &str = "W:/data/trie-store/concept-store.json";
const DEFAULT_CONTENT_PATH: &str = "W:/data/trie-store/content-store.json";
const DEFAULT_LAYER_PATH: &str = "W:/data/trie-store/layer-store.json";

/// Dispatch a tools/call request to the appropriate handler.
pub fn call_tool(
    trie: &mut Trie,
    word_trie: &mut Trie,
    store: &mut ContentStore,
    concepts: &mut ConceptStore,
    layers: &mut LayerStore,
    context: &mut ContextWindow,
    name: &str,
    arguments: &Value,
) -> Value {
    match name {
        "trie_write" => handle_write(trie, word_trie, layers, arguments),
        "trie_read" => handle_read(trie, arguments),
        "trie_query" => handle_query(trie, word_trie, arguments),
        "trie_stats" => handle_stats(trie, word_trie, arguments),
        "trie_tokenize" => handle_tokenize(arguments),
        "trie_perceive" => handle_perceive(trie, arguments),
        "trie_perceive_window" => handle_perceive_window(trie, arguments),
        "trie_remember" => handle_remember(trie, word_trie, store, arguments),
        "trie_recall" => handle_recall(trie, store, context, arguments),
        "trie_suggest_groups" => handle_suggest_groups(trie, arguments),
        "trie_group" => handle_group(trie, arguments),
        "trie_snapshot" => handle_snapshot(trie, word_trie, store, layers, arguments),
        "trie_restore" => handle_restore(trie, word_trie, store, layers, arguments),
        "trie_path_key" => handle_path_key(trie, word_trie, arguments),
        "concept_create" => handle_concept_create(trie, concepts, layers, arguments),
        "concept_bind" => handle_concept_bind(trie, word_trie, concepts, arguments),
        "concept_lookup" => handle_concept_lookup(trie, word_trie, concepts, arguments),
        "concept_bind_auto" => handle_concept_bind_auto(trie, word_trie, concepts, layers, arguments),
        "concept_snapshot" => handle_concept_snapshot(concepts, arguments),
        "concept_restore" => handle_concept_restore(concepts, arguments),
        "layer_begin" => handle_layer_begin(trie, layers, arguments),
        "layer_commit" => handle_layer_commit(layers, arguments),
        "layer_list" => handle_layer_list(layers, arguments),
        "layer_info" => handle_layer_info(layers, arguments),
        "context_show" => handle_context_show(context),
        "context_reset" => handle_context_reset(context, arguments),
        "trie_ask" => handle_ask(trie, store, context, arguments),
        _ => tool_error(&format!("Unknown tool: {}", name)),
    }
}

fn select_trie<'a>(byte_trie: &'a Trie, word_trie: &'a Trie, args: &Value) -> &'a Trie {
    match args.get("trie").and_then(|v| v.as_str()) {
        Some("word") => word_trie,
        _ => byte_trie,
    }
}

fn path_key_to_json(key: &[Option<u64>]) -> Vec<Value> {
    key.iter()
        .map(|seg| match seg {
            Some(h) => json!(h),
            None => Value::Null,
        })
        .collect()
}

fn write_to_word_trie(word_trie: &mut Trie, data: &str) -> (usize, usize, usize, u64) {
    use tokenizer::WordOrSilence;
    let items = tokenizer::tokenize_with_silence(data);
    let initial_nodes = word_trie.nodes.len();
    let mut words_written = 0usize;
    let mut tokens_processed = 0usize;
    let mut silence_ticks = 0u64;
    for item in items {
        match item {
            WordOrSilence::Word(w) => {
                let tok = tokenizer::word_token(&w);
                let r = word_trie.write(&tok);
                tokens_processed += r.tokens_processed;
                words_written += 1;
            }
            WordOrSilence::Silence(gap) => {
                for _ in 0..gap {
                    word_trie.next_tick();
                }
                silence_ticks += gap;
            }
        }
    }
    let nodes_created = word_trie.nodes.len() - initial_nodes;
    (words_written, tokens_processed, nodes_created, silence_ticks)
}

fn handle_write(trie: &mut Trie, word_trie: &mut Trie, layers: &mut LayerStore, args: &Value) -> Value {
    let data = match args.get("data").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return tool_error("Missing required parameter: data"),
    };

    let byte_result = trie.write(data.as_bytes());
    let (words_written, word_tokens, word_nodes_created, silence) =
        write_to_word_trie(word_trie, data);

    layers.record_write(words_written, current_tick(trie));

    tool_success(json!({
        "byte": {
            "tokens_processed": byte_result.tokens_processed,
            "nodes_created": byte_result.nodes_created,
            "trie_size": byte_result.trie_size,
        },
        "word": {
            "words_written": words_written,
            "tokens_processed": word_tokens,
            "nodes_created": word_nodes_created,
            "trie_size": word_trie.nodes.len(),
            "silence_ticks": silence,
        },
    }))
}

fn handle_read(trie: &Trie, args: &Value) -> Value {
    let leaf_id = match args.get("leaf_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: leaf_id"),
    };

    match trie.read(leaf_id) {
        Some(path) => tool_success(json!({ "path": path })),
        None => tool_error(&format!("Node {} not found", leaf_id)),
    }
}

fn handle_query(trie: &Trie, word_trie: &Trie, args: &Value) -> Value {
    let pattern = match args.get("pattern").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return tool_error("Missing required parameter: pattern"),
    };

    let byte_result = trie.query(pattern.as_bytes());

    // Word-trie: query each word separately (word-atom semantics).
    let per_word: Vec<Value> = tokenizer::split_words(pattern)
        .into_iter()
        .map(|w| {
            let tok = tokenizer::word_token(&w);
            let qr = word_trie.query(&tok);
            json!({
                "word": w,
                "deepest_node": qr.deepest_node,
                "match_depth": qr.match_depth,
            })
        })
        .collect();

    tool_success(json!({
        "byte": {
            "deepest_node": byte_result.deepest_node,
            "match_depth": byte_result.match_depth,
            "matches_per_depth": byte_result.matches_per_depth,
        },
        "word": {
            "per_word": per_word,
        },
    }))
}

fn handle_stats(trie: &Trie, word_trie: &Trie, args: &Value) -> Value {
    let node_id = args.get("node_id").and_then(|v| v.as_u64());
    let t = select_trie(trie, word_trie, args);

    match t.stats(node_id) {
        Some(stats) => tool_success(serde_json::to_value(stats).unwrap()),
        None => tool_error("Node not found"),
    }
}

fn handle_tokenize(args: &Value) -> Value {
    use tokenizer::WordOrSilence;
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return tool_error("Missing required parameter: text"),
    };

    let items = tokenizer::tokenize_with_silence(text);
    let mut total_words = 0u64;
    let mut total_silence = 0u64;
    let tokens_json: Vec<Value> = items
        .iter()
        .map(|item| match item {
            WordOrSilence::Word(w) => {
                total_words += 1;
                let t = tokenizer::word_token(w);
                let hash_u16 = u16::from_le_bytes(t);
                json!({
                    "type": "word",
                    "word": w,
                    "hash": format!("{:04x}", hash_u16),
                })
            }
            WordOrSilence::Silence(g) => {
                total_silence += *g;
                json!({
                    "type": "silence",
                    "silence": *g,
                })
            }
        })
        .collect();

    tool_success(json!({
        "tokens": tokens_json,
        "total_words": total_words,
        "total_silence": total_silence,
        "byte_count": text.len(),
    }))
}

fn handle_perceive(trie: &Trie, args: &Value) -> Value {
    let leaf_ids = match args.get("leaf_ids").and_then(|v| v.as_array()) {
        Some(arr) => {
            let mut ids = Vec::new();
            for v in arr {
                match v.as_u64() {
                    Some(id) => ids.push(id),
                    None => return tool_error("leaf_ids must be an array of numbers"),
                }
            }
            ids
        }
        None => return tool_error("Missing required parameter: leaf_ids"),
    };

    let result = trie.perceive(&leaf_ids);
    tool_success(serde_json::to_value(result).unwrap())
}

fn handle_perceive_window(trie: &Trie, args: &Value) -> Value {
    let last_n_ticks = args
        .get("last_n_ticks")
        .and_then(|v| v.as_u64())
        .unwrap_or(1000);

    let result = trie.perceive_window(last_n_ticks);
    tool_success(serde_json::to_value(result).unwrap())
}

fn parse_source_type(s: &str) -> Option<SourceType> {
    match s {
        "user_direct" => Some(SourceType::UserDirect),
        "user_correction" => Some(SourceType::UserCorrection),
        "agent_inference" => Some(SourceType::AgentInference),
        "sensor_direct" => Some(SourceType::SensorDirect),
        "web_fetched" => Some(SourceType::WebFetched),
        "tool_result" => Some(SourceType::ToolResult),
        "memory_recall" => Some(SourceType::MemoryRecall),
        "unknown" => Some(SourceType::Unknown),
        _ => None,
    }
}

fn parse_origin_block(v: &Value) -> Origin {
    Origin {
        author: v
            .get("author")
            .and_then(|a| a.as_str())
            .map(|s| s.to_string()),
        captured_at: v.get("captured_at").and_then(|c| c.as_u64()).unwrap_or(0),
        via: v
            .get("via")
            .and_then(|s| s.as_str())
            .unwrap_or("direct")
            .to_string(),
        chain: v
            .get("chain")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.as_u64())
                    .collect::<Vec<u64>>()
            })
            .unwrap_or_default(),
        corrects: v.get("corrects").and_then(|c| c.as_u64()),
    }
}

fn handle_remember(
    trie: &mut Trie,
    word_trie: &mut Trie,
    store: &mut ContentStore,
    args: &Value,
) -> Value {
    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return tool_error("Missing required parameter: content"),
    };
    let topic = args
        .get("topic")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Optional provenance (Task 01 schema).
    let observer_id = args
        .get("observer_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let stream_id = args
        .get("stream_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let source_type = match args.get("source_type").and_then(|v| v.as_str()) {
        Some(s) => match parse_source_type(s) {
            Some(st) => st,
            None => return tool_error(&format!("Unknown source_type: {}", s)),
        },
        None => SourceType::Unknown,
    };
    let trust_level = args
        .get("trust_level")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(1000) as u32)
        .unwrap_or_else(|| source_type_default_trust(&source_type));
    let top_level_corrects = args.get("corrects").and_then(|v| v.as_u64());
    let origin = args.get("origin").map(parse_origin_block).map(|mut o| {
        if o.corrects.is_none() && top_level_corrects.is_some() {
            o.corrects = top_level_corrects;
        }
        o
    }).or_else(|| top_level_corrects.map(|c| Origin {
        corrects: Some(c),
        via: "direct".to_string(),
        ..Default::default()
    }));
    let modality = args
        .get("modality")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let language = args
        .get("language")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let tick_start = trie.tick.load(std::sync::atomic::Ordering::Relaxed);
    let write_result = trie.write(content.as_bytes());
    let _ = write_to_word_trie(word_trie, &content);
    let tick_end = trie.tick.load(std::sync::atomic::Ordering::Relaxed);

    let query_result = trie.query(content.as_bytes());
    let path_content_ids = trie
        .path_key(query_result.deepest_node)
        .unwrap_or_default();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entry = MemoryEntry {
        timestamp,
        tick_range: (tick_start, tick_end),
        content,
        topic,
        depth_profile: query_result.matches_per_depth.clone(),
        id: 0,
        observer_id,
        session_id,
        stream_id,
        source_type,
        trust_level,
        origin,
        modality,
        language,
        path_content_ids: path_content_ids.clone(),
        legacy_origin: false,
        revised_by: None,
    };

    let deposit_id = store.add_by_path(entry);

    tool_success(json!({
        "deposit_id": deposit_id,
        "path_key": path_content_ids,
        "depth_profile": query_result.matches_per_depth,
        "tokens_processed": write_result.tokens_processed,
        "memories_stored": store.entry_count(),
        "trust_level": trust_level,
    }))
}

fn handle_recall(
    trie: &Trie,
    store: &ContentStore,
    context: &mut ContextWindow,
    args: &Value,
) -> Value {
    let query_str = match args.get("query").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return tool_error("Missing required parameter: query"),
    };
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;
    let use_context = args
        .get("use_context")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Enrich before routing — this is the Task 03 contract.
    let enriched = if use_context {
        context.enrich(&query_str)
    } else {
        super::context::EnrichedQuery {
            raw: query_str.clone(),
            enriched: query_str.clone(),
            topics_used: Vec::new(),
        }
    };

    let query_result = trie.query(enriched.enriched.as_bytes());
    let path_content_ids = trie
        .path_key(query_result.deepest_node)
        .unwrap_or_default();

    let memories = store.recall_by_path(&path_content_ids, max_results);

    let memories_json: Vec<Value> = memories
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "content": m.content,
                "topic": m.topic,
                "timestamp": m.timestamp,
                "depth_profile": m.depth_profile,
                "observer_id": m.observer_id,
                "session_id": m.session_id,
                "source_type": m.source_type,
                "trust_level": m.trust_level,
                "origin": m.origin,
                "modality": m.modality,
                "language": m.language,
                "legacy_origin": m.legacy_origin,
            })
        })
        .collect();

    // Record the exchange so the next query sees this as prior context.
    let recalled_topics: Vec<String> = memories
        .iter()
        .filter_map(|m| m.topic.clone())
        .collect();
    let tick = trie.tick.load(std::sync::atomic::Ordering::Relaxed);
    if use_context {
        context.record_exchange(&query_str, recalled_topics, tick);
    }

    tool_success(json!({
        "recognition": {
            "match_depth": query_result.match_depth,
            "deepest_node": query_result.deepest_node,
            "path_key": path_content_ids,
            "depth_profile": query_result.matches_per_depth,
        },
        "query": {
            "raw": enriched.raw,
            "enriched": enriched.enriched,
            "topics_used": enriched.topics_used,
            "context_applied": use_context,
        },
        "memories": memories_json,
        "total_matches": memories_json.len(),
    }))
}

fn handle_context_show(context: &ContextWindow) -> Value {
    let exchanges: Vec<Value> = context
        .recent_exchanges()
        .map(|e| {
            json!({
                "user_input": e.user_input,
                "topics": e.topics,
                "tick": e.tick,
            })
        })
        .collect();
    tool_success(json!({
        "observer_id": context.observer_id,
        "session_id": context.session_id,
        "conversation_depth": context.conversation_depth(),
        "hot_concepts": context.hot_concepts(),
        "exchanges": exchanges,
    }))
}

fn handle_context_reset(context: &mut ContextWindow, args: &Value) -> Value {
    let new_session = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    context.reset(new_session);
    tool_success(json!({
        "session_id": context.session_id,
        "conversation_depth": context.conversation_depth(),
    }))
}

fn handle_ask(
    trie: &Trie,
    store: &ContentStore,
    context: &mut ContextWindow,
    args: &Value,
) -> Value {
    let query_str = match args.get("query").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return tool_error("Missing required parameter: query"),
    };
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    let enriched = context.enrich(&query_str);
    // Route the *raw* query through the trie. Enrichment's original purpose
    // was boosting byte-mass on a flat trie; with the current
    // content-addressable path index plus the coverage gate, appending
    // topic keywords to the routed bytes actually moves the query to a
    // *different* subtree from the stored memory and causes literal
    // recall to miss. Enrichment is still recorded and exposed in the
    // response for diagnostics and future context-driven reranking.
    let qr = trie.query(query_str.as_bytes());
    let query_path = trie.path_key(qr.deepest_node).unwrap_or_default();
    // Recall a generous candidate pool — the cascade needs to see the
    // high-coverage memories even if they aren't the newest, and
    // `recall_by_path` truncates by timestamp before ranking. The final
    // answer still respects `max_results` via the supporting filter in
    // `select_mode_with_tick`.
    const RECALL_POOL: usize = 200;
    let recall_pool = max_results.max(RECALL_POOL);
    let memories = store.recall_by_path(&query_path, recall_pool);

    let current_tick = trie.tick.load(std::sync::atomic::Ordering::Relaxed);
    let mode_selection = responder::select_mode_with_tick(
        &query_str,
        &query_path,
        &memories,
        max_results,
        current_tick,
    );
    let rendered = responder::render(&mode_selection, &query_str, context);

    // Record exchange *after* rendering so the next turn sees this one. Only
    // record topics that actually supported the answer — UNKNOWN adds no
    // topics (honest: don't poison future context with noise).
    let topics: Vec<String> = mode_selection
        .supporting
        .iter()
        .filter_map(|m| m.topic.clone())
        .collect();
    context.record_exchange(&query_str, topics, current_tick);

    let supporting_json: Vec<Value> = mode_selection
        .supporting
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "topic": m.topic,
                "content": m.content,
                "timestamp": m.timestamp,
                "source_type": m.source_type,
                "trust_level": m.trust_level,
                "legacy_origin": m.legacy_origin,
            })
        })
        .collect();

    tool_success(json!({
        "mode": mode_selection.mode,
        "response": rendered,
        "reasoning": mode_selection.reasoning,
        "top_shared_suffix": mode_selection.top_shared_suffix,
        "top_coverage_ppm": mode_selection.top_coverage,
        "confidence": mode_selection.confidence,
        "supporting": supporting_json,
        "query": {
            "raw": enriched.raw,
            "enriched": enriched.enriched,
            "topics_used": enriched.topics_used,
        },
        "recognition": {
            "match_depth": qr.match_depth,
            "deepest_node": qr.deepest_node,
            "path_key": query_path,
        },
    }))
}

fn handle_suggest_groups(trie: &Trie, args: &Value) -> Value {
    let node_id = args.get("node_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let min_overlap = args.get("min_overlap").and_then(|v| v.as_f64()).unwrap_or(0.3);

    let suggestions = trie.suggest_groups(node_id, min_overlap);
    tool_success(serde_json::to_value(suggestions).unwrap())
}

fn handle_group(trie: &mut Trie, args: &Value) -> Value {
    let parent_id = match args.get("parent_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: parent_id"),
    };
    let child_ids = match args.get("child_ids").and_then(|v| v.as_array()) {
        Some(arr) => {
            let mut ids = Vec::new();
            for v in arr {
                match v.as_u64() {
                    Some(id) => ids.push(id),
                    None => return tool_error("child_ids must be numbers"),
                }
            }
            ids
        }
        None => return tool_error("Missing required parameter: child_ids"),
    };

    match trie.insert_intermediate(parent_id, &child_ids) {
        Ok(result) => tool_success(serde_json::to_value(result).unwrap()),
        Err(e) => tool_error(&e),
    }
}

fn handle_snapshot(trie: &Trie, word_trie: &Trie, store: &ContentStore, layers: &LayerStore, args: &Value) -> Value {
    let byte_path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_SNAPSHOT_PATH);
    let word_path = args
        .get("word_path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_WORD_SNAPSHOT_PATH);
    let content_path = args
        .get("content_path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_CONTENT_PATH);
    let layer_path = args
        .get("layer_path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_LAYER_PATH);

    let byte = match trie.snapshot(byte_path) {
        Ok(r) => r,
        Err(e) => return tool_error(&format!("Byte-trie snapshot failed: {}", e)),
    };
    let word = match word_trie.snapshot(word_path) {
        Ok(r) => r,
        Err(e) => return tool_error(&format!("Word-trie snapshot failed: {}", e)),
    };
    let content_bytes = match store.save(content_path) {
        Ok(n) => n,
        Err(e) => return tool_error(&format!("Content-store snapshot failed: {}", e)),
    };
    let layer_bytes = match layers.save(layer_path) {
        Ok(n) => n,
        Err(e) => return tool_error(&format!("Layer-store snapshot failed: {}", e)),
    };

    tool_success(json!({
        "byte": { "path": byte.path, "bytes_written": byte.bytes_written },
        "word": { "path": word.path, "bytes_written": word.bytes_written },
        "content": { "path": content_path, "bytes_written": content_bytes },
        "layers": { "path": layer_path, "bytes_written": layer_bytes },
    }))
}

fn handle_restore(trie: &mut Trie, word_trie: &mut Trie, store: &mut ContentStore, layers: &mut LayerStore, args: &Value) -> Value {
    let byte_path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_SNAPSHOT_PATH);
    let word_path = args
        .get("word_path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_WORD_SNAPSHOT_PATH);
    let content_path = args
        .get("content_path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_CONTENT_PATH);
    let layer_path = args
        .get("layer_path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_LAYER_PATH);

    let byte_restored = match Trie::restore_from(byte_path) {
        Ok(r) => r,
        Err(e) => return tool_error(&format!("Byte-trie restore failed: {}", e)),
    };
    let word_restored = match Trie::restore_from(word_path) {
        Ok(r) => r,
        Err(e) => return tool_error(&format!("Word-trie restore failed: {}", e)),
    };

    let byte_count = byte_restored.nodes.len();
    let word_count = word_restored.nodes.len();
    *trie = byte_restored;
    *word_trie = word_restored;

    // Restore content store if file exists
    let content_entries = if ContentStore::exists(content_path) {
        match ContentStore::load(content_path) {
            Ok(restored) => {
                let count = restored.entry_count();
                *store = restored;
                count
            }
            Err(e) => return tool_error(&format!("Content-store restore failed: {}", e)),
        }
    } else {
        0
    };

    // Restore layer store if file exists
    let layer_count = if LayerStore::exists(layer_path) {
        match LayerStore::load(layer_path) {
            Ok(restored) => {
                let count = restored.count();
                *layers = restored;
                count
            }
            Err(e) => return tool_error(&format!("Layer-store restore failed: {}", e)),
        }
    } else {
        0
    };

    tool_success(json!({
        "byte": { "path": byte_path, "nodes_restored": byte_count },
        "word": { "path": word_path, "nodes_restored": word_count },
        "content": { "path": content_path, "entries_restored": content_entries },
        "layers": { "path": layer_path, "layers_restored": layer_count },
    }))
}

fn handle_path_key(trie: &Trie, word_trie: &Trie, args: &Value) -> Value {
    let node_id = match args.get("node_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: node_id"),
    };
    let t = select_trie(trie, word_trie, args);

    match t.path_key(node_id) {
        Some(key) => {
            let path_key_json = path_key_to_json(&key);
            let path_key_hex: Vec<Value> = key
                .iter()
                .map(|seg| match seg {
                    Some(h) => json!(format!("{:016x}", h)),
                    None => Value::Null,
                })
                .collect();
            tool_success(json!({
                "node_id": node_id,
                "path_key": path_key_json,
                "path_key_hex": path_key_hex,
                "depth": key.len(),
            }))
        }
        None => tool_error(&format!("Node {} not found", node_id)),
    }
}

fn current_tick(trie: &Trie) -> u64 {
    use std::sync::atomic::Ordering;
    trie.tick.load(Ordering::Relaxed)
}

fn handle_concept_create(trie: &Trie, concepts: &mut ConceptStore, layers: &mut LayerStore, args: &Value) -> Value {
    let label = args.get("label").and_then(|v| v.as_str()).map(|s| s.to_string());
    let tick = current_tick(trie);
    let id = concepts.create(label, tick);
    layers.record_concept(id);
    tool_success(json!({ "concept_id": id }))
}

fn byte_key_for_input(trie: &Trie, input: &str) -> Vec<Option<u64>> {
    trie.path_key_for_input(input.as_bytes()).unwrap_or_default()
}

/// Per-word word-trie keys for `input`. Each (word, key) pair is one binding.
fn word_keys_for_input(word_trie: &Trie, input: &str) -> Vec<(String, Vec<Option<u64>>)> {
    tokenizer::split_words(input)
        .into_iter()
        .map(|w| {
            let tok = tokenizer::word_token(&w);
            let key = word_trie
                .path_key_for_input(&tok)
                .unwrap_or_default();
            (w, key)
        })
        .collect()
}

fn handle_concept_bind(
    trie: &Trie,
    word_trie: &Trie,
    concepts: &mut ConceptStore,
    args: &Value,
) -> Value {
    let concept_id = match args.get("concept_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: concept_id"),
    };
    let input = match args.get("input").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return tool_error("Missing required parameter: input"),
    };

    let byte_key = byte_key_for_input(trie, input);
    let word_keys = word_keys_for_input(word_trie, input);
    let tick = current_tick(trie);

    // Bind byte-trie key once, and each word's word-trie key separately.
    if !byte_key.is_empty() {
        let _ = concepts.bind(concept_id, byte_key.clone(), tick);
    }
    for (w, wk) in &word_keys {
        if !wk.is_empty() {
            let _ = concepts.bind_word_only(concept_id, wk.clone(), tick);
        }
        let wh = u16::from_le_bytes(tokenizer::word_token(w));
        let _ = concepts.bind_word_hash(concept_id, wh);
    }

    let word_bindings_json: Vec<Value> = word_keys
        .iter()
        .map(|(w, k)| json!({ "word": w, "word_path_key": path_key_to_json(k) }))
        .collect();

    tool_success(json!({
        "concept_id": concept_id,
        "path_key": path_key_to_json(&byte_key),
        "word_bindings": word_bindings_json,
    }))
}

fn handle_concept_lookup(
    trie: &Trie,
    word_trie: &Trie,
    concepts: &mut ConceptStore,
    args: &Value,
) -> Value {
    let input = match args.get("input").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return tool_error("Missing required parameter: input"),
    };

    let byte_key = byte_key_for_input(trie, input);
    let word_keys = word_keys_for_input(word_trie, input);

    // Direct word-hash lookup: match by word identity, not trie path keys.
    let word_hashes: Vec<u16> = word_keys
        .iter()
        .map(|(w, _)| u16::from_le_bytes(tokenizer::word_token(w)))
        .collect();
    let ranked = concepts.lookup_by_word_hashes(&word_hashes);

    let mut matched_via = "word";
    let hits: Vec<(u64, usize)> = if !ranked.is_empty() {
        ranked
    } else {
        matched_via = "byte";
        concepts
            .lookup(&byte_key)
            .iter()
            .map(|c| (c.id, 0usize))
            .collect()
    };

    if hits.is_empty() {
        matched_via = "none";
    }
    for (id, _) in &hits {
        concepts.get_accessed(*id);
    }

    let concepts_json: Vec<Value> = hits
        .iter()
        .filter_map(|(id, score)| concepts.get(*id).map(|c| (c, *score)))
        .map(|(c, score)| {
            json!({
                "id": c.id,
                "label": c.label,
                "bindings_count": c.bindings.len(),
                "word_match_score": score,
            })
        })
        .collect();

    let word_keys_json: Vec<Value> = word_keys
        .iter()
        .map(|(w, k)| json!({ "word": w, "word_path_key": path_key_to_json(k) }))
        .collect();

    tool_success(json!({
        "path_key": path_key_to_json(&byte_key),
        "word_keys": word_keys_json,
        "matched_via": matched_via,
        "concepts": concepts_json,
    }))
}

fn handle_concept_bind_auto(
    trie: &mut Trie,
    word_trie: &mut Trie,
    concepts: &mut ConceptStore,
    layers: &mut LayerStore,
    args: &Value,
) -> Value {
    let inputs = match args.get("inputs").and_then(|v| v.as_array()) {
        Some(arr) => {
            let mut strs = Vec::new();
            for v in arr {
                match v.as_str() {
                    Some(s) => strs.push(s.to_string()),
                    None => return tool_error("inputs must be an array of strings"),
                }
            }
            strs
        }
        None => return tool_error("Missing required parameter: inputs"),
    };

    if inputs.is_empty() {
        return tool_error("inputs must not be empty");
    }

    // Write all inputs into both tries in the same tick window to establish
    // recognition structure before binding.
    for input in &inputs {
        trie.write(input.as_bytes());
        write_to_word_trie(word_trie, input);
    }

    let label = args.get("label").and_then(|v| v.as_str()).map(|s| s.to_string());
    let tick = current_tick(trie);
    let concept_id = concepts.create(label, tick);
    layers.record_concept(concept_id);

    let mut bindings_json = Vec::new();
    for input in &inputs {
        let byte_key = byte_key_for_input(trie, input);
        let word_keys = word_keys_for_input(word_trie, input);
        if !byte_key.is_empty() {
            let _ = concepts.bind(concept_id, byte_key.clone(), tick);
        }
        let mut word_bindings = Vec::new();
        for (w, wk) in &word_keys {
            if !wk.is_empty() {
                let _ = concepts.bind_word_only(concept_id, wk.clone(), tick);
            }
            // Bind word hash directly for exact word matching
            let wh = u16::from_le_bytes(tokenizer::word_token(w));
            let _ = concepts.bind_word_hash(concept_id, wh);
            word_bindings.push(json!({
                "word": w,
                "word_path_key": path_key_to_json(wk),
                "word_hash": format!("{:04x}", wh),
            }));
        }
        bindings_json.push(json!({
            "input": input,
            "path_key": path_key_to_json(&byte_key),
            "word_bindings": word_bindings,
        }));
    }

    tool_success(json!({
        "concept_id": concept_id,
        "bindings": bindings_json,
    }))
}

fn handle_concept_snapshot(concepts: &ConceptStore, args: &Value) -> Value {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_CONCEPT_PATH);

    match concepts.snapshot(path) {
        Ok(bytes) => tool_success(json!({ "path": path, "bytes_written": bytes })),
        Err(e) => tool_error(&format!("Concept snapshot failed: {}", e)),
    }
}

fn handle_concept_restore(concepts: &mut ConceptStore, args: &Value) -> Value {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_CONCEPT_PATH);

    match ConceptStore::restore(path) {
        Ok(restored) => {
            let count = restored.concept_count();
            *concepts = restored;
            tool_success(json!({ "path": path, "concepts_restored": count }))
        }
        Err(e) => tool_error(&format!("Concept restore failed: {}", e)),
    }
}

fn tool_success(result: Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&result).unwrap()
        }]
    })
}

fn handle_layer_begin(trie: &Trie, layers: &mut LayerStore, args: &Value) -> Value {
    let label = match args.get("label").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return tool_error("Missing required parameter: label"),
    };
    let domain = args.get("domain").and_then(|v| v.as_str()).map(|s| s.to_string());
    let tick = current_tick(trie);
    let id = layers.begin(label, domain, tick);
    tool_success(json!({ "layer_id": id, "tick_start": tick }))
}

fn handle_layer_commit(layers: &mut LayerStore, _args: &Value) -> Value {
    match layers.commit() {
        Some(layer) => tool_success(json!({
            "layer_id": layer.id,
            "label": layer.label,
            "tick_range": [layer.tick_start, layer.tick_end],
            "word_count": layer.word_count,
            "concepts_created": layer.concept_ids.len(),
        })),
        None => tool_error("No active layer to commit"),
    }
}

fn handle_layer_list(layers: &LayerStore, args: &Value) -> Value {
    let domain_filter = args.get("domain").and_then(|v| v.as_str());
    let list: Vec<&_> = match domain_filter {
        Some(d) => layers.layers_by_domain(d),
        None => layers.layers().iter().collect(),
    };
    let items: Vec<Value> = list
        .iter()
        .map(|l| {
            json!({
                "id": l.id,
                "label": l.label,
                "domain": l.domain,
                "tick_range": [l.tick_start, l.tick_end],
                "word_count": l.word_count,
                "concepts_created": l.concept_ids.len(),
                "origin_timestamp": l.origin_timestamp,
            })
        })
        .collect();
    tool_success(json!({
        "layers": items,
        "active": layers.active().map(|a| json!({
            "id": a.id,
            "label": a.label,
            "word_count": a.word_count,
        })),
    }))
}

fn handle_layer_info(layers: &LayerStore, args: &Value) -> Value {
    let layer_id = match args.get("layer_id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: layer_id"),
    };
    match layers.get(layer_id) {
        Some(l) => tool_success(json!({
            "id": l.id,
            "label": l.label,
            "domain": l.domain,
            "origin_timestamp": l.origin_timestamp,
            "tick_start": l.tick_start,
            "tick_end": l.tick_end,
            "tick_duration": l.tick_duration(),
            "word_count": l.word_count,
            "concept_ids": l.concept_ids,
        })),
        None => tool_error(&format!("Layer {} not found", layer_id)),
    }
}

fn tool_error(message: &str) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": message
        }],
        "isError": true
    })
}
