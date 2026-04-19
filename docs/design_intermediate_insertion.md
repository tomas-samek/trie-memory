# Design: Intermediate Node Insertion — Abstraction Through Grouping

**Status:** Ready for experimental implementation  
**Date:** April 10, 2026  
**Extends:** design_radiation_binding.md

## The Insight

The trie currently only grows **downward** — specialization. But abstraction grows **upward** — discovering that existing nodes share a common property and grouping them under a new intermediate node.

The concept of "letter" doesn't live below 'a' — it lives ABOVE 'a', 'b', 'c' as the shared abstraction that groups them.

## Append-Only Still Holds

Inserting an intermediate node is still append-only:
- A new node is **added** (append)
- Existing nodes' parent pointers are **updated** (reparenting)
- No node is removed, no spectrum is modified, no history is lost

The only mutation is `parent: Some(old_parent)` → `parent: Some(new_intermediate)` on the grouped children. This is "appending a relationship," not deleting structure.

## The Operation: Insert Intermediate

Given a parent and a subset of its children that share a pattern:

```
Before:
parent → A (knows vowel deltas)
parent → B (knows consonant deltas)  
parent → C (knows punctuation deltas)

Observation: A and B both handle "letter-like" deltas

After:
parent → NEW (knows "letter-ness") → A
                                    → B
parent → C (unchanged)
```

What happens:
1. NEW node is created with `parent = parent_id`
2. NEW node's spectrum = what A and B have in common (intersection or shared frequent values)
3. A and B's parent pointers change from `parent_id` to `NEW.id`
4. A and B are removed from parent's children list and added to NEW's children list
5. NEW is added to parent's children list

Parent doesn't change its spectrum. A and B don't change their spectra. C is unaffected.

## Key Constraint: Root Is Always Root

Root (node 0, depth 0) can never be preceded. It IS the big bang. You can't prepend the universe.

But root CAN insert intermediate nodes between itself and its children. This is the only direction abstraction works — inward/downward from any existing node, grouping its children.

This applies at every depth, not just root:

```
Any node X with children [M, N, O]:
X discovers M and N share a pattern →
X inserts NEW between itself and {M, N}
```

## What Triggers Insertion?

A node observes its children's spectra and classifies their similarity:

- **Same across children:** their spectra share significant overlap → insert intermediate grouping them
- **Different:** genuinely distinct spectra → leave as separate branches
- **Unknown:** not enough children or not enough observation → keep watching

The trigger condition (simplest version):
```
If two or more children share >= 50% of their spectrum values:
    → create intermediate node
    → intermediate's spectrum = intersection of children's spectra
    → reparent those children under intermediate
```

## Every Node Is an Agent

There is no external "agent" needed. Every node already does:
- **Consumer:** classifies incoming stream (same/different/unknown)
- **Observer:** can examine its children's spectra for shared patterns
- **Producer:** can insert intermediate nodes when patterns found

The "agent loop" from earlier designs is just this mechanism running at every node, at every level. Not a separate system.

## Depth Recalculation

When an intermediate node is inserted, the grouped children's depth increases by 1. And all THEIR descendants' depths increase by 1. This is a cascading update.

Simplest approach: store depth as computed from parent chain, not as a fixed field. Or: update depths recursively after insertion.

## New MCP Tool: `trie_group`

Expose the intermediate insertion as a tool so it can be triggered externally (by Claude or by internal logic):

**`trie_group`**
- Input: `{ "parent_id": number, "child_ids": [number, number, ...], "label": string (optional) }`
- Validation:
  - parent_id must exist and be crystallized
  - all child_ids must be direct children of parent_id
  - at least 2 children required
- Action:
  1. Compute intermediate spectrum from children's shared values
  2. Create new intermediate node
  3. Reparent specified children under intermediate
  4. Update depths recursively
- Output: `{ "new_node_id": number, "spectrum": [...], "children_grouped": number, "depth": number }`

## New MCP Tool: `trie_suggest_groups`

Read-only analysis: examine a node's children and suggest which ones could be grouped:

**`trie_suggest_groups`**
- Input: `{ "node_id": number, "min_overlap": number (optional, default 0.5) }`
- Action: compare all pairs of children's spectra, find clusters with >= min_overlap shared values
- Output: `{ "suggestions": [{ "child_ids": [...], "shared_spectrum": [...], "overlap_ratio": number }] }`

This allows the agent (Claude or automatic) to first inspect, then decide whether to group.

## Automatic vs Manual Grouping

**Phase 1 (implement now):** Manual grouping only via `trie_group` and `trie_suggest_groups`. Let the agent (Claude) decide when to group based on `trie_suggest_groups` output.

**Phase 2 (later):** Automatic grouping where each node periodically examines its children and groups when overlap exceeds threshold. This is the "every node is an agent" version.

Start with Phase 1 — we want to observe what grouping does before automating it.

## Example: Curriculum Experiment

Feed alphabet → syllables → words → sentences. After the alphabet phase:

```
root → A (knows delta 129 = sequential step)
root → B (knows delta 128 = same repeated)
root → C (knows other deltas)
```

`trie_suggest_groups(root)` might show A and B share common values. `trie_group(root, [A, B])` creates:

```
root → NEW ("small deltas — sequential data") → A
                                                → B
root → C (unchanged)
```

NEW represents "the concept of sequential text" — an abstraction above specific delta patterns.

## Connection to Theory

- RAW 113: Same at a higher level = gravity between siblings (they share a property)
- "Neurons that fire together wire together" = children with similar spectra get grouped
- The hierarchy `something exists → sequential → character set → specific letter` emerges through repeated grouping
- Each grouping is an act of abstraction — discovering shared properties in existing structure

## What Changes in Existing Code

### Node struct
- No changes to spectrum, classify, observe, consume, crystallize

### Trie struct — new methods:
- `suggest_groups(node_id, min_overlap) → Vec<GroupSuggestion>`
- `insert_intermediate(parent_id, child_ids) → new_node_id`
- `recalculate_depths(node_id)` — recursive depth update after insertion

### MCP tools — add:
- `trie_group` 
- `trie_suggest_groups`

### Existing functionality — unchanged:
- Write path, read path, query, perceive, perceive_window
- Remember/recall
- Snapshot/restore (intermediate nodes are just regular nodes with children)

---

*Conversation: April 10, 2026*
*References: RAW 113, session_2026_04_08.md, design_radiation_binding.md*
