# Session 2026-04-19 — Phase A in Action

## Purpose

Phase A builds clean on a test suite (142 tests green) but test code is not
usage. Before declaring success and starting Phase B, drive the honest-agent
stack through scenarios that look like a real conversation and capture what
actually happens — both where the contract holds and where it visibly
breaks.

Exercise is `tests/phase_a_exercise.rs`. Run with:

```
cargo test --test phase_a_exercise -- --nocapture
```

Seven scenarios, labelled A–G, plus a final trie-state dump.

---

## What works

### A — Literal recall
- **Empty trie:** `trie_ask("A semaphore is a synchronization primitive…")`
  → `mode: unknown`, no supporting memories, response admits ignorance.
- **After `trie_remember` with identical phrasing** → `mode: answer`,
  `shared_suffix: 3`, response quotes the stored content verbatim with
  `[semaphore] … (from you)`.

Contract: no hallucination on empty trie. ✓
Contract: stored content recalled after teaching. ✓

### B — Near-literal recall
- `"… counts permits today."` (trailing word) → **answer**, still
  `shared: 3`, correct memory surfaced.
- Prefix-reordered variant → **answer**, same.

The byte-trie tolerates small phrasing changes. Shared suffix remains 3
because the delta distribution of English is narrow enough that tiny
additions don't divert the route.

### F — Context enrichment
- Turn 1 on fresh context: recalls semaphore memory.
- Turn 2 `"what about today"` (ambiguous, short): the enriched query
  carries `topics_used: ["semaphore", "tides"]` from the rolling buffer,
  proving the context plumbing injects prior topics into follow-ups as
  designed.

Contract: Task 03 query enrichment works end-to-end. ✓

### Scenarios 1, 2, 3 (Task 08)
All three pass as strict-assert tests in `tests/honest_agent_scenarios.rs`:
no hallucination on unknown, within-session learning, cross-session
snapshot/restore.

---

## Where Phase A visibly breaks

### D — False positives on unrelated queries (serious)
- With the semaphore memory stored, asking
  `"What temperature for chocolate chip cookies?"`
  returns **answer** with the semaphore memory, `shared_suffix: 3`.

This is not a marginal failure; it is the mode selector's core
weakness. The path-suffix ≥ 2 rule treats "shared byte-trie ancestry"
as a proxy for "topical relevance," and at this corpus size English
content all routes through the same shallow English subtree. Any
English query that reaches the same depth matches any taught English
memory.

Concretely: the byte-trie ends up with ~4 nodes total on this corpus,
root's spectrum covers ~64 of the 256 possible deltas, and the two
depth-1 children cover most of the rest. Every English query's
`deepest_node` ends up identical to the semaphore memory's. Suffix
comparison returns 3.

### E — Ranking collapses on ties
- After teaching both `semaphore` and `tides` memories:
  - `"A semaphore is a synchronization primitive…"` → answer, top =
    semaphore ✓, but `tides` appears in "Also matched".
  - `"The bay of Fundy has tidal ranges…"` → answer, top = **semaphore**
    (wrong), tides shown as "Also matched".

Both memories tie on `shared_suffix: 3`. The tie-breaker is timestamp
(newer first) — but both were stored in the same test run with nearly
identical ticks, so stable sort returns them in insertion order.
Semaphore was stored first, so it always wins. The tides query,
verbatim with its own memory, gets the semaphore memory as top answer.

### G — Provenance tagging can be hidden by a tie
- Teaching a `web_fetched` memory with lower trust and then querying
  its exact content returns the **semaphore** memory as top (same
  depth tie), so the "(from the web — lower trust)" rendering never
  fires. The low-trust source is demoted to "Also matched" and its
  trust flag isn't surfaced as loudly.

### C — Paraphrase (Phase B preview)
- `"How do threads coordinate with shared counters?"` →
  `mode: answer` with the semaphore memory, `shared: 3`.

This looks correct in isolation (semaphores do coordinate threads
with counters), but the mechanism is wrong: the trie doesn't
understand the semantic relationship, it just shares a path because
both strings are English. D proves the same mechanism fires for
completely unrelated questions. The paraphrase "success" here is
fortuitous, not principled.

