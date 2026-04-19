# Design: Trie as Recognition Index + Content Store

**Status:** Ready for implementation  
**Date:** April 8, 2026

## What We Learned

The trie is a **familiarity detector**, not a content store. It answers "have I seen this before?" and "how specifically do I recognize it?" but cannot answer "what was it about?"

This maps to a real cognitive phenomenon: **feeling of knowing** — the brain's recognition signal fires separately from recall. Alzheimer's patients lose recall long before recognition. Recognition is the older, faster, more fundamental layer.

### Evidence from testing

Query depth profiles distinguish topics the trie has experienced:

```
"reactor containment hull"      → depth 0:29, 1:4, 2:1         (very familiar — book content)
"bronze age disease refugees"   → depth 0:63, 1:11, 2-6:spread (specifically recognized — our discussion)
"cooking pasta recipe"          → depth 0:15, 1:4, 7:1         (unknown — never discussed)
```

The trie knows THAT it has seen something, and HOW SPECIFICALLY it recognizes it (depth spread). It does not know WHAT it was about.

## Architecture

Two-layer memory, same as the brain:

```
Layer 1: Trie (recognition index)
  - Input arrives → routes through trie → lands at depth N
  - Depth profile = recognition signature = lookup key
  - "I've seen this, and here's how specifically I recognize it"

Layer 2: Content Store (actual memory)
  - Key: recognition signature (from trie)
  - Value: actual content (text, topic, timestamp, context)
  - "Here's what happened when I last saw something like this"
```

### Flow

**Write (storing a memory):**
```
1. Content arrives (conversation text)
2. Feed through trie → get recognition signature (which nodes fired, at what depths)
3. Store content in content-store, keyed by signature
4. Trie remembers the PATTERN, store remembers the CONTENT
```

**Read (recalling a memory):**
```
1. New input arrives
2. Feed through trie (query, read-only) → get recognition signature
3. Use signature as key → look up content store
4. Return: "Last time I saw something like this, here's what it was about"
```

### What the recognition signature looks like

From trie_query, we get a depth profile:
```json
{"deepest_node": 7, "match_depth": 7, "matches_per_depth": [[0,4], [1,1], [4,1], [7,1]]}
```

The signature could be encoded as: the node path from the deepest match to root, or simply the depth distribution as a compact key. Options:

**Option A (simple):** Use `deepest_node` ID as the key. Nodes at the same depth with the same parent represent the same "recognition class."

**Option B (richer):** Use the full depth distribution `[0:4, 1:1, 4:1, 7:1]` as a composite key. Different patterns that hit the same depths but different counts are distinguishable.

**Option C (recommended):** Use the set of node IDs that fired during the query. `{0, 1, 4, 7}` — this is unique to the specific path through the trie and acts as a natural composite key.

Start with Option A — simplest, works, can evolve.

## Implementation Plan

### The content store is intentionally dumb

It's a key-value store. Nothing fancy:

```
Key:   node_id (u64) — the deepest node reached during trie recognition
Value: Vec<MemoryEntry>
```

Where:
```rust
struct MemoryEntry {
    timestamp: u64,        // when this was stored  
    tick_range: (u64, u64), // trie ticks covered
    content: String,       // the actual text/summary
    topic: Option<String>, // optional topic label
    depth_profile: Vec<(u32, usize)>, // full depth distribution for this write
}
```

Multiple entries per key — the same recognition pattern can occur multiple times (different conversations about similar topics).

### New MCP tools

**`trie_remember`** — Combined write + store
- Input: `{ "content": string, "topic": string (optional) }`
- Action: 
  1. Feed content bytes through trie (write path — trie grows)
  2. Query the content against the trie (get recognition signature)
  3. Store content + signature in content store
- Output: `{ "key": node_id, "depth_profile": [...], "tokens_processed": N }`

**`trie_recall`** — Combined query + lookup
- Input: `{ "query": string, "max_results": number (default 5) }`
- Action:
  1. Query the trie (read-only — no modification)
  2. Get deepest_node as key
  3. Look up content store for entries under that key
  4. Also check nearby keys (parent nodes) for broader matches
- Output: `{ "recognition": { depth_profile, match_depth }, "memories": [{ content, topic, timestamp }] }`

**Keep all existing tools** — trie_write, trie_query, trie_read, trie_perceive, trie_perceive_window, trie_stats, trie_snapshot, trie_restore all stay as-is. The new tools are higher-level operations built on top.

### Content store persistence

The content store is a separate file from the trie snapshot:
- Trie: `trie-memory.dat` (binary, existing)  
- Content: `trie-content.json` (JSON, new — human readable for debugging)

Snapshot and restore should handle both files.

### Updated snapshot/restore

**`trie_snapshot`** — saves both trie state and content store  
**`trie_restore`** — restores both

Or add separate tools:
**`content_snapshot`** / **`content_restore`** — just the content store

Start with extending existing snapshot/restore to handle both.

## Example Usage

### Storing a conversation

```
Agent calls: trie_remember({
  "content": "We discussed the Bronze Age Collapse. Tomas proposed disease as the primary trigger, carried through interconnected trade networks. Sea Peoples were refugees, not invaders. Burned cities may be quarantine not conquest. Urban populations had lower immunity than rural farmers who lived with livestock daily.",
  "topic": "Bronze Age Collapse"
})

Returns: {
  "key": 7,
  "depth_profile": [[0, 180], [1, 28], [2, 5], [3, 2], [4, 1], [7, 1]],
  "tokens_processed": 412
}
```

### Recalling later

```
Agent calls: trie_recall({
  "query": "what did we discuss about ancient civilizations collapsing"
})

Returns: {
  "recognition": { "match_depth": 4, "depth_profile": [[0, 40], [1, 6], [4, 1]] },
  "memories": [
    {
      "content": "We discussed the Bronze Age Collapse...",
      "topic": "Bronze Age Collapse",
      "timestamp": 1712564400
    }
  ]
}
```

The agent (Claude) then uses this recalled content to inform its response.

## Data Structures

### Content Store

```rust
use std::collections::HashMap;

struct ContentStore {
    entries: HashMap<u64, Vec<MemoryEntry>>,  // key = deepest node_id
}

struct MemoryEntry {
    timestamp: u64,
    tick_range: (u64, u64),
    content: String,
    topic: Option<String>,
    depth_profile: Vec<(u32, usize)>,
}
```

### File layout

```
trie-memory/
├── src/
│   ├── trie/          (existing — unchanged)
│   ├── mcp/           (existing — add new tool handlers)
│   ├── store/          (NEW)
│   │   ├── mod.rs     — ContentStore struct, add/lookup/persistence
│   │   └── memory.rs  — MemoryEntry, serialization
│   ├── lib.rs         (update module declarations)
│   └── main.rs        (update to load/save content store alongside trie)
```

## What NOT to Do

- Don't change the trie core — it works, don't touch it
- Don't make the content store clever — it's a HashMap, keep it that way
- Don't try to do semantic search in the content store — the trie handles recognition, the store is just retrieval
- Don't combine the trie snapshot and content store into one file — keep them separate for independent debugging

## Success Criteria

- [ ] `trie_remember` stores content with recognition key
- [ ] `trie_recall` returns stored content for matching queries
- [ ] Content persists across restarts (content store saves/restores)
- [ ] Existing trie tools all still work unchanged
- [ ] Claude can call `trie_recall("bronze age")` and get back the actual discussion summary
