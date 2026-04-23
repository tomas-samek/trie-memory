# Session 2026-04-19 — Character-Weight Experiment

## What we were testing

The hypothesis: the whole word/sentence/topic hierarchy should fall out
from a single trie mechanism if we just feed it at the right scale with
a richer input encoding. Letters carry weight; spaces have no weight but
advance time; punctuation = graduated silence. One substrate, scale-free
discovery.

Concretely: change the input encoding, nothing else. Three encodings run
on the same corpus (`tests/fixtures/corpus.md`, 8.6 KB, 12 topics, 2
passes):

- **A — byte-trie**: raw UTF-8 bytes. What the current system uses.
- **B — weight-inline**: `char_weight()` for every char *including*
  spaces and punctuation. Everything is a token.
- **C — weight + silence**: `char_weight()` only for content chars;
  whitespace and punctuation advance `trie.tick` but write nothing.
  Content runs are written per-burst so delta encoding resets between
  runs — matches the user's "each word is a fresh visual event" model.

Harness: `tests/character_weight_experiment.rs`.

## Measurement

### Trie shape after feeding

| Encoding | Total nodes | Max depth | Depth histogram |
|----------|-------------|-----------|-----------------|
| A byte-trie       | 14 | 6 | `[(0,1), (1,2), (2,2), (3,3), (4,2), (5,2), (6,2)]` |
| B weight-inline   |  4 | 2 | `[(0,1), (1,2), (2,1)]` |
| C weight+silence  |  5 | 3 | `[(0,1), (1,2), (2,1), (3,1)]` |

### Topic discrimination across 9 distinct-topic queries

| Encoding | Distinct path-keys / 9 queries |
|----------|--------------------------------|
| A byte           | 4 |
| B weight-inline  | 4 |
| C weight+silence | 3 |

### Japanese query specifically

- A byte:   `d7` — seven-segment path, deeper than any other query.
- B weight: `d1` — root-only.
- C weight+silence: `d1` — root-only.

## What the numbers say

**The weight encoding is strictly less discriminating than raw UTF-8
bytes on this corpus.** Not tied, not close: flatter tree, fewer
distinct path-keys, total collapse on non-Latin scripts.

### Why, concretely

1. **Multi-byte UTF-8 is a free script discriminator.** Raw Japanese
   kana/kanji produce 3-byte sequences whose byte values sit outside
   the ASCII range. Delta encoding on those bytes produces exotic
   values that force trie branching. Czech diacritics do the same at
   2 bytes. The byte-trie's `d7` path for Japanese came from that.

2. **The weight function collapses the non-ASCII signal to 128.**
   `char_weight`'s fallback (`_ => 128`) assigns a single neutral
   weight to every non-ASCII character. So Japanese, Czech accents,
   and Greek all get the same token. The substrate loses the one
   signal it could most easily use to differentiate scripts.

3. **Within a single script (ASCII English), weight encoding is
   flat by design.** Lowercase 200..=225, uppercase 230..=255. All
   consecutive-letter deltas are in ±25. The root spectrum (size 64)
   absorbs everything. This matches CLAUDE.md's Phase 3 note exactly:
   *"root absorbed everything because English letter transitions are
   too uniform."* Silence-as-tick-advance didn't rescue this — time
   doesn't become data in this mechanism.

## Honest read

The one-mechanism claim does not hold at the character scale in this
codebase. Not because the mechanism is wrong in principle, but because:

- **Input encoding alone can't fix it.** The user's intuition ("this
  should only be an input question") was reasonable but the
  experiment says otherwise. Swapping in weight + silence made the
  trie strictly worse at what it already did.

- **The trie mechanism doesn't compose.** Even if we got letter-level
  structure to branch, there's no mechanism that says "this recurring
  letter-pattern is now a unit at the next level." Crystallization
  sets a spectrum and routes within that one node; it doesn't create
  higher-order nodes for repeating sub-patterns. Words won't emerge
  from letters just because we fed letters.

- **The byte-trie is already doing the work we wanted from
  character-weight.** Its script discrimination (Latin / Czech /
  Japanese separating at depth 3–7) is the one thing it does well.
  That's not nothing — but it's a coarse partition, and it's what we
  already had.

## What this implies for the project

The tokenization concern you raised is real, but the fix isn't an
input re-encoding. The real choice is:

1. **Accept the hybrid.** The byte-trie is a script classifier; the
   word-trie is a topic classifier; the coverage gate is a word
   normalizer. None of them is the substrate for all three scales.
   Write this up as the architecture instead of pretending it's one
   mechanism.

2. **Change the mechanism, not the input.** If words should emerge
   from letters, the trie would need some explicit multi-scale
   composition — e.g., "a node that crystallizes becomes an
   available *input token* at the parent's level on the next pass."
   That's a non-trivial redesign of the trie itself, not a tokenizer
   swap. Candidate for a separate project.

3. **Keep the current stack and be narrow about the claim.** The
   honest-agent layer works. The byte-trie-as-substrate part of the
   story doesn't hold up. Publish both findings and stop.

No path here is a rewrite of the current code. The three trees still
compose (byte → script, word → topic, coverage → normalization), and
the honest-agent layer doesn't care what produces the path keys. This
session was a falsification exercise: the one-mechanism claim is the
part of the ontology that didn't survive contact with the corpus.

## Raw output

See `cargo test --test character_weight_experiment -- --nocapture`.
