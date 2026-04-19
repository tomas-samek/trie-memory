# CLAUDE.md — Implementation Task (Phase 2: Two-Mechanism Architecture)

## Project: trie-memory

**Goal:** Extend the working trie-memory MCP server with a concept store (mechanism 2) and tick-based binding between trie recognition and concept storage.

**Priority:** Correctness over performance. Simple over clever. Build incrementally — each task should compile and pass tests before moving to the next.

---

## Context

The trie (mechanism 1) is a byte-delta recognition engine. It routes input, classifies it as Same/Different/Unknown, and builds depth through crystallization. It now has content-addressable path keys (FNV-1a hash of spectrum per node).

Experiments showed:
- English prose: depth 1 (flat, uniform delta patterns)
- Czech text: depth 12 (diacritics create exotic deltas)
- Rust code: depth 5 (brackets/operators diverge from prose)
- Japanese/Chinese: depth 24 (multi-byte UTF-8 = very different deltas)
- The trie differentiates **writing systems**, not topics within a language
- To differentiate meaning, we need a second mechanism: a **concept store**

The architecture is:
- **Mechanism 1 (trie):** Fast recognition index. Routes input → produces addresses (path keys). Answers "have I seen this before? how deeply do I recognize it?"
- **Mechanism 2 (concept store):** Meaning storage. Holds concepts at abstraction levels, addressed by trie path keys. Answers "what does this mean?"
- **Binding:** Signals arriving in the same tick window get linked to the same concept. No explicit wiring — temporal co-occurrence IS the binding.

---

## Task 1: Fix `content_id()` for Learning nodes

**File:** `src/trie/node.rs`

**Problem:** `content_id()` hashes the spectrum, but Learning nodes have an empty spectrum. This produces the FNV offset basis (`14695981039346656037`) for ALL Learning nodes — they all look identical, which is wrong.

**Fix:** Return `Option<u64>` instead of `u64`. Return `None` when the node is still Learning (empty spectrum). Only return `Some(hash)` when Crystallized.

```rust
/// Content-addressable ID derived purely from the delta spectrum.
/// Returns None for Learning nodes (no stable identity yet).
pub fn content_id(&self) -> Option<u64> {
    if self.state == NodeState::Learning || self.spectrum.is_empty() {
        return None;
    }
    Some(fnv1a_hash(&self.spectrum))
}
```

**Propagate the change:**
- Update `Trie::path_key()` in `src/trie/mod.rs` to handle `Option<u64>` — use `None` or a sentinel in the key vector, or skip Learning nodes, or return the partial key up to the last Crystallized node.
- Update `handle_path_key()` in `src/mcp/tools.rs` — represent `None` as `null` in JSON output.

**Test:** Feed a small amount of data (not enough to crystallize all nodes). Call `trie_path_key` on a Learning node — should show `null` for that segment. Call on a fully Crystallized path — should show all hashes.

---

## Task 2: Concept Store (Mechanism 2)

**New file:** `src/store/concept.rs`

### What it is

A store that holds **concepts** — abstract meaning nodes that can be referenced by multiple trie path keys. One concept can have multiple surface forms (e.g., "bush" in English, "křoví" in Czech, "茂み" in Japanese all point to the same concept).

### Data structure

```rust
pub type ConceptId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: ConceptId,
    pub label: Option<String>,           // optional human-readable label
    pub bindings: Vec<Binding>,          // trie addresses bound to this concept
    pub created_at: u64,                 // tick when concept was created
    pub access_count: u64,               // how often this concept was retrieved
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub path_key: Vec<Option<u64>>,      // content-addressable trie path
    pub bound_at: u64,                   // tick when binding was created
    pub strength: u64,                   // reinforced by repeated co-occurrence
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConceptStore {
    concepts: Vec<Concept>,
    /// Index: path_key hash → concept IDs (for fast lookup)
    path_index: HashMap<u64, Vec<ConceptId>>,
}
```

### API

