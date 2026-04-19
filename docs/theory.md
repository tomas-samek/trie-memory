# Theoretical Foundation

## Origin

This project implements the computational model described in the tick-frame-space research,
specifically the trie stream filtering experiments (v4–v10) and the theoretical documents
RAW 113 (The Semantic Isomorphism) and RAW 123 (The Stream and the Trie).

## The Core Idea

Every observation produces exactly one of three classification outcomes:

| Outcome     | Meaning                          | Action           |
|-------------|----------------------------------|------------------|
| **Same**    | I recognize this                 | Consume, count   |
| **Different** | I don't recognize it, but my children might | Route deeper |
| **Unknown** | I have no experience with this   | Learn / grow     |

These are not numbers. They cannot be added. They are routing decisions.

## Key Properties

### Change, Not Values

The trie receives a stream of changes (deltas), never absolute values. This is
inspired by biological sensors — retinal cells report brightness *change*, not
brightness. The first thing any system needs to learn is that 0 ≠ 1. Everything
else builds from there.

### Append-Only

The trie only grows. No nodes are deleted. No spectra are modified after
crystallization. History is structural — the shape of the trie IS the
accumulated knowledge.

### Asymmetric Read/Write

- **Write (root → leaf):** "I don't know this" pushes deeper. Specialization increases with depth.
- **Read (leaf → root):** Start with the most specific thing you know, walk up for broader context.

This mirrors perception: you see "red blob" first (leaf), then resolve finer
detail (atoms) by looking more carefully (walking toward root where the simplest
patterns live).

### Visit Count as Mass

The only numerical quantity in the system. Frequently visited nodes are "heavier."
This provides a natural importance signal without any learned weights.

## Experimental Validation

The mechanism was validated in Python prototypes:

- **v7:** Raw bytes fed through cascade. Mechanism distinguishes structured (English) from random data without being told what structure is.
- **v8:** Causal window — deeper entities learn better. Hierarchy inverts naturally.
- **v9:** Video frame diffs. Root learns "no change." Children separate static/motion/noise autonomously.
- **v10:** 100% lossless reconstruction from stored trie. Progressive retrieval by depth (22% → 45% → 71% → 84% → 93% → 100%).

## References

- RAW 113 — The Semantic Isomorphism: Same / Different / Unknown
- RAW 123 — The Stream, the Trie, and What the Data Tells Us  
- RAW 112 — The Single Mechanism
- tick-frame-space/experiments/trie_stream_filtering (v4–v10)
- model-c (v0.1–v0.14) — earlier Rust exploration of related ideas