---

## Root cause — one sentence

**`shared_suffix_depth` is a language-family discriminator, not a
topic discriminator, at current trie scales.** The byte-trie
differentiates writing systems (CLAUDE.md's Czech=12, Japanese=24,
English=1 finding), not topics within a writing system. Phase A
leans on suffix depth alone and therefore can't tell semaphore from
cookies when both are English.

---

## What this means for Phase B

The failures above are exactly what the Task 04 confidence vector
and Task 05 mode selector are designed to filter. Specifically:

1. **`coverage` kills D (cookies).** `coverage = matched_query_tokens
   / total_query_tokens`. `"chocolate chip cookies"` has zero content
   overlap with the stored semaphore memory, so coverage ≈ 0 and the
   selector routes to PARTIAL or UNKNOWN, not ANSWER.

2. **`signal_clarity` catches E (verbatim-tides-returns-semaphore).**
   When two supporting memories tie on shared suffix, `clarity =
   1 - (second / top) ≈ 0`, which routes to DISAMBIGUATE — surfacing
   both versions to the user rather than arbitrarily picking one.

3. **`source_trust` weighting fixes G (hidden web-source tag).** If
   the top tie-breaker prefers lower-trust sources to be disclosed
   regardless of tie position, the trust tag always surfaces.

4. **`domain_density` catches C (fake paraphrase match).** A paraphrase
   that lands at a sparse part of the graph with one nominal match
   should be treated as `PARTIAL` ("I have partial info"), not
   `ANSWER`. Coverage plus density together should separate genuine
   paraphrase from accidental path overlap.

The Phase B plan in `progress.md` already flags paraphrase as the
≥ 45 % likelihood of failing. This exercise confirms the failure
is real on literal data — not just a hypothetical. The confidence
vector is not a luxury feature; without it, Phase A is strict for
UNKNOWN admission but leaky in the reverse direction.

---

## Phase A success criteria — status

| Claim | Status |
|-------|--------|
| No hallucination on unknown/empty query | ✓ holds |
| Store + recall within session (literal) | ✓ holds |
| Cross-session persistence (snapshot/restore) | ✓ holds |
| Context window enriches follow-ups | ✓ holds |
| Never fabricates content in response | ✓ holds (templates only, sourced from `supporting`) |
| Correctly distinguishes topics within a writing system | ✗ **does not hold** — false-positive rate is high on small corpora |
| Ranking picks the best memory when multiple match | ✗ ties broken by insertion order, not relevance |
| Trust/provenance visible to the user | partial — surfaces on top, but low-trust can hide beneath a tie |

Five of eight hold; three fail in ways Phase B (confidence vector +
full mode selector) is specifically designed to address.

---

## Recommendation

Proceed to Phase B. Do not try to patch the false-positive rate inside
Phase A — the fix is architecturally the confidence vector, not a
tweak to `shared_suffix_depth`. Attempting to harden the Phase A
heuristic (e.g., add word-overlap rules directly in the mode selector)
would duplicate Task 04 in a weaker form.

Specifically, start Phase B with **Task 04 `coverage`** as the
priority — it is the single component that directly defuses D
(unrelated-query false positives), which is the loudest failure
mode. Ties into Task 05's cascade cleanly.

---

## Raw exercise output

See output of `cargo test --test phase_a_exercise -- --nocapture`.
Key lines captured inline above.

---

## Addendum — Phase B step 1 (coverage gate) applied

Added Task 04 `coverage` as the first Phase B move and wired it into
the mode selector. Coverage = fraction of the raw query's distinct
content-words that appear in the candidate memory's content
(case-insensitive, integer ppm out of 1000). Selector rules:

| suffix ≥ 2 | coverage ppm | mode |
|------------|--------------|------|
| no | — | Unknown |
| yes | 0–199 | Unknown (path-only fall-through) |
| yes | 200–499 | Partial (new mode) |
| yes | ≥ 500 | Answer |

Tie-breaker changed from insertion order to
`(suffix DESC, coverage DESC, timestamp DESC)`. Re-ran
`tests/phase_a_exercise.rs`.

### Before vs after (same exercise)