```rust
impl ConceptStore {
    pub fn new() -> Self;

    /// Create a new concept, optionally with a label.
    pub fn create(&mut self, label: Option<String>, tick: u64) -> ConceptId;

    /// Bind a trie path key to an existing concept.
    /// If the binding already exists, increment its strength.
    pub fn bind(&mut self, concept_id: ConceptId, path_key: Vec<Option<u64>>, tick: u64) -> Result<(), String>;

    /// Look up concepts by trie path key.
    /// Returns concepts whose bindings match the given key.
    /// Supports partial matching: a shorter key matches concepts with longer keys that share the same prefix (from root).
    pub fn lookup(&self, path_key: &[Option<u64>]) -> Vec<&Concept>;

    /// Find concepts that had bindings created within a tick window.
    /// This is the temporal co-occurrence query.
    pub fn co_occurring(&self, tick: u64, window: u64) -> Vec<Vec<ConceptId>>;

    /// Persistence
    pub fn snapshot(&self, path: &str) -> Result<usize, String>;
    pub fn restore(path: &str) -> Result<Self, String>;
}
```

### Path key hashing for index

To index path keys in a HashMap, hash the entire key vector:

```rust
fn hash_path_key(key: &[Option<u64>]) -> u64 {
    let mut hash = 14695981039346656037u64; // FNV offset
    for segment in key {
        let val = segment.unwrap_or(0);
        hash ^= val;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}
```

### Register in module

Add `pub mod concept;` to `src/store/mod.rs`.

---

## Task 3: Temporal Binding MCP Tools

**File:** `src/mcp/tools.rs`

Add these MCP tools:

### `concept_create`
- Input: `{ "label": string? }`
- Creates a new concept in the ConceptStore
- Output: `{ "concept_id": number }`

### `concept_bind`
- Input: `{ "concept_id": number, "input": string }`
- Routes `input` through the trie (query, not write) to get path key
- Binds that path key to the concept
- Output: `{ "concept_id": number, "path_key": [...], "binding_strength": number }`

### `concept_lookup`
- Input: `{ "input": string }`
- Routes `input` through trie to get path key, then looks up matching concepts
- Output: `{ "path_key": [...], "concepts": [{ "id": number, "label": string?, "bindings_count": number }] }`

### `concept_bind_auto`
- Input: `{ "inputs": [string, string, ...] }`
- **This is the temporal co-occurrence binder**
- Writes ALL inputs through the trie in the same tick window
- Gets path keys for each
- Creates a new concept (or finds existing shared concept)
- Binds all path keys to that concept
- Output: `{ "concept_id": number, "bindings": [{ "input": string, "path_key": [...] }] }`
- Example: `concept_bind_auto({ "inputs": ["křoví", "bush", "茂み"] })` → creates one concept, binds all three surface forms

### `concept_snapshot` / `concept_restore`
- Same pattern as trie_snapshot/trie_restore
- Default path: `./concept-store.dat`

Don't forget to:
- Add the ConceptStore to the server state alongside the Trie and ContentStore
- Add all new tools to `tool_list()` and `call_tool()` dispatch
- Snapshot/restore concept store alongside trie on startup/shutdown

---

## Task 4: Integration Test

**File:** `tests/concept_binding.rs`

Write a test that proves the two-mechanism architecture works:

```rust
#[test]
fn test_cross_language_binding() {
    // 1. Create trie and concept store
    // 2. Feed Czech, English, Japanese text into trie to build structure
    // 3. Create a concept with label "bush/vegetation"
    // 4. Bind "křoví", "bush", "茂み" to the same concept
    // 5. Query with "křoví" → should find the concept
    // 6. Query with "bush" → should find the SAME concept
    // 7. Query with "茂み" → should find the SAME concept
    // 8. Query with "automobile" → should NOT find the concept
}

#[test]
fn test_auto_binding_co_occurrence() {
    // 1. Use concept_bind_auto equivalent with ["tree", "strom", "木"]
    // 2. Verify single concept created with 3 bindings
    // 3. Lookup via any surface form returns the same concept
}

#[test]
fn test_partial_key_matching() {
    // 1. Bind a concept at a deep path key
    // 2. Query with a shorter (more general) key
    // 3. Should still find the concept (broader match)
}
```

---

## Project Structure (updated)

