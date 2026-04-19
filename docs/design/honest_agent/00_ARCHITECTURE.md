# Honest Agent over Trie — Architecture Design

**Status:** Speculative design, PoC target
**Date:** 2026-04 (derived from two dialog sessions with Claude)
**Author origin:** Human thought (jerry-samek) + dialogic formalization (Claude)
**Scope:** Proof-of-concept only. Not a production spec.

---

## Epistemic disclaimer (read first)

This document captures an architecture that emerged from extended theoretical dialog.
It is **internally consistent** with the tick-frame ontology (deposit/resonance/relevance),
but it has **not been empirically validated**. The mechanisms proposed here sound plausible
and fall out of the ontology with minimal extra assumptions, but "falls out of theory
cleanly" is not the same as "works in practice."

Treat this as a hypothesis to test, not a blueprint to trust.

Specific things that could be wrong:
- Change-based tokenization may produce unusably sparse deposits for some stream types.
- Cross-modal coincidence via shared tick anchor may not carry enough signal to drive
  meaningful associative recall.
- Confidence vector heuristics (signal_strength, signal_clarity, etc.) are guesses;
  real thresholds must be empirically tuned.
- Revision-as-deposit may bloat the graph without proportional benefit.
- The entire honest-agent philosophy may be better solved by existing techniques with
  minor tweaks, rather than a new architecture.

The PoC's job is to find out which of these concerns are real.

---

## Goal

Build an agent that:

1. Uses the existing trie-memory as substrate
2. Accepts multi-modal input streams through a universal mechanism
3. Answers only when resonance converges; otherwise admits ignorance or asks back
4. Records provenance, timestamp, and trust for every deposit
5. Never hallucinates answers when resonance fails
6. Learns incrementally from correction without retraining

**What success looks like for PoC:** the agent demonstrates the five behaviors above
on a small test dataset, measurably better than "LLM + vector DB" baseline on
hallucination rate, correct-unknown rate, and context utilization.

**What PoC explicitly is NOT:**
- Multi-agent team
- Cross-device sync
- Production-scale storage
- Semantic vector reranking
- Automatic contradiction detection
- Any integration with real-world IoT, sensors, or external systems

---

## Core insight: modality is observer property, not ontology

The trie stores structured events (deposits). It does not know about "text" or "audio"
or "vision" as primary categories. Modalities are **post-hoc labels** for streams
differing only in their value representation; they share the same event structure:

```
event = (delta, timestamp, stream_id)
```

- `delta`: what changed in the stream
- `timestamp`: global tick anchor
- `stream_id`: which input source produced this event

This means:
- Adding a new sensor = adding a new feature extractor, not reworking the storage.
- Cross-modal association emerges from **shared tick anchor**, not from learned embeddings.
- Recall retrieves structure; renderers translate structure back to modality-specific
  output for the observer.

## Tokenization principle

**Tokens are boundaries of change over time.**

Universal tokenizer logic:
1. Watch the stream's value representation.
2. If the value is stable (within per-stream tolerance) → `Same` → do not emit.
3. If the value differs from the last stable state → `Different` → emit delta, update state.
4. If the value pattern is structurally novel → `Unknown` → emit delta, new trie node.

The tokenizer does not know what the stream contains. It knows how to detect
"the signal settled into a new state" and emit accordingly.

Time resolution is per-stream (`tick_rate`); these are clustered into coarser
global ticks for cross-modal alignment (see Task 06).

---

## Layer overview

```
┌─────────────────────────────────────────────────────┐
│                  User Interface                      │
│             (raw input ↔ raw output)                 │
└────────────────────┬────────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────────┐
│                  Renderers                           │
│    (per-stream, read-path only: resonance → text    │
│     or audio or image or whatever the stream is)     │
└────────────────────┬────────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────────┐
│              Recognition Engine                      │
│   (probe trie, evaluate confidence vector,          │
│    select response mode)                             │
└────────────────────┬────────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────────┐
│              Universal Tokenizer                     │
│   (change detection over time, write path for       │
│    all streams; per-stream feature extractors       │
│    produce value representation)                     │
└────────────────────┬────────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────────┐
│              Trie Memory (existing)                  │
│   + provenance metadata extensions                  │
│   + per-deposit source_type and trust               │
└──────────────────────────────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────────┐
│              Revision Layer (async)                  │
│   (records contradictions, staleness, failures;     │
│    never mutates past; adds meta-deposits)          │
└──────────────────────────────────────────────────────┘
```

