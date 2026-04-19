# trie-memory

An append-only, integer-only hierarchical memory structure exposed as an MCP server.

## What It Is

A trie where every node classifies incoming data using exactly three outcomes:

- **Same** — recognized pattern → consume, increment visit count
- **Different** — known domain but unrecognized pattern → route to child
- **Unknown** — no experience yet → create new branch, start learning

No floats. No gradients. No loss functions. No weights. The structure *is* the knowledge.

## Core Principles

1. **Change detection, not values.** Input is always a stream of changes (deltas), never absolute state. What changed matters. What something "is" doesn't.

2. **Same / Different / Unknown are not numbers.** They are classification outcomes. You cannot add them. They don't sit on an axis. They route.

3. **Append-only.** The trie only grows. Nodes are never deleted. History is structural.

4. **Write = root → leaf.** Input flows down. "I don't know this" pushes deeper. Specialized entities live at depth.

5. **Read = leaf → root.** Retrieval flows up. Start with the most specific match, accumulate broader context walking toward root. Fixed-length path per leaf — no search needed.

6. **Visit count = mass.** Frequently visited nodes are "heavier." This is the only weighting mechanism. It's an atomic integer increment, not a learned parameter.

7. **Organic growth.** The trie structure is not designed — it emerges from what the data contains. Entities crystallize from repeated observation. New branches appear when unknown patterns arrive.

8. **Parallelizable.** Same = atomic increment (lock-free). Different = follow existing pointer (read-only). Unknown = compare-and-swap to append new branch. 99%+ of operations are contention-free.

## Architecture

```
┌─────────────┐     MCP Protocol      ┌──────────────┐
│  AI Client   │ ◄──────────────────► │  trie-memory  │
│  (Claude)    │   write / read /      │  MCP Server   │
│              │   query / stats       │               │
└─────────────┘                        └──────┬───────┘
                                              │
                                       ┌──────▼───────┐
                                       │  Trie Core   │
                                       │  (Rust)      │
                                       │              │
                                       │  append-only │
                                       │  integer-only│
                                       │  lock-free   │
                                       └──────────────┘
```

## MCP API

| Method | Direction | Description |
|--------|-----------|-------------|
| `write(stream)` | root → leaf | Feed data into the trie. Tokens flow down, trie may grow. |
| `read(leaf_id)` | leaf → root | Retrieve spectrum path from specific leaf to root. |
| `query(pattern)` | root → leaf | Find closest matching leaf for a pattern. Returns leaf_id + match depth. |
| `stats(node_id)` | — | Visit count, depth, children count, spectrum. |
| `snapshot()` | — | Serialized trie state for persistence. |

## Theoretical Background

This project implements ideas from the tick-frame-space research:

- **RAW 113** — The Semantic Isomorphism: Same / Different / Unknown
- **RAW 123** — The Stream, the Trie, and What the Data Tells Us
- **RAW 112** — The Single Mechanism

The trie stream filtering experiments (v4–v10) validated the core mechanism on real data (text, video), demonstrating that the same/different/unknown classification discovers hierarchical structure without being told what structure looks like.

## Prior Art

- **tick-frame-space/experiments/trie_stream_filtering** — Python prototypes (v4–v10) proving the mechanism works on byte streams and video frames
- **model-c** — Rust ternary computational model (v0.1–v0.14) exploring related ideas through matrix-based approach (this project takes a different, simpler path)

## License

CC BY-NC 4.0 — Free for research, academic, and educational use. Commercial use prohibited without permission.