```
trie-memory/
├── Cargo.toml              ✅ exists
├── CLAUDE.md               ✅ this file
├── README.md               ✅ exists
├── LICENSE                 ✅ exists
├── docs/                   ✅ exists
├── src/
│   ├── lib.rs              ✅ exists
│   ├── main.rs             ✅ exists — SSE + stdio MCP server
│   ├── trie/
│   │   ├── mod.rs          ✅ exists — Trie struct, stats, path_key
│   │   ├── node.rs         ✅ exists — Node, classify, observe, consume, crystallize, content_id, fnv1a_hash
│   │   ├── write.rs        ✅ exists — delta encoding, routing
│   │   ├── read.rs         ✅ exists — leaf→root walk
│   │   ├── query.rs        ✅ exists — read-only traversal
│   │   ├── perceive.rs     ✅ exists — batch multi-leaf read, perceive_window
│   │   ├── persistence.rs  ✅ exists — snapshot/restore
│   │   └── grouping.rs     ✅ exists — spectrum overlap, intermediate nodes
│   ├── store/
│   │   ├── mod.rs          ✅ exists
│   │   ├── memory.rs       ✅ exists — MemoryEntry, ContentStore
│   │   └── concept.rs      ❌ TODO — ConceptStore, Concept, Binding
│   └── mcp/
│       ├── mod.rs          ✅ exists
│       ├── tools.rs        ✅ exists — tool definitions and handlers
│       ├── dispatch.rs     ✅ exists
│       ├── sse.rs          ✅ exists
│       └── transport.rs    ✅ exists
└── tests/
    └── concept_binding.rs  ❌ TODO
```

---

## Implementation Notes

### What to prioritize
1. **Task 1 first** — small fix, unblocks meaningful path keys
2. **Task 2 next** — ConceptStore is pure data structure, no MCP wiring yet
3. **Task 3** — expose via MCP tools
4. **Task 4** — prove it works end-to-end

### What NOT to do
- No floats. All values are integers (except overlap_ratio in existing grouping code).
- No premature optimization. Single-threaded is fine.
- Don't restructure existing code. Build on top of it.
- Don't change the trie write/read/query paths. They work correctly.
- Don't add external dependencies unless strictly necessary.

### Key design principle
The concept store is NOT a database. It's a binding map. Concepts don't "contain" their surface forms — they're just linked by path keys. The trie owns recognition. The concept store owns meaning. They communicate through path keys.

### Temporal co-occurrence
The `concept_bind_auto` tool is the critical innovation. It takes multiple inputs, feeds them through the trie in the same tick window, and binds all resulting path keys to one concept. This is how the system learns that "křoví" and "bush" mean the same thing — not through a dictionary, but through simultaneous presentation.

---

## Success Criteria

Phase 2 is done when:
- [x] `cargo build` succeeds with no warnings
- [x] `cargo test` passes all tests including new concept_binding tests
- [x] `content_id()` returns `None` for Learning nodes
- [x] `trie_path_key` shows `null` for Learning node segments
- [x] `concept_create` creates concepts in the store
- [x] `concept_bind` links trie path keys to concepts
- [x] `concept_lookup` finds concepts by routing input through trie
- [x] `concept_bind_auto` binds multiple surface forms to one concept via co-occurrence
- [x] Cross-language lookup works: bind "křoví"+"bush"+"茂み" → query any one → find same concept
- [x] `concept_snapshot` / `concept_restore` round-trips correctly
- [x] Claude can connect and use all new tools via MCP

---

---

# Phase 3: Word-Token Architecture (Revised)

## What we learned

We tried three approaches for the word-level trie:
1. **Hash each word to 4 bytes, feed as continuous stream** → delta encoding smoothed out differences between word hashes
2. **Weight-encode characters (eye-like perception)** → root absorbed everything because English letter transitions are too uniform
3. **Weight-encode + per-word separate writes** → same problem, within-word letter deltas all look the same

The fundamental discovery: **individual letter transitions cannot carry word identity.** No matter how you encode letters, "catalog" and "endpoint" use the same 26 characters with similar patterns. The trie needs word-sized atoms, not letter-sized.

The key insight about silence: **space is not data — it is lack of data.** You don't encode silence. You stop encoding. The GAP between writes (tick difference) IS the silence. And the AMOUNT of silence encodes structure:
- Word gap (space) → 1 tick → tightly bound
- Phrase gap (comma) → small tick gap → related
- Sentence gap (period) → larger tick gap → same topic, different thought
- Paragraph gap → many ticks → topic shift

## Solution: Three mechanisms working together

1. **Word hash** → each word becomes a 2-byte token (single atom)
2. **Tick gap** → silence duration between writes encodes structure
3. **Concept binding** → temporal proximity in tick space determines meaning

## FIRST: Clean Start