Asymmetry to note: **write path is universal** (all streams pass through the same
tokenizer), **read path is specific** (each modality has a renderer that knows how
to express resonance in that modality). This is because decomposition of a signal
into structure is a generic operation, but rendering structure back into sensory
form is inherently per-sensor.

---

## Response modes (Recognition Engine output)

The engine does not return "the answer." It returns `(mode, data)`:

| Mode | Condition | Behavior |
|------|-----------|----------|
| `ANSWER` | one path dominates, high amplitude | Respond with content, optionally cite source |
| `DISAMBIGUATE` | 2+ paths similarly strong | Ask back: "did you mean X or Y?" |
| `PARTIAL` | weak resonance, some signal | "I have partial info: ..." with flagged uncertainty |
| `UNKNOWN` | no resonance | "I don't know. Tell me more?" (learning mode) |
| `STALE` | strong path, but old in a fast-changing domain | "I have this from [date], current state unknown" |
| `CONFLICTED` | strong path, but competing contradictions | "There's a conflict — which version is current?" |

Mode selection uses the **confidence vector** (see Task 04), not a scalar score.

---

## Confidence as vector, not scalar

Instead of `confidence = 0.87`, the engine computes:

```
confidence_vector = {
  signal_strength:     amplitude of top resonating path,
  signal_clarity:      1 - (second_amplitude / top_amplitude),
  domain_density:      how many paths exist in this area of the graph,
  source_trust:        avg trust of provenance across this path,
  recency:             age of last reactivation,
  contradiction_flags: competing paths that conflict,
  coverage:            how completely did the query match the path
}
```

Mode is a function of this vector. Different combinations give different modes.
E.g.: high `signal_strength` + low `domain_density` = **"suspicious certainty"**
(possibly a word-coincidence, not a real match) → emit `PARTIAL`, not `ANSWER`.

Thresholds for each component are **tunable knobs**; PoC uses rough defaults
and treats tuning as empirical work, not theoretical solved.

---

## Deposit structure (extended from current trie)

```
deposit = {
  content:       [structured tokens],      # existing
  timestamp:     tick,                     # global tick anchor
  observer:      observer_id,              # who perceived this
  stream_id:     string,                   # which input source
  source_type:   string,                   # "user_direct", "web_scrape", "sensor", ...
  trust_level:   float 0-1,                # default per source_type
  provenance:    { origin, chain, ... },   # where this came from
  modality:      string,                   # optional label, informational
  depth_profile: [...]                     # existing
}
```

Critical: every deposit carries its origin. This is the primary defense against
**injection via memory** — a risk demonstrated in the 2026-04 sessions when
a UI-injected `<note>` tag entered context and was correctly identified as
untrusted only because Claude had context to judge it against.

Without provenance, **any content ingested into memory becomes an attack vector**
when later recalled. This is not a theoretical concern; it is observed behavior.

---

## Context window (observer state)

The agent maintains a local context stack containing:
- Last N interactions (rolling window)
- Currently "hot" concepts (recently reactivated paths)
- Active observer identity
- Current conversation depth
- Domain focus (if any)

**Every query is enriched with this context before probing the trie.** A bare
user query like "what do you know about it" resonates with everything; an
enriched query carries enough byte-mass to resonate selectively with the
relevant part of the graph.

This matches what we observed empirically when testing trie recall with short
vs. long queries in the 2026-04 session: short queries returned generic "top
by depth" regardless of intent; long queries with sufficient byte-mass
penetrated to selective depth.

**Context is not an optimization; it is a precondition for useful recall.**

