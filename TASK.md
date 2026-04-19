# Task: Allow Depth Growth — Route Into Childless Crystallized Nodes

**Status:** ✅ Completed (verified 2026-04-19)

- Fix applied in `src/trie/write.rs::route()` (Phase 2 recurses into all
  crystallized children, skips Learning).
- Fix applied in `src/trie/query.rs::query_route()` (same logic, read-only).
- All four test categories below covered in `tests/trie_growth.rs` and
  passing (`cargo test --test trie_growth`: 31 passed, 0 failed).
- Unblocks honest-agent Phase B — see
  `docs/design/honest_agent/progress.md` Prerequisites section.

## The Problem

The trie only grows WIDE (new siblings), never DEEP (grandchildren). After feeding alphabet, syllables, words, sentences, and code — root has 8 children, ALL at depth 1. Zero depth 2 nodes.

**Root cause:** Phase 2 in `route()` only recurses into children that already have subtrees:

```rust
// Phase 2: recurse into children with subtrees
for &child_id in &children {
    let cidx = child_id as usize;
    if !self.nodes[cidx].children.is_empty() {  // ← BLOCKS DEPTH GROWTH
        if self.route(cidx, token, tick) {
            return true;
        }
    }
}
```

A childless crystallized node is skipped. But it can never GET children because it's never routed into. Chicken-and-egg.

Additionally, with the classify fix (crystallized nodes never return Unknown), `handle_unknown` for crystallized nodes is dead code. The ONLY way children are created is Phase 3, which always creates siblings at the current level.

## The Fix

Remove the `!children.is_empty()` guard in Phase 2. Route into ALL crystallized children, not just ones with subtrees:

```rust
// Phase 2: All children said Different at this level.
// Recurse into each child's subtree (or the child itself if childless).
for &child_id in &children {
    let cidx = child_id as usize;
    if self.nodes[cidx].state == NodeState::Crystallized {
        if self.route(cidx, token, tick) {
            return true;
        }
    }
}
```

When `route(child_idx, token, tick)` is called on a childless crystallized node:
1. Node classifies token → Different (it's crystallized, token not in spectrum)
2. Phase 1: no children to check → skip
3. Phase 2: no children to recurse → skip  
4. Phase 3: create child → **DEPTH 2 IS BORN**

The recursive routing naturally creates depth by letting each level decide: "this isn't for me, let me create a more specialized child."

## Important: Don't route into Learning nodes in Phase 2

Learning nodes in Phase 2 should be skipped — they're still collecting observations and shouldn't spawn children. Only crystallized nodes that actively say "this is Different from me" should be recursed into.

```rust
// Skip Learning nodes — they handle tokens via Phase 1 (Unknown)
if self.nodes[cidx].state == NodeState::Crystallized {
    if self.route(cidx, token, tick) {
        return true;
    }
}
```

## Also fix in query.rs

Apply the same change to `query_route` — allow descent into childless crystallized nodes:

```rust
// Phase 2: recurse into crystallized children (even if childless)
for &child_id in &node.children {
    let cidx = child_id as usize;
    if self.nodes[cidx].state == NodeState::Crystallized {
        let result = self.query_route(cidx, token);
        if result.1 > self.nodes[cidx].depth {
            return result;
        }
    }
}
```

## Expected Behavior After Fix

Feed curriculum (alphabet → syllables → words → sentences → code):
- Root: depth 0, knows sequential steps
- Depth 1: syllable/word transition families (multiple branches)
- Depth 2: finer distinctions WITHIN each family
- Depth 3+: increasingly specific patterns

The tree should grow both wide AND deep. Different content types should land at different depths and different branches.

## Risk: Unbounded Depth

With this fix, every Different at every level can create a new child. The tree could grow very deep very fast. Monitor with `trie_stats` — if depth exceeds 20 on normal English text, something is wrong.

Natural depth limiting: `params_for_depth` makes deeper nodes crystallize faster (threshold=16 at depth 4+) with smaller spectra (size=4). This means deep nodes are very specific and crystallize quickly, which is correct behavior.

## Tests

1. **Depth growth test:** Feed 1000+ bytes of English text to a fresh trie. Verify at least one node exists at depth 2+.

2. **Depth limit test:** Feed 5000 bytes. Verify max depth is reasonable (< 20).

3. **Branching + depth test:** Feed diverse content (English + code). Verify both width (multiple children at depth 1) AND depth (children at depth 2+).

4. **Query depth test:** Query with English text, then code. Verify different depth profiles.

## After Implementing

1. `cargo build && cargo test`
2. Delete trie-memory.dat and trie-content.json
3. Restart MCP server
4. Run curriculum experiment: alphabet → syllables → words → sentences
5. Check: do nodes exist at depth 2+? Does `trie_stats` show children on depth-1 nodes?

## Verification Log

- 2026-04-19 — `cargo build` clean, `cargo test --test trie_growth`
  reports 31 passed / 0 failed. Key tests passing:
  - `test_routing_produces_depth_growth` — recursive routing creates depth 2+
  - `test_depth_growth_english_text` — English text reaches depth 2+
  - `test_depth_limit_is_reasonable` — depth stays < 20 on 5k bytes
  - `test_diverse_content_grows_depth` — mixed content grows both wide and deep
  - `test_depth_2_nodes_have_correct_parent_chain` — parent links valid

Next: re-run the 2026-04 short-vs-long query experiments on the now-deep
trie before trusting honest-agent Task 03 thresholds.