**Delete all data files before implementing:**
- Delete `trie-memory.dat` (byte trie data)
- Delete `word-trie-memory.dat` (word trie data)
- Delete `concept-store.json` (concept store data)
- Start completely fresh — old data was experimental

---

## Task 5: Word Tokenizer (Revised)

**File:** `src/trie/tokenizer.rs`

### Keep existing
- `split_words()` — CamelCase/dot/underscore splitter. Works correctly, keep it.
- `weight_encode()` — may be useful later, keep it.

### Add word-to-token function

Each word becomes exactly 2 bytes (u16 hash). This is the word's identity — its "shape" as seen by the trie.

```rust
/// Convert a word to a 2-byte token (u16 FNV-1a hash, little-endian).
/// This is the word's atomic identity in the word-trie.
pub fn word_token(word: &str) -> [u8; 2] {
    let hash = fnv1a_hash(word.to_lowercase().as_bytes()) as u16;
    hash.to_le_bytes()
}

/// Tokenize input into word tokens. Returns a vec of (word, 2-byte token) pairs.
/// Used for diagnostics and for the write path.
pub fn tokenize_words(input: &str) -> Vec<(String, [u8; 2])> {
    split_words(input)
        .into_iter()
        .map(|w| {
            let token = word_token(&w);
            (w, token)
        })
        .collect()
}
```

### Why 2 bytes per word?
- u16 = 65536 distinct values. For typical vocabulary of 200-500 words, collision probability is near zero.
- 2 bytes per word = 2 trie tokens per word. Enough for the trie to build depth without drowning in data.
- Each word is ONE write call of 2 bytes. Clean, atomic.

---

## Task 6: Variable Tick Gaps for Silence

**File:** `src/trie/tokenizer.rs`

Add a function that determines how many ticks to advance based on the separator between words:

```rust
/// Determine tick gap for a separator character.
/// Encodes the "silence duration" — how much pause between signals.
pub fn silence_ticks(separator: char) -> u64 {
    match separator {
        ' ' => 1,                          // word gap: tightly bound
        '-' | '_' => 1,                    // compound word: tightly bound
        ',' => 3,                          // phrase boundary
        ';' | ':' => 5,                    // clause boundary
        '.' | '!' | '?' => 10,            // sentence boundary
        '\n' => 20,                        // line break
        _ => 1,                            // default: tight
    }
}

/// Split input into words AND separators, preserving the structure.
/// Returns Vec<WordOrSilence> for the write path to process.
pub enum WordOrSilence {
    Word(String),
    Silence(u64),  // tick gap
}

pub fn tokenize_with_silence(input: &str) -> Vec<WordOrSilence> {
    // Walk the input character by character.
    // Accumulate letters into words.
    // When hitting a separator, emit the word, then emit Silence with the tick gap.
    // When hitting CamelCase boundary, emit the sub-word, then Silence(1).
    // Multiple consecutive separators: use the LARGEST tick gap (paragraph = many newlines).
    // ...
}
```

---

## Task 7: Word-Trie Write Path

**File:** `src/mcp/tools.rs`

### `trie_write` — updated

The word-trie write path now:
1. Tokenizes input into words and silences
2. For each word: hash to 2 bytes, call `word_trie.write(&token)`
3. For each silence: advance `word_trie.tick` by the gap amount (WITHOUT writing any data)

```rust
fn write_to_word_trie(word_trie: &mut Trie, data: &str) -> (usize, usize) {
    let tokens = tokenizer::tokenize_with_silence(data);
    let mut words_written = 0;
    let mut tokens_processed = 0;
    
    for item in tokens {
        match item {
            WordOrSilence::Word(w) => {
                let token = tokenizer::word_token(&w);
                let result = word_trie.write(&token);
                tokens_processed += result.tokens_processed;
                words_written += 1;
            }
            WordOrSilence::Silence(gap) => {
                // Advance tick counter without writing — this IS the silence
                for _ in 0..gap {
                    word_trie.next_tick();
                }
            }
        }
    }
    (words_written, tokens_processed)
}
```

### `trie_query` — updated for word-trie

Query the word-trie by querying each word separately:
```rust
fn query_word_trie(word_trie: &Trie, pattern: &str) -> Vec<QueryResult> {
    let words = tokenizer::split_words(pattern);
    words.iter().map(|w| {
        let token = tokenizer::word_token(w);
        word_trie.query(&token)
    }).collect()
}
```