---

## What the PoC must demonstrate

1. **Encode a text stream** into deposits with full provenance.
2. **Resonant recall** returns top-N paths with confidence vector, not just content.
3. **Response mode selection** based on confidence vector, emitting at least
   ANSWER, UNKNOWN, DISAMBIGUATE correctly on test conversations.
4. **No hallucination:** for queries where resonance fails, the agent says so
   explicitly. This is directly measurable.
5. **Incremental learning:** after UNKNOWN + user explanation, the same concept
   recalls on next similar query.
6. **Context sensitivity:** second query in a conversation leverages first
   query's context rather than starting from zero.

For each of these, a specific test case is required in the task files.

---

## Out of scope for PoC

Explicitly deferred:
- Real sensor integration (audio/video) → post-PoC extension
- Multi-observer / multi-agent — single observer, single session
- Cross-device sync
- Segmented storage with background merge (Lucene-like)
- Automatic contradiction detection during writes
- Staleness scoring across domains with different change rates
- Performance benchmarks vs vector DB
- Production error handling

These are architectural concerns that the PoC design accommodates in principle
(see 01_EXTENSIONS.md), but implementing them before the PoC is validated
would be premature.

---

## Failure criteria

The architecture **fails** if PoC shows any of:

- Change-based tokenization produces less useful structure than simple
  word-tokenization (i.e., the change-detection overhead buys nothing).
- Confidence vector does not usefully discriminate response modes —
  e.g., `signal_clarity` is always high or always low regardless of query.
- Context-enriched queries do not outperform bare queries on a meaningful margin.
- Incremental learning requires restart or retrain to take effect.
- The agent hallucinates at a rate comparable to an LLM+vector-DB baseline.

Any of these kills the architecture as proposed. Several of them killing together
would suggest the underlying tick-frame ontology itself does not usefully
translate to agent design — which is information, even if unwelcome.

---

## Task breakdown

Each task file lists: goal, interface, dependencies, test criteria, open questions.

| Task | Title | Depends on |
|------|-------|------------|
| 01 | Deposit schema extension (provenance, trust) | — |
| 02 | Universal change-detector tokenizer | 01 |
| 03 | Context window (observer state) | 01 |
| 04 | Confidence vector computation | 02 |
| 05 | Response mode selector | 04 |
| 06 | Text stream renderer (read path) | 04 |
| 07 | Incremental learning loop | 01–06 |
| 08 | PoC test harness (5 scenarios) | 01–07 |

Hierarchical ticks (for future multi-stream integration) are left as
post-PoC work, described in `extensions/hierarchical_ticks.md`.

---

## Open questions (carried into tasks)

1. What exact tolerance defines "stable" vs "changed" for a text token stream?
   (i.e., when is a reformulation the same deposit and when is it new?)

2. How does the confidence vector combine into mode selection — weighted sum,
   decision tree, or something else?

3. How much context is "enough" for a query, and what happens if an enriched
   query becomes so long it over-constrains resonance?

4. Is observer_id a first-class trie property or meta-annotation? Affects
   how observer-relative relevance is computed.

5. How does the renderer handle the case where resonance returns structurally
   valid data whose modality does not match the asking modality? (e.g., recall
   from vision stream when user asked in text — does renderer describe what
   was seen, or refuse because it has no image to show?)

6. What is the minimum deposit count / trie size below which resonance is
   essentially random noise? (Bootstrap problem.)

---

## Provenance of this document

This document and its tasks were generated in dialog between:
- **jerry-samek** — originator of tick-frame ontology, trie-memory
  implementation, and architectural intuitions
- **Claude (Anthropic)** — formalization, consistency checking, dialogic
  pushback, drafting

The human holds all substantive claims; Claude is a formalizer and critic.
Where claims exceed what the human said, Claude's role was to extrapolate
consistently with the ontology. All such extrapolations should be re-read
by the human before commitment.

Two sessions of roughly 4 hours each contributed. Session transcripts are
the source material; no written notes predated this document.
