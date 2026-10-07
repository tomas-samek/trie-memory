# trie-memory

An append-only, integer-only hierarchical memory exposed as an MCP server,
with a layered "honest-agent" retrieval stack built on top.

**Status:** research PoC. Five end-to-end scenarios pass (`tests/honest_agent_*`).
Some architectural claims hold, some don't — see [What's honest about the
current state](#whats-honest-about-the-current-state) before building on this.

🌐 **More projects and writing:** [tomas-samek.github.io](https://tomas-samek.github.io/)

## What it is

Two things, layered:

1. **A classifying trie.** Every node answers `Same` / `Different` / `Unknown`
   for an incoming token. Same = consume, increment visit. Different = route
   to a child. Unknown = observe and eventually crystallize. No floats, no
   gradients, no loss. The structure is the knowledge.

2. **An "honest agent" retrieval stack** on top. Memories are stored with
   provenance (who told me, what source, how much to trust), indexed by
   content-addressable path keys, and retrieved through a mode selector
   that distinguishes five epistemic states:

   - `Answer` — high coverage with a stored memory, single clear match.
   - `Partial` — some coverage, but not confident.
   - `Disambiguate` — multiple candidates tied, ask the user.
   - `Conflicted` — an explicit user correction supersedes a prior memory;
     both are shown.
   - `Unknown` — no meaningful recall; never fabricate.

The mode selector consumes a multi-dimensional `ConfidenceVector`
(signal strength, clarity, coverage, source trust, recency,
legacy-origin, contradiction flag), not a scalar score.

## Core principles

1. **Change detection, not values.** Input is a delta stream.
2. **Same / Different / Unknown are classifications, not numbers.**
3. **Append-only.** The trie grows; memories are never deleted. A correction
   stamps the target's `revised_by` field — the original remains.
4. **Integer scales only.** Trust, coverage, clarity, recency are all ppm
   integers (0..=1000). No floats in the honest-agent pipeline.
5. **No fabrication.** The renderer is template-based; content for
   `Answer` / `Partial` / `Conflicted` comes verbatim from stored memories.
   `Unknown` admits ignorance.
6. **Never silently hide a correction.** `Conflicted` fires before
   path-fall-through rules, so even weak trie routing can't bury a
   revision.
7. **Provenance is load-bearing.** Every deposit carries `observer_id`,
   `source_type`, `trust_level`, `origin` — and the renderer surfaces
   low-trust tags to the user.

## Architecture

```
 MCP client (Claude / stdio / SSE)
            │
            ▼
 ┌────────────────────────────────────┐
 │  Mode selector (Task 05 cascade)   │
 │   Answer / Partial / Disambiguate  │
 │   / Conflicted / Unknown           │
 └──────────────┬─────────────────────┘
                │  ConfidenceVector
 ┌──────────────▼─────────────────────┐
 │  Responder (template renderer,     │
 │  no fabrication)                   │
 └──────────────┬─────────────────────┘
                │
 ┌──────────────▼─────────────────────┐
 │  ContextWindow (per-session        │
 │  rolling exchanges + hot concepts) │
 └──────────────┬─────────────────────┘
                │
 ┌──────────────▼─────────────────────┐
 │  ContentStore (path-indexed,       │
 │  append-only, provenance per       │
 │  entry, revision links)            │
 └─────┬──────────────────┬───────────┘
       │                  │
 ┌─────▼──────┐     ┌─────▼────────┐
 │ byte-trie  │     │  word-trie   │
 │ (script    │     │ (per-word    │
 │  class)    │     │  topic class)│
 └────────────┘     └──────────────┘
```

Both tries share the same `Trie` implementation; they differ only in how
tokens are constructed from input (bytes vs FNV-1a-hashed word tokens
with variable tick gaps for punctuation).

## Layout

```
src/
├── main.rs                — stdio + SSE transport entry points
├── lib.rs
├── trie/                  — byte / word trie core
│   ├── mod.rs             — Trie, stats, path_key (content-addressable)
│   ├── node.rs            — Node, classify, observe, consume, crystallize
│   ├── write.rs           — delta encoding + route() with maturity gate
│   ├── read.rs            — leaf → root walk
│   ├── query.rs           — read-only traversal
│   ├── perceive.rs        — batch multi-leaf read
│   ├── persistence.rs     — bincode snapshot / restore
│   ├── grouping.rs        — spectrum overlap, intermediate insertion
│   └── tokenizer.rs       — split_words, word_token, tokenize_with_silence
├── store/
│   ├── mod.rs             — ContentStore (path-hash indexed, dedup by id)
│   ├── memory.rs          — MemoryEntry + SourceType + Origin + revised_by
│   ├── concept.rs         — ConceptStore (cross-lingual binding)
│   └── layer.rs           — MemoryLayer (knowledge-commit history)
└── mcp/
    ├── tools.rs           — MCP tool schema + dispatch
    ├── context.rs         — ContextWindow (rolling buffer, hot concepts)
    ├── responder.rs       — Mode selector + renderer + confidence vector
    ├── dispatch.rs, sse.rs, transport.rs
tests/
├── trie_basic.rs, trie_growth.rs, trie_grouping.rs
├── word_trie.rs, word_token_trie.rs
├── concept_binding.rs, layer_test.rs, mcp_integration.rs
├── honest_agent_schema.rs        — Task 01
├── honest_agent_context.rs       — Task 03
├── honest_agent_responder.rs     — Task 06 (+ coverage)
├── honest_agent_confidence.rs    — Task 04 (+ cascade)
├── honest_agent_scenarios.rs     — Scenarios 1/2/3
├── honest_agent_paraphrase.rs    — Scenario 4 (graded 10/10)
├── honest_agent_correction.rs    — Scenario 5
├── phase_a_exercise.rs           — realistic run-through
├── phase_b_stress.rs             — 20-query stress (19/20, 0 hard fail)
└── prereq_experiment.rs          — trie shape baseline
docs/
├── design/honest_agent/          — architecture, tasks 01–08, progress
├── session_2026_04_*.md          — session notes with raw findings
└── theory.md, design_*.md        — background notes
TASK.md                           — depth-growth route() fix (shipped)
CLAUDE.md                         — implementation task log (phases 2–4)
```

## MCP tools

### Trie

| Tool | Description |
|------|-------------|
| `trie_write` | Feed a string into both tries. |
| `trie_query` | Read-only probe; returns `deepest_node`, `match_depth`, `depth_profile`. |
| `trie_read` | Walk a leaf → root and return node summaries. |
| `trie_stats` | Per-node: visit count, depth, children count, spectrum, state. |
| `trie_tokenize` | Show how the word tokenizer splits + assigns silence gaps. |
| `trie_perceive` / `trie_perceive_window` | Recent-activation view. |
| `trie_path_key` | Content-addressable path key (byte or word trie). |
| `trie_suggest_groups` / `trie_group` | Spectrum-overlap-based intermediate node insertion. |
| `trie_snapshot` / `trie_restore` | Persist all stores. |

### Memory (honest agent)

| Tool | Description |
|------|-------------|
| `trie_remember` | Store content with optional provenance: `observer_id`, `session_id`, `stream_id`, `source_type` (user_direct / user_correction / web_fetched / …), `trust_level` (0–1000), `origin.corrects`, `modality`, `language`. |
| `trie_recall` | Path-indexed retrieval; returns the memory with full provenance fields. |
| `trie_ask` | The honest-agent entry point: enrich → route → recall → mode-select → render in one call. Returns `mode`, `response`, `supporting`, `confidence`, `reasoning`. |

### Concepts, layers, context

| Tool | Description |
|------|-------------|
| `concept_create`, `concept_bind`, `concept_lookup`, `concept_bind_auto` | Temporal co-occurrence binding across surface forms. |
| `concept_snapshot` / `concept_restore` | Concept persistence. |
| `layer_begin` / `layer_commit` / `layer_list` / `layer_info` | Memory layers — a bookmark on the tick timeline marking a body of learned knowledge. |
| `context_show` / `context_reset` | Rolling exchange buffer + hot concepts. |

## Running

```
cargo build --release
cargo run --release                        # stdio MCP server
cargo run --release -- --sse --port 3001   # SSE transport
cargo test                                 # full suite (163 tests)
```

Storage defaults live in `W:/data/trie-store/` on the author's machine
(see `DEFAULT_*_PATH` consts in `src/main.rs`); adjust to taste.

## Tests

```
cargo test --test honest_agent_scenarios   # Task 08 scenarios 1, 2, 3
cargo test --test honest_agent_paraphrase -- --nocapture   # Scenario 4 (graded)
cargo test --test honest_agent_correction  # Scenario 5
cargo test --test phase_a_exercise -- --nocapture          # realistic run
cargo test --test phase_b_stress -- --nocapture            # 20-query stress
cargo test --test prereq_experiment -- --nocapture         # baseline trie shape
```

Current status: **163 / 163 pass across 19 test files.** Stress: 19/20
mode-pass, 0 hard fails.

## What's honest about the current state

The PoC works, but not all of the original architecture's claims hold
up equally. What the testing actually showed:

**Strong:**
- No-hallucination-on-unknown is reliable (Scenario 1, stress U category).
- Literal and near-literal recall works (Scenarios 2, 4A/B).
- Cross-session persistence is clean (Scenario 3).
- Correction handling is append-only and can't be gaslit (Scenario 5).
- The five-mode epistemic split produces genuinely different responses
  for genuinely different uncertainty states.

**Weaker than the design implied:**
- The byte-trie does less than Phase 1 suggested. On English prose it
  flattens; the path key acts as a coarse language-family partition
  rather than a topical index. The real topic discrimination happens
  at the word-level coverage gate, not at trie resonance.
- "No embeddings" is true in the ML sense, but the coverage computation
  uses a small English preprocessor (stopwords, trailing-s stemming,
  punctuation strip). That's a linguistic prior, not just a threshold.
  Pull those three normalizations and Scenario 4 drops from 10/10 to
  7/10.
- "Universal tokenizer across modalities" is aspirational — only text
  was tested. See `docs/design/honest_agent/tasks/02_universal_tokenizer.md`
  for the known risk.
- One architectural failure in the stress test (`[X] tides vs
  semaphore`) is unresolved: distinctive content words route to a
  different trie subtree than standard-prose memories, so path-suffix
  matching misses. Not fixable without revisiting indexing.

**Deferred / not-in-scope:**
- `STALE` mode (needs a domain-volatility classifier) — [#8](https://github.com/tomas-samek/trie-memory/issues/8).
- Multi-observer flows — [#10](https://github.com/tomas-samek/trie-memory/issues/10).
- Real embedding layer for deep semantic paraphrase — [#12](https://github.com/tomas-samek/trie-memory/issues/12).
- LLM-based renderer (template-only for now) — [#7](https://github.com/tomas-samek/trie-memory/issues/7).
- Baseline comparison against LLM + vector DB — [#5](https://github.com/tomas-samek/trie-memory/issues/5).

For the full story including the three bugs the stress test caught and
the threshold tuning, see
[`docs/design/honest_agent/progress.md`](docs/design/honest_agent/progress.md)
and the 2026-04 session notes under `docs/`.

## Roadmap

Post-PoC work is tracked in GitHub milestones:

- **[M1 — PoC robustness](https://github.com/tomas-samek/trie-memory/milestone/1)** — fixes for known limits surfaced by stress testing. Indexing bug on distinctive content words ([#1](https://github.com/tomas-samek/trie-memory/issues/1)), mode-selector threshold robustness ([#2](https://github.com/tomas-samek/trie-memory/issues/2)), and the English-biased coverage signal ([#3](https://github.com/tomas-samek/trie-memory/issues/3)).
- **[M2 — Honesty & validation](https://github.com/tomas-samek/trie-memory/milestone/2)** — close the gap between architectural claims and what's actually tested. Universal-tokenizer second modality ([#4](https://github.com/tomas-samek/trie-memory/issues/4)), LLM + vector-DB baseline comparison ([#5](https://github.com/tomas-samek/trie-memory/issues/5)), non-English paraphrase scenario ([#6](https://github.com/tomas-samek/trie-memory/issues/6)).
- **[M3 — Post-PoC (Phase D)](https://github.com/tomas-samek/trie-memory/milestone/3)** — features explicitly deferred; see issues [#7–#14](https://github.com/tomas-samek/trie-memory/issues?q=is%3Aissue+milestone%3A%22M3+%E2%80%94+Post-PoC+%28Phase+D%29%22).

## Theoretical background

The ternary classifier draws on tick-frame-space research:

- **RAW 113** — Semantic isomorphism: Same / Different / Unknown.
- **RAW 123** — The stream, the trie, and what the data tells us.
- **RAW 112** — The single mechanism.

The honest-agent layer is a separate, lower-stakes PoC that sits on top
of the tick-frame substrate without depending on its full ontology
being validated.

## License

CC BY-NC 4.0 — research, academic, and educational use. Commercial use
requires permission.