Return per-word query results so the caller can see which words were recognized and at what depth.

### `trie_tokenize` — updated diagnostic

Show the new tokenization:
```
Input: { "text": "CatalogQuery. SearchResult" }
Output: {
    "tokens": [
        { "word": "catalog", "hash": "d8e9", "type": "word" },
        { "silence": 1, "type": "silence" },
        { "word": "query", "hash": "c36c", "type": "word" },
        { "silence": 10, "type": "silence" },
        { "word": "search", "hash": "a928", "type": "word" },
        { "silence": 1, "type": "silence" },
        { "word": "result", "hash": "2476", "type": "word" }
    ],
    "total_words": 4,
    "total_silence": 13
}
```

---

## Task 8: Concept Binding with Word Tokens

**File:** `src/mcp/tools.rs` and `src/store/concept.rs`

### `keys_for_input` — updated

For concept binding/lookup, generate word-trie keys per-word:
```rust
fn word_keys_for_input(word_trie: &Trie, input: &str) -> Vec<(String, Vec<Option<u64>>)> {
    let words = tokenizer::split_words(input);
    words.iter().map(|w| {
        let token = tokenizer::word_token(w);
        let qr = word_trie.query(&token);
        let key = word_trie.path_key(qr.deepest_node).unwrap_or_default();
        (w.clone(), key)
    }).collect()
}
```

### `concept_bind_auto` — updated

When binding "CatalogQuery" to a concept:
1. Split into ["catalog", "query"]
2. Get word-trie path key for each
3. Bind EACH word's path key to the concept
4. The concept now has multiple word-level bindings

### `concept_lookup` — updated

When looking up "CatalogQuery":
1. Split into ["catalog", "query"]
2. Get word-trie path key for each
3. Find concepts that match ANY of the words
4. **Rank by overlap**: concept matching BOTH "catalog" AND "query" ranks higher than one matching only "catalog"
5. Return ranked results

Add to ConceptStore:
```rust
/// Look up concepts by multiple word keys. Returns concepts ranked by
/// how many of the query words match their bindings.
pub fn lookup_multi_word(&self, word_keys: &[(String, Vec<Option<u64>>)]) -> Vec<(ConceptId, usize)> {
    // For each word key, find matching concepts
    // Count how many query words each concept matches
    // Return sorted by match count (descending)
}
```

---

## Task 9: Integration Tests

**File:** `tests/word_token_trie.rs`

```rust
#[test]
fn test_word_token_determinism() {
    // Same word always produces same 2-byte token
    assert_eq!(word_token("catalog"), word_token("catalog"));
    assert_ne!(word_token("catalog"), word_token("query"));
    assert_ne!(word_token("catalog"), word_token("endpoint"));
}

#[test]
fn test_word_trie_builds_depth() {
    // Feed 50+ distinct words, verify trie builds beyond depth 1
    let mut trie = Trie::new();
    let words = ["catalog", "query", "search", "result", "audit", "manager",
                  "component", "endpoint", "identity", "service", "dependency",
                  "resolver", "index", "writer", "storage", "provider",
                  "configuration", "event", "dispatcher", "logger"];
    for _ in 0..20 {  // repeat to build volume
        for word in &words {
            let token = word_token(word);
            trie.write(&token);
        }
    }
    let stats = trie.stats(None).unwrap();
    assert!(stats.total_nodes > 5, "Word trie should build real depth");
}

#[test]
fn test_word_trie_differentiates_words() {
    // "catalog" and "endpoint" should produce different path keys
    let mut trie = Trie::new();
    // Feed enough data...
    // Then:
    let cat_key = trie.path_key_for_input(&word_token("catalog"));
    let end_key = trie.path_key_for_input(&word_token("endpoint"));
    assert_ne!(cat_key, end_key);
}

#[test]
fn test_silence_encoding() {
    let tokens = tokenize_with_silence("CatalogQuery. SearchResult");
    // Should produce: Word("catalog"), Silence(1), Word("query"), Silence(10), 
    //                 Word("search"), Silence(1), Word("result")
}

#[test]
fn test_concept_multi_word_lookup() {
    // Bind "CatalogQuery" to concept "lucene" (binds both "catalog" and "query")
    // Bind "CatalogEndpoint" to concept "rest-api" (binds "catalog" and "endpoint")
    // Lookup "CatalogQuery" → both words match "lucene" (score 2), only "catalog" matches "rest-api" (score 1)
    // Result: "lucene" ranked first
    //
    // THIS IS THE TEST THAT PROVES THE ARCHITECTURE WORKS
}
```

