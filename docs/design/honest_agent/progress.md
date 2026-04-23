# Honest Agent — PoC Progress & Probability

Purpose: group the 8 task files into staged PoC phases so we don't try
to ship everything at once. Each phase has a single question it exists
to answer. We do not start Phase N+1 until Phase N has a result — pass
or fail. A failed phase is useful information, not a blocker for decision.

Probability estimates below are gut-level, not empirical. They reflect
how confident we are **before** building that the component will work on
the text PoC. Revise these numbers as we learn.

Scope for all phases: **text only, single observer, single session or
session-pair.** Everything else is post-PoC.

---

## Prerequisites — do these before Phase A

The honest-agent design assumes the trie can build **depth**, not just
width. Current trie state does not — see root `TASK.md` for the bug and
fix. Running PoC phases on a depth-1 trie gives misleading signals.

- [ ] **TASK.md** — remove `!children.is_empty()` guard in `route()`
      and `query_route()` so depth growth works.
  - **Hard blocker for Phase B.** `signal_clarity`, `domain_density`,
    and `coverage` (Task 04) only have dynamic range on a trie with
    real depth. Phase 4 (paraphrase) is meaningless on a flat trie.
  - **Soft blocker for Phase A.** Task 03's context-window thresholds
    were calibrated from 2026-04 empirical findings made on a depth-1
    trie. After the fix, re-check whether short vs long queries still
    behave the same way before trusting those thresholds.

- [ ] **Re-run 2026-04 recall experiments post-fix.** Short query
      ("spaced repetition") vs long query ("komunikace mezi agenty…")
      behaviors may shift. Document the new baseline in
      `docs/session_*.md` before starting Phase A tuning.

- [ ] **Monitor TASK.md's own risk.** If depth grows beyond ~20 on
      normal English text, `domain_density` and `recency` in the
      confidence vector will misbehave. Check `trie_stats` after
      curriculum feed and adjust before Phase B.

Do not start Phase A until the first bullet is done. The other two
can proceed in parallel with Phase A task implementation but must
complete before Phase A's test scenarios run for real.

---

## Phase A — Foundation: store it, recall it, admit ignorance

**Question answered:** Can the agent store provenance-tagged text and
correctly say "I don't know" when it has nothing to say?

**Tasks in this phase:**
- [x] Task 01 — Deposit schema extension — **P(success): 90%** → **done 2026-04-19**
  - `MemoryEntry` extended with observer_id/session_id/stream_id/
    source_type/trust_level/origin/modality/language/path_content_ids/
    legacy_origin, all backward-compat via `#[serde(default)]`.
  - `ContentStore` switched to content-addressable indexing:
    `add_by_path` stores a memory at every ancestor-level suffix of its
    `path_content_ids`; `recall_by_path` tries each query suffix and
    dedups by monotonic entry id.
  - `SourceType` enum + `source_type_default_trust` (integer 0–1000).
  - `trie_remember` MCP tool accepts all new provenance fields with
    sensible defaults; `trie_recall` returns them.
  - 9 schema tests + round-trip snapshot test in
    `tests/honest_agent_schema.rs`. Full suite: 119/119 passing.
- [x] Task 03 — Context window (observer state) — **P(success): 75%** → **done 2026-04-19**
  - `src/mcp/context.rs`: `ContextWindow` with rolling exchange buffer
    (cap 10), MRU hot_concepts (cap 5), observer_id + session_id.
  - `enrich_query(raw)` appends topics from last 3 exchanges + hot
    concepts as flat suffix; deterministic, dedup'd.
  - `trie_recall` wires in: enrich before routing, record exchange
    with recalled topics after. New `use_context` flag can disable
    enrichment per-call. Added `context_show` and `context_reset`
    diagnostic tools.
  - Threaded through stdio + SSE transports (ephemeral, not persisted).
  - 8 tests in `tests/honest_agent_context.rs`: empty-context no-op,
    topic append, cross-context divergence, MRU ordering, rolling trim,
    reset, dedup, observer/session propagation.