| Scenario | Phase A | Phase B step 1 |
|----------|---------|----------------|
| A.1 empty → ask | Unknown ✓ | Unknown ✓ |
| A.2 literal recall | Answer ✓ | Answer (cov 1000‰) ✓ |
| B.1 trailing word | Answer ✓ | Answer (cov 888‰) ✓ |
| B.2 reordered | Answer ✓ | Answer (cov 700‰) ✓ |
| C.1 paraphrase | **Answer (fortuitous)** | **Unknown (cov 0‰)** — honest |
| D.1 unrelated ("cookies") | **Answer (false +)** | **Unknown (cov 0‰)** — fixed |
| E.1 semaphore query | Answer, top=semaphore (lucky) | Answer, top=semaphore (principled) |
| E.2 tides query (verbatim) | **Answer, top=semaphore** (bug) | **Answer, top=tides** — fixed |
| F.1 first turn | Answer ✓ | Answer ✓ |
| F.2 short follow-up | Answer (leaked into semaphore) | Unknown (cov 0‰) — more honest |
| G.1 web-source recall | Answer, top=semaphore, trust tag hidden | Answer, **top=coffee-claim**, "(from the web — lower trust)" surfaces |

### Fixed

- **D (false-positive on unrelated query):** cookies → Unknown. The
  most visible Phase A failure is gone.
- **E.2 (ranking collapsed on ties):** verbatim tides query now
  returns tides memory as top; coverage wins over insertion order.
- **G (trust hidden behind tie):** coffee-claim surfaces as top and
  the "(from the web — lower trust)" tag fires.
- **C (fake paraphrase match):** paraphrase honestly admits ignorance
  rather than returning the stored semaphore sentence for the wrong
  reason.

### Interesting new behavior

F.2 ("what about today" follow-up after teaching semaphore) now
returns Unknown instead of an incorrect semaphore answer. This is
more honest — the user's literal question has zero topical content.
Proper handling would be a DISAMBIGUATE mode that asks "are you
asking about semaphore or tides?", which is the next Phase B step.

### Still open

- Paraphrase that actually uses stem-equivalent words (`counts` vs
  `counters`) still fails — word-level exact match doesn't bridge
  morphology. This is the next layer after the full confidence
  vector lands.
- No DISAMBIGUATE mode yet; F.2 would benefit from it.
- Partial mode exists but isn't yet exercised by the scenario set.

### Suite status

All 146 tests pass (14 responder tests including 5 new ones for
coverage). Phase A exercise still green.

---

## Addendum — Phase B step 2 (confidence vector + cascade)

Landed the full `ConfidenceVector` struct (signal_strength,
signal_clarity, domain_density, source_trust, recency, coverage,
legacy_origin, plus a reserved contradiction_flag) and the Task 05
cascade with four active modes: Unknown, Partial, Answer, and the
new **Disambiguate**.

Cascade order (abridged from the responder):

1. No memories / root-only suffix → Unknown.
2. Coverage below PARTIAL (200‰) → Unknown.
3. Legacy origin with source_trust < 500‰ → Partial.
4. Two or more candidates tied on (suffix, coverage) AND clarity <
   250‰ → **Disambiguate**.
5. Coverage below ANSWER (500‰) → Partial.
6. Otherwise → Answer.

Key refinement: `signal_clarity` is now measured on the **coverage
gap within the top suffix bucket**, not the combined score. This
was the original `1 - (second / top)` formula from Task 04 applied
to the right signal — bucket-differentiated memories always
dominate their shallower siblings (clarity 1000), but two tied
memories at the same depth with similar coverage fire
Disambiguate correctly.

Also: `supporting` now filters by coverage ≥ MIN_COVERAGE_PARTIAL,
so the rendered "Also matched:" section no longer shows zero-overlap
siblings that happen to share a path bucket.

### Re-run highlights

- E.1 supporting count: 2 → **1**. The zero-coverage tides memory
  no longer appears in "Also matched" under a semaphore query.
- E.2 supporting count: 2 → **1**, top still tides (coverage wins).
- G.1: top=coffee-claim, trust tag surfaces, supporting=1 (the
  other taught memories with 0 coverage dropped out).