---

## Implementation Notes for Phase 3 (Revised)

### What to prioritize
1. **Delete all data files first** — fresh start
2. **Task 5** — word_token() function (tiny, pure)
3. **Task 6** — silence_ticks and tokenize_with_silence
4. **Task 7** — wire into MCP write/query paths
5. **Task 8** — update concept binding/lookup for multi-word
6. **Task 9** — prove it works

### What NOT to do
- Don't modify the Trie struct. Same code for both byte and word tries.
- Don't remove byte-trie functionality. Both run in parallel.
- Don't over-engineer silence detection. Simple char-based rules are fine.
- Don't try to make the byte-trie do word-level work. It's a script classifier. Let it be.

### Key design principles
- **Silence is not data.** Space/pause = advance tick, don't write.
- **Words are atoms.** Each word is ONE recognition event (2-byte hash).
- **Timing is structure.** Tick gaps encode grammar (tight = same phrase, loose = different topic).
- **Lookup is intersection.** More matching words = stronger concept match.

### About the byte-trie
The byte-trie still serves a purpose: it classifies writing systems (English vs Czech vs Japanese vs code). It's the "what language is this?" layer. The word-trie is the "what topic is this?" layer. They complement each other.

---

## Success Criteria

Phase 3 is done when:
- [ ] All data files deleted (trie-memory.dat, word-trie-memory.dat, concept-store.json)
- [ ] `cargo build` succeeds with no warnings
- [ ] `cargo test` passes all tests
- [ ] `word_token("catalog") != word_token("endpoint")` (deterministic, distinct)
- [ ] Word-trie builds 5+ nodes from 20 distinct words repeated 20 times
- [ ] `trie_tokenize` shows words + silence gaps
- [ ] `concept_bind_auto` with "CatalogQuery" binds both "catalog" and "query" words
- [ ] `concept_lookup` for "CatalogQuery" ranks concepts by word overlap
- [ ] "CatalogQuery" lookup finds "lucene" (2 words match) over "rest-api" (1 word match)
- [ ] Variable tick gaps work: word space=1, period=10, newline=20
- [x] Both tries snapshot/restore correctly

---
---

# Phase 4: Layered Memory with Commit History

## What we learned

The current system loses data on restart because:
1. Content store paths were mismatched between main.rs and tools.rs (FIXED)
2. There's no structured way to organize what the system knows
3. Each feeding session is ephemeral — no record of WHAT was taught WHEN

The trie is append-only. Memory should be too. Each "commit" adds a layer of knowledge that never gets deleted or overwritten.

## Design: Two Timelines

Each memory layer has:
- **Origin timestamp** — wall-clock time when committed. Answers: "when did you learn this?"
- **Relative tick range** — internal tick 0..N within the layer. Answers: "how are things related within this knowledge?"

Relative ticks enable structural comparison across layers:
- Two layers with same tick spacing = same granularity = comparable
- A word at relative tick 5 in layer A is structurally equivalent to a word at relative tick 5 in layer B
- Overlaying layers by relative ticks reveals structural similarities

## Data Structures

**New file:** `src/store/layer.rs`