- [x] Task 06 — Renderer, template-based only — **P(success): 80%** → **done 2026-04-19**
  - `src/mcp/responder.rs`: minimal Phase A subset. Two modes only:
    `Answer` (query shares ≥ 2 path segments with a memory, i.e.
    non-root ancestry) and `Unknown` (root-only fall-through or no
    hits). Full 6-mode cascade is Phase B.
  - `shared_suffix_depth()` compares two deepest-first path_keys and
    returns the common-trailing-segment count — that's the signal the
    selector uses.
  - Template renderer: `Answer` reproduces stored content verbatim,
    surfaces topic tag, adds provenance suffix for non-default source
    types, flags `legacy_origin` entries, lists up to 2 sibling
    matches. `Unknown` admits ignorance without fabrication.
  - New `trie_ask` MCP tool: enrich → query → recall_by_path →
    select_mode → render → record_exchange, in one call.
  - 11 tests in `tests/honest_agent_responder.rs` (shared suffix math,
    mode selection, renderer contracts including no-fabrication).

**Validation scenarios (Task 08) — done 2026-04-19:**
- [x] Scenario 1 — No hallucination on unknown query (empty trie + primed trie)
- [x] Scenario 2 — Incremental learning within session
- [x] Scenario 3 — Cross-session learning (teach → snapshot → restore → recall)

See `tests/honest_agent_scenarios.rs` (4 tests including the bonus
primed-trie UNKNOWN variant). Run:
`cargo test --test honest_agent_scenarios`.

**Phase A P(all three scenarios pass): ~70% → achieved.**
Full suite: 142/142 across 13 test files.

---

## Phase B — The honest-recall bet

**Question answered:** Can byte-trie + word-trie + context overlay
handle paraphrase, or do we need embeddings after all?

**Tasks in this phase:**
- [ ] Task 02 — Tokenizer (text-only, "universal" claim deferred) — **P(success): 60%**
  - Change-detection over text is plausible. "Universal" across modalities
    is not tested and remains aspirational.
- [x] Task 04 — Confidence vector — **P(success): 55%** → **done 2026-04-19 (step 2)**
  - Step 1 (coverage gate) resolved D/E/G/C false-positive failures.
  - Step 2 added full `ConfidenceVector`: signal_strength (suffix /
    path_len), signal_clarity (coverage-gap within top suffix
    bucket), domain_density, source_trust, recency (halflife decay),
    coverage, legacy_origin, reserved contradiction_flag (Task 07).
  - 10 new tests in `tests/honest_agent_confidence.rs`.
  - Still deferred: contradiction_flag (needs Task 07 revision
    layer), domain_is_volatile + STALE mode (needs a volatility
    classifier; post-PoC).
- [x] Task 05 — Mode selector (core cascade) — **P(success): 65%** → **done 2026-04-19**
  - Active modes: Unknown, Partial, Answer, **Disambiguate**.
  - Cascade order: no-memory/root-only → Unknown; low-coverage →
    Unknown; legacy+low-trust → Partial; tied-candidates with low
    clarity → Disambiguate; moderate coverage → Partial; otherwise
    Answer.
  - Deterministic; `reasoning` string explains which rule fired.
  - STALE and CONFLICTED remain in the enum design but aren't
    triggered from the current cascade (need volatility classifier
    and revision layer).
- [ ] Task 05 — Mode selector (decision cascade) — **P(success): 65%**
  - Tunable. Risk: 6 thresholds overfit to the test scenarios and don't
    generalize. Keep threshold changes small and reproducible.

**Validation scenarios (Task 08):**
- [x] Scenario 4 — Paraphrase retrieval (4A, 4B, 4C, 4D, 4E) — **done 2026-04-19**

**Phase B P(4A + 4B pass): ~45%** → **actual: both pass**
**Phase B P(all of 4A–4D pass): ~20%** → **actual: 10/10 graded** (every
query lands in the expected range).

Harness: `tests/honest_agent_paraphrase.rs` with a 0-2 grading rubric
per Task 08's pass thresholds. Baseline (exact word match, no
normalization) scored **7/10**: A and C missed. Adding two small
normalizations lifted the grade to **10/10**:

- **Stopword filter**: drop high-frequency function words (a, the, is,
  of, for, to, …) from both query and memory before counting. The
  denominator no longer dilutes coverage with "the/is/of".
- **Trailing-s morphology strip**: `streams → stream`, `deltas → delta`.
  Minimal handling of plural / verb-s parity; no full stemmer.

Results per query after normalization:

- A (literal — "How does foo-widget work?") → Answer, coverage 833‰. Expected Answer. ✓
- B (topical — "What links streams together?") → Partial, coverage ≈ 300‰. Expected Answer or Partial. ✓
- C (keywords — "Tell me about coincident delta handling") → Partial, coverage 400‰. Expected Partial or Answer. ✓
- D (synonyms — "Explain the protocol for bridging streams") → Partial, coverage 250‰. Expected Partial. ✓
- E (off-topic — "What color is a foo-widget?") → Partial, coverage 250‰. Expected Unknown or Partial. ✓