- F.2 still Unknown (as in step 1). The Disambiguate mode would
  fire if the query itself had coverage but multiple memories
  tied; for a short zero-content query like "what about today"
  the honest answer stays Unknown.
- All 12 new confidence tests pass (tests/honest_agent_confidence.rs).

### Suite status

10 test files with the new confidence suite; **156/156 tests
passing across 16 files**.

### What remains to validate

- Scenario 4 (paraphrase harness) — the stemming/synonym case.
- Whether the morphology gap (`counts` vs `counters`) ever
  resolves without a word normalizer.
- Whether `domain_is_volatile` + STALE makes sense in PoC
  (probably not — defer to post-PoC).

---

## Addendum — Phase B stress test (2026-04-19)

Built a broader stress harness (`tests/phase_b_stress.rs`) running
20 queries across 9 categories against a primed trie + 11 taught
memories. Dozens of smaller fixes fell out; two systemic problems
surfaced and were fixed mid-run.

### Systemic issues found + fixed

1. **Recall pool was truncated by `max_results` before coverage
   ranking.** `recall_by_path` sorts by timestamp and truncates to
   `max_results` (default 5). On a corpus of 11 stored memories, the
   later-inserted ones were dropped before the cascade could see
   them — so the SQL-join memory, 8th to be stored, wasn't available
   when the SQL-literal query came in. **Fix:** `handle_ask` now
   requests a generous pool (200 memories) from recall and lets the
   cascade's supporting-filter enforce the final count. The
   `max_results` arg still bounds the JSON output size.

2. **Punctuation stuck to words in coverage normalization.**
   `split_words` only breaks on whitespace plus `.`, `_`, `-`, `/`,
   `:`. Tokens like `semaphore?` and `work?` kept the trailing `?`,
   so `semaphore?` ≠ `semaphore` and coverage quietly returned 0 on
   any query ending in `?` or `,`. **Fix:** `normalize_words` now
   strips leading and trailing non-alphanumeric characters from
   every token after tokenization.

3. **Enriched query was routed through the trie.** Context-window
   enrichment was appending topic keywords to the routed bytes,
   which moved the query to a different subtree from the stored
   memory and caused literal recall to miss once context
   accumulated. **Fix:** `handle_ask` routes the raw query through
   the trie. Enrichment is still recorded in the response for
   diagnostics but no longer perturbs routing.

### Coverage threshold tuning

Original `MIN_COVERAGE_ANSWER = 500‰` promoted a false-positive: a
short question about a taught topic ("what color is a foo-widget")
matched 2 of 4 words — same ratio as a legitimate "how does it
work" question. Since word-coverage cannot tell a valid-question
from a specific-answer-absent question, the honest move was to
demand stricter overlap for Answer and let Partial absorb the
ambiguous middle. Raised threshold to **666‰** (two-thirds).

### Final stress results

- **19 / 20 pass, 0 hard fails.**
- Per category: L 3/3, P 3/3, X 1/2, D 1/1, U 3/3, T 1/1, Z 2/2,
  C 2/2, S 3/3.
- Only remaining soft fail: `[X] tides vs semaphore` — the query
  "Why do tides have such large ranges in some bays?" routes to
  `[6692..., 4272...]` while the tides memory is stored at
  `[None, 11675..., 4272...]`. Shared suffix = 1 (root only), so
  rule 1 of the cascade fires Unknown regardless of coverage.
  This is the path-vs-content mismatch case the user flagged
  ("we are tokenizing already abstracted things"): distinctive
  content words like "tides" / "bays" produce a distinctive byte
  trie route, which then doesn't match the broader-prose route the
  stored memory landed on. Genuine architectural limit; no
  attempt to paper over it.

### Suite status

**160 / 160 tests across 18 files.** Added stress harness and the
follow-on debug probes were removed.

### Net Phase B read

The stress test found three bugs the unit tests missed, all of
which turned out to be easy fixes once surfaced. The remaining
failure mode — query/memory path divergence on distinctive-word
content — is a structural limit of using raw byte-trie paths as
the primary index, and is exactly the "tokenizing already
abstracted things" concern worth holding for the post-Phase-B
discussion.