```rust
use serde::{Deserialize, Serialize};

pub type LayerId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLayer {
    pub id: LayerId,
    pub label: String,                  // human-readable: "java codebase", "novel ch1-5"
    pub domain: Option<String>,         // optional domain prefix: "code:java", "prose", "math"
    pub origin_timestamp: u64,          // wall-clock time when committed (epoch secs)
    pub tick_start: u64,                // absolute tick when layer begins
    pub tick_end: u64,                  // absolute tick when layer ends
    pub word_count: usize,              // total words fed in this layer
    pub concept_ids: Vec<u64>,          // concepts created during this layer
}

impl MemoryLayer {
    /// Relative tick for a given absolute tick within this layer.
    pub fn relative_tick(&self, absolute_tick: u64) -> Option<u64> {
        if absolute_tick >= self.tick_start && absolute_tick <= self.tick_end {
            Some(absolute_tick - self.tick_start)
        } else {
            None
        }
    }

    /// Duration in ticks.
    pub fn tick_duration(&self) -> u64 {
        self.tick_end - self.tick_start
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LayerStore {
    layers: Vec<MemoryLayer>,
    /// Currently active (uncommitted) layer, if any.
    active: Option<MemoryLayer>,
}

impl LayerStore {
    pub fn new() -> Self {
        Default::default()
    }

    /// Begin a new layer. All writes between begin and commit belong to this layer.
    pub fn begin(&mut self, label: String, domain: Option<String>, tick_start: u64) -> LayerId {
        let id = self.layers.len() as LayerId;
        self.active = Some(MemoryLayer {
            id,
            label,
            domain,
            origin_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            tick_start,
            tick_end: tick_start,  // updated on commit
            word_count: 0,
            concept_ids: Vec::new(),
        });
        id
    }

    /// Update word count and tick_end on the active layer (called during writes).
    pub fn record_write(&mut self, words: usize, current_tick: u64) {
        if let Some(ref mut layer) = self.active {
            layer.word_count += words;
            layer.tick_end = current_tick;
        }
    }

    /// Record a concept created during this layer.
    pub fn record_concept(&mut self, concept_id: u64) {
        if let Some(ref mut layer) = self.active {
            layer.concept_ids.push(concept_id);
        }
    }

    /// Commit the active layer — freeze it, add to history.
    pub fn commit(&mut self) -> Option<&MemoryLayer> {
        if let Some(layer) = self.active.take() {
            self.layers.push(layer);
            self.layers.last()
        } else {
            None
        }
    }

    /// Get all committed layers.
    pub fn layers(&self) -> &[MemoryLayer] {
        &self.layers
    }

    /// Get layer by ID.
    pub fn get(&self, id: LayerId) -> Option<&MemoryLayer> {
        self.layers.get(id as usize)
    }

    /// Find layers that contain a given absolute tick.
    pub fn layers_at_tick(&self, tick: u64) -> Vec<&MemoryLayer> {
        self.layers.iter()
            .filter(|l| tick >= l.tick_start && tick <= l.tick_end)
            .collect()
    }

    /// Find layers by domain prefix.
    pub fn layers_by_domain(&self, domain: &str) -> Vec<&MemoryLayer> {
        self.layers.iter()
            .filter(|l| l.domain.as_deref() == Some(domain))
            .collect()
    }

    /// Find layer that was active at a given wall-clock timestamp.
    pub fn layers_at_time(&self, timestamp: u64) -> Vec<&MemoryLayer> {
        self.layers.iter()
            .filter(|l| l.origin_timestamp <= timestamp)
            .collect()
    }

    /// Get the active (uncommitted) layer, if any.
    pub fn active(&self) -> Option<&MemoryLayer> {
        self.active.as_ref()
    }

    /// Total number of committed layers.
    pub fn count(&self) -> usize {
        self.layers.len()
    }

    pub fn save(&self, path: &str) -> std::io::Result<usize> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let len = json.len();
        std::fs::write(path, &json)?;
        Ok(len)
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let store: Self = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(store)
    }

    pub fn exists(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}
```

### Register in module
Add `pub mod layer;` to `src/store/mod.rs`.

---

## Task 10: MCP Tools for Layers

**File:** `src/mcp/tools.rs`

Add LayerStore to server state alongside Trie, ContentStore, ConceptStore.

Add constant: `const DEFAULT_LAYER_PATH: &str = "W:/data/trie-store/layer-store.json";`

### New tools:

**`layer_begin`** — Start a new memory layer
- Input: `{ "label": string, "domain": string? }`
- Gets current tick from trie
- Calls `layers.begin(label, domain, current_tick)`
- Output: `{ "layer_id": number, "tick_start": number }`

**`layer_commit`** — Freeze the current layer
- Input: `{}` (no args needed)
- Calls `layers.commit()`
- Output: `{ "layer_id": number, "label": string, "tick_range": [start, end], "word_count": number, "concepts_created": number }`

**`layer_list`** — Show all committed layers
- Input: `{ "domain": string? }` (optional domain filter)
- Output: array of layer summaries

**`layer_info`** — Get details about a specific layer
- Input: `{ "layer_id": number }`
- Output: full layer info including concept_ids, tick range, word count

### Update existing tools:

**`trie_write`** — If active layer exists, call `layers.record_write(word_count, current_tick)` after writing.