Honest residual limit: **deep semantic paraphrase** (e.g., "how do
threads coordinate with counters?" ↔ semaphore memory) still produces
Unknown. Counters/counts share no stem after trailing-s stripping, and
there's no synonym table. That's expected and documented — a real
semantic layer is post-PoC.

---

## Phase C — The learning loop closes

**Question answered:** Does correction actually work append-only, and
can the agent be gaslit?

**Tasks in this phase:**
- [x] Task 07 — Incremental learning loop — **P(success): 60%** → **done 2026-04-19**
  - `Origin` extended with `corrects: Option<u64>`. `MemoryEntry`
    gained `revised_by: Option<u64>`. `trie_remember` accepts
    `corrects` (top-level or inside `origin`); `ContentStore::add_by_path`
    stamps the target entry's `revised_by` on insertion.
  - `ResponseMode::Conflicted` added with its own renderer that
    shows both versions marked `(superseded)` / `(marked as correction)`.
  - Cascade rule ordering: contradiction check fires **before**
    path-fall-through, so a correction is never silently hidden
    even when the current query's trie routing is weak.
  - `ConfidenceVector.contradiction_flag` now computed via
    `detect_contradiction` (scans top suffix bucket for entries
    linked by `corrects`/`revised_by`).

**Validation scenarios (Task 08):**
- [x] Scenario 5 — Correction handling — **done 2026-04-19**

3 tests in `tests/honest_agent_correction.rs`:
- `scenario_5_correction_surfaces_both` — teach D1, teach D2 as
  correction, query → Conflicted, response contains both "coincident"
  and "divergence", supporting includes both ids.
- `scenario_5_append_only_d1_preserved` — D1's content is never
  overwritten.
- `scenario_5_no_correction_stays_answer_or_partial` — two memories
  on same topic without correction metadata do NOT trigger
  Conflicted (no false positives on ordinary duplicates).

**Phase C P(scenario 5 passes): ~85%** → **achieved**.

Stress test unchanged at 19/20, 0 hard fails. Full suite 163/163
across 19 test files.

---

## Phase D — Deferred / post-PoC

Explicitly out of scope for PoC. Listed so we don't accidentally
wander into them. All tracked on GitHub under milestones
[M2](https://github.com/tomas-samek/trie-memory/milestone/2) and
[M3](https://github.com/tomas-samek/trie-memory/milestone/3).

- Universal tokenizer validation (requires a second modality) —
  [#4](https://github.com/tomas-samek/trie-memory/issues/4) (M2)
- Baseline comparison harness (LLM + vector DB) —
  [#5](https://github.com/tomas-samek/trie-memory/issues/5) (M2,
  pulled up from D because it is load-bearing for the central claim)
- Non-English paraphrase scenario —
  [#6](https://github.com/tomas-samek/trie-memory/issues/6) (M2)
- LLM-based renderer (Task 06 option B) —
  [#7](https://github.com/tomas-samek/trie-memory/issues/7) (M3)
- STALE mode + domain-volatility classifier —
  [#8](https://github.com/tomas-samek/trie-memory/issues/8) (M3)
- Cross-domain confidence calibration + per-domain recency half-life —
  [#9](https://github.com/tomas-samek/trie-memory/issues/9) (M3)
- Multi-observer flows —
  [#10](https://github.com/tomas-samek/trie-memory/issues/10) (M3)
- Automatic contradiction detection during writes —
  [#11](https://github.com/tomas-samek/trie-memory/issues/11) (M3)
- Embedding layer for deep semantic paraphrase —
  [#12](https://github.com/tomas-samek/trie-memory/issues/12) (M3)
- Cryptographic provenance signing —
  [#13](https://github.com/tomas-samek/trie-memory/issues/13) (M3)
- Persona / style tuning —
  [#14](https://github.com/tomas-samek/trie-memory/issues/14) (M3)

**P(we resist scope-creep into these during PoC): ~50%**, be honest.

### M1 — PoC robustness (open issues uncovered by Phase B)

Not deferred; these are the known gaps still visible in the current
PoC state:

- Indexing bug: distinctive content words route to a different subtree
  than standard prose (Phase B stress `tides vs semaphore` soft fail) —
  [#1](https://github.com/tomas-samek/trie-memory/issues/1)
- Mode selector threshold robustness (sensitivity sweep + reproducible
  tuning log) —
  [#2](https://github.com/tomas-samek/trie-memory/issues/2)
- Coverage signal has a hidden English prior (stopwords + trailing-s
  stemming) —
  [#3](https://github.com/tomas-samek/trie-memory/issues/3)

---

## Decision gates

After Phase A:
- **All three scenarios pass** → proceed to Phase B.
- **Scenario 1 fails** → stop. Hallucination prevention is foundational.
- **Scenario 2 fails** → stop. No learning means no agent, just search.
- **Scenario 3 fails** → diagnose persistence layer, do not proceed to B.

After Phase B:
- **4A + 4B pass** → proceed to Phase C with the "no embeddings" claim alive.
- **4A passes, 4B fails** → proceed to Phase C with claim narrowed to
  "works on literal + near-literal recall." Plan embedding layer as
  post-PoC addition.
- **4A fails** → architecture is too literal. Revise before Phase C.

After Phase C:
- **Scenario 5 passes + Phase A/B minimum** → PoC succeeded. Write up
  what we learned and decide on next architecture step.
- **Scenario 5 fails hard (gaslight susceptibility)** → revision layer
  design is wrong; fix before declaring PoC complete.

---

## What we are betting on, explicitly

Ranked by how load-bearing the bet is, highest first:

1. **No-hallucination discipline** (Phase A). If we can't achieve this,
   the whole project's value proposition is gone. High confidence we
   can — it's mostly a matter of rendering discipline + mode selector.

2. **Paraphrase without embeddings** (Phase B). This is the architectural
   novelty. Medium-low confidence. If it fails, we learn something real.

3. **Append-only correction** (Phase C). Mechanical, high confidence
   conditional on A.

4. **Universality of tokenizer** (Phase D). Unknown, not tested in PoC.
   Aspirational.

---

## Status log

Update this section as phases complete. Keep entries short.

- 2026-04-19 — Design directory complete (8 tasks + architecture).
  progress.md created. No code yet.
- 2026-04-19 — Prereq done: TASK.md fix landed, routing revised for
  width+depth balance, corpus scaled, ContentStore switched to
  content-addressable path-key indexing.
- 2026-04-19 — Phase A complete: Task 01 / 03 / 06 shipped, Scenarios
  1/2/3 pass. 143/143 tests green across 14 suites.
- 2026-04-19 — Phase A exercised end-to-end on realistic conversations
  (`tests/phase_a_exercise.rs`). Findings in
  `docs/session_2026_04_19_phase_a_exercise.md`:
  - ✓ literal and near-literal recall, context enrichment,
    snapshot/restore, no-fabrication all hold.
  - ✗ false positives on unrelated English queries ("cookies" returns
    "semaphore"): `shared_suffix_depth` discriminates writing systems,
    not topics within a writing system.
  - ✗ ties broken by insertion order, so verbatim tides query returns
    semaphore as top when both are taught.
  - Decision gate: pass-with-caveats. Proceed to Phase B with Task 04
    `coverage` as priority — that's the single metric that defuses
    the false-positive failure mode.
- 2026-04-19 — Phase B shipped (steps 1–3): coverage gate, full
  `ConfidenceVector`, Task 05 cascade with Disambiguate,
  scenario-4 harness (10/10 with stopword + trailing-s
  normalization).
- 2026-04-19 — Phase B stress-tested (`tests/phase_b_stress.rs`).
  Three real bugs surfaced: recall-pool truncation, punctuation
  sticking to tokens, enriched-query polluting routing. All fixed.
  Coverage-Answer threshold retuned to 666‰ to avoid "valid
  question vs absent-answer" false positive. Final stress: 19/20
  pass, 0 hard fails. Remaining soft fail is the path/content
  mismatch on distinctive-word queries — genuine architectural
  limit.
- 2026-04-19 — Phase C (Task 07 + Scenario 5) complete. Correction
  metadata (`corrects`/`revised_by`) wired through schema,
  `handle_remember`, and the cascade. New `Conflicted` mode fires
  before path-fall-through so revisions always surface. Append-only
  preserved. 163/163 tests pass.
- 2026-04-23 — Post-PoC work migrated to GitHub milestones. Three
  milestones (M1 PoC robustness, M2 Honesty & validation, M3 Post-PoC
  Phase D) covering 14 issues (#1–#14) now track everything beyond
  current PoC state. README gained a Roadmap section linking them.

---

## Provenance

This file is the PoC execution plan on top of the architecture design.
It exists because staged execution + explicit probability estimates
force us to be honest about what we believe before building, and give
us a record to compare against after.