**`concept_bind_auto`** and **`concept_create`** — If active layer exists, call `layers.record_concept(concept_id)` after creating.

**`trie_snapshot`** / **`trie_restore`** — Also save/load the LayerStore alongside everything else.

---

## Task 11: Layer-Aware Persistence

**File:** `src/main.rs`

Add to server state:
```rust
const DEFAULT_LAYER_PATH: &str = "W:/data/trie-store/layer-store.json";
```

Add `load_layers()` and `save_layers()` functions following the same pattern as the other stores.

Add to `save_all()` function. Add to SSE server state and stdio loop.

Update `call_tool()` signature to accept `&mut LayerStore`.

---

## Task 12: Integration Test

**File:** `tests/layer_test.rs`

```rust
#[test]
fn test_layer_lifecycle() {
    let mut layers = LayerStore::new();
    
    // Begin a layer
    let id = layers.begin("test layer".into(), Some("math".into()), 100);
    assert_eq!(id, 0);
    assert!(layers.active().is_some());
    
    // Record some activity
    layers.record_write(50, 200);
    layers.record_concept(0);
    
    // Commit
    let layer = layers.commit().unwrap();
    assert_eq!(layer.tick_start, 100);
    assert_eq!(layer.tick_end, 200);
    assert_eq!(layer.word_count, 50);
    assert_eq!(layer.concept_ids, vec![0]);
    assert!(layers.active().is_none());
    
    // Query
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
    assert_eq!(layer.relative_tick(999), None);  // before layer
    assert_eq!(layer.relative_tick(2001), None); // after layer
}

#[test]
fn test_layer_persistence() {
    let mut layers = LayerStore::new();
    layers.begin("test".into(), None, 0);
    layers.record_write(10, 100);
    layers.commit();
    
    let path = "./test-layers.json";
    layers.save(path).unwrap();
    let loaded = LayerStore::load(path).unwrap();
    assert_eq!(loaded.count(), 1);
    std::fs::remove_file(path).ok();
}
```

---

## Usage Pattern

Teaching the system looks like this:

```
layer_begin(label: "java codebase", domain: "code:java")
  trie_write("CatalogQuery SearchResult ...")
  concept_bind_auto(["CatalogQuery", ...], label: "lucene")
  trie_remember("CatalogQuery is the main search class", topic: "lucene")
layer_commit()

layer_begin(label: "novel chapters 1-5", domain: "prose:novel")
  trie_write("The corridors were empty...")
  concept_bind_auto(["corridors dark frost", ...], label: "Heph'ai")
layer_commit()

layer_begin(label: "math basics 0-5", domain: "math")
  concept_bind_auto(["1", "one", "jedna"], label: "quantity: 1")
layer_commit()
```

Later, querying:
```
layer_list()  →  3 layers: java, novel, math
layer_list(domain: "code:java")  →  1 layer
layer_info(0)  →  { tick_range: [0, 1500], word_count: 222, concepts: [0, 1] }
```

The system knows WHAT it knows, WHEN it learned it, and HOW things relate within each body of knowledge.

---

## Implementation Notes

### What to prioritize
1. **Task 10** — LayerStore is a simple data structure
2. **Task 11** — Wire into MCP tools
3. **Task 12** — Wire persistence into main.rs and save/restore
4. **Task 13** — Integration tests

### What NOT to do
- Don't change existing trie/concept mechanisms
- Don't require layers — they should be optional. If no layer is active, tools work exactly as before
- Don't do cross-layer comparison yet — that's a future feature
- Layer store is separate from content store and concept store

### Key principle
Layers are bookmarks in the tick timeline. They don't own data — the trie owns data, the concept store owns bindings. Layers just record what happened when, so you can navigate the history.

---

## Success Criteria

Phase 4 is done when:
- [ ] `cargo build` succeeds with no warnings
- [ ] `cargo test` passes all tests including layer tests
- [ ] `layer_begin` creates an active layer with current tick
- [ ] `trie_write` within an active layer updates word count and tick range
- [ ] `concept_bind_auto` within an active layer records concept IDs
- [ ] `layer_commit` freezes the layer and adds to history
- [ ] `layer_list` shows all committed layers with summaries
- [ ] `layer_info` shows full details for a specific layer
- [ ] LayerStore persists across restarts via snapshot/restore
- [ ] Existing tools work unchanged when no layer is active
- [ ] Relative tick calculation works correctly
