# Task 04 — Confidence Vector Computation

**Status:** Design, heuristics speculative
**Depends on:** Tasks 01, 02, 03
**Blocks:** Tasks 05, 06, 08

## Goal

Given a recall result (top-N resonating paths), compute a multi-dimensional
confidence vector that the Response Mode Selector (Task 05) uses to pick
the right response mode.

## Why a vector and not a scalar

A scalar confidence score (e.g., 0.87) is the industry default and it
**lies**. It conflates:

- "I have a strong match on an irrelevant path" (high by word coincidence)
- "I have a weak match on a highly relevant path" (right area, uncertain details)
- "I have two equally strong matches that contradict" (confident but wrong)
- "I have one strong match with no corroboration" (suspicious certainty)

All four produce similar scalar scores but warrant very different responses.
The agent must distinguish these cases, so it must compute the components
separately.

## The vector

```
confidence_vector = {
  signal_strength:     float [0, 1],   # amplitude of top resonance
  signal_clarity:      float [0, 1],   # top vs second, gap
  domain_density:      float [0, 1],   # how well-covered is this area
  source_trust:        float [0, 1],   # provenance-weighted trust
  recency:             float [0, 1],   # how fresh is the match
  contradiction_flag:  bool,            # competing paths disagree
  coverage:            float [0, 1],   # how much of query matched
  legacy_origin:       bool,            # any legacy-schema deposits involved
}
```

### signal_strength

Normalized amplitude of the top-ranked resonating path. The trie returns
some internal measure (match_depth, visit_count, combined score). This is
normalized against some reference — a calibration path, a session max,
or a rolling percentile. PoC uses a simple normalization to [0, 1] over
the top-N's range. Inadequate for cross-session comparison, acceptable
for PoC.

### signal_clarity

How much the top path dominates the runner-up:

```
clarity = 1 - (amplitude[1] / amplitude[0])
```

Clamped to [0, 1]. Clarity near 0 means multiple paths tied; clarity near 1
means one path clearly dominates. This is **the primary input to the
DISAMBIGUATE decision**.

If only one result is returned, clarity is undefined; convention: return
1.0 but flag `low_N` so the mode selector can de-weight.

### domain_density

How many paths exist in the subgraph near the top match. Computed by
counting distinct paths with amplitude above some threshold relative to
the top. High density means the area is well-explored; low density means
the top match may be coincidental.

PoC implementation: count of top-N matches with amplitude > 0.3 × top.
Range: 0 (only top matches anything) to 1 (all N are strong matches).

### source_trust

Aggregated `trust_level` from the provenance of deposits along the top path.
If all deposits on the top path are `user_direct` trust=0.9, `source_trust`
is 0.9. If any are `web_fetched` trust=0.4, aggregate weights them down.

Weighting: mean, weighted by deposit contribution to the resonance
(deeper in match = more weight). For PoC, simple mean is acceptable.

### recency

Time since the top path's deposits were last reactivated, normalized:

```
recency = exp(-age_ticks / half_life)
```

`half_life` is **domain-dependent** — post-PoC, per-domain config; for PoC,
a single global value. Proposed default: half_life = 7 days in wall-clock,
translated to ticks.

Recency feeds the STALE mode indirectly (low recency + time-sensitive
domain → STALE).

### contradiction_flag

True if the top-N contains paths that are semantically contradictory.
**Detection of contradiction is its own problem** (see Task 07) and for
PoC is limited to explicit revision markers (a path has a `revised_by`
reference pointing to a different path that is also in the top-N).

In PoC, most contradictions will not be detected. This is acceptable as
an explicit limitation.

### coverage

How much of the enriched query matched the top path:

```
coverage = matched_tokens / total_query_tokens
```

Low coverage with high signal_strength is **suspicious**: the path matched
a small part of the query very well, but most of the query was ignored.
This is a word-coincidence signal.

### legacy_origin

Boolean: did the top path include any deposits from the legacy v1 schema
(no provenance metadata)? If so, `source_trust` is unreliable and mode
selector must downgrade accordingly.

## Interface

```
compute_confidence(
  query: EnrichedQuery,
  recall_result: RecallResult  # top-N paths with metadata
) -> ConfidenceVector
```

Deterministic; no hidden state; pure function over recall output.

## Test criteria

- [ ] A query with one clear match returns high `signal_strength`,
      high `signal_clarity`, moderate `coverage`.
- [ ] A query with two tied matches returns high `signal_strength`,
      low `signal_clarity`, flag for `DISAMBIGUATE`.
- [ ] A query with only legacy deposits in top-N sets `legacy_origin=true`.
- [ ] A query matching one keyword but missing most of the rest returns
      high `signal_strength` but low `coverage`.
- [ ] A query with no meaningful matches returns low across the board.
- [ ] Recency decays monotonically over elapsed wall-clock time.
- [ ] Changing `half_life` changes recency proportionally.

## Failure criteria

- Vector components do not usefully discriminate. Signal: across a test
  set of 20 queries covering all 6 target modes, the selector (Task 05)
  chooses the right mode <50% of the time. This is below useful threshold.
- Signal_clarity proves non-informative because trie recall rarely
  returns single-path-dominant top-N (e.g., all recalls have roughly
  equal amplitudes). Signal: clarity distribution is tightly clustered
  rather than spread.
- Coverage cannot be computed meaningfully because the trie doesn't track
  which query tokens matched which path sections. This requires the trie
  to expose matching detail not currently in its API.

Last item is important: **this task may require extending the trie MCP**
to expose matching detail. If not feasible, coverage becomes a weaker
approximation (e.g., query length relative to path depth), which is
one of the empirical risks.

## Open questions

1. Normalization strategy for `signal_strength`. Session-max is simple
   but makes first queries in a session incomparable with later ones.
   Rolling percentile is robust but requires history. PoC uses session-max
   and accepts the limitation.

2. Aggregation of `source_trust` across a path — mean, min, or weighted?
   Mean is forgiving; min is harsh; weighted is arbitrary. PoC uses
   weighted mean by depth; revisit if it proves wrong.

3. Is `legacy_origin` a sufficient signal, or should there be a separate
   `mixed_trust` for deposits with widely varying trust levels? Proposed:
   start with `legacy_origin`; add `trust_variance` if needed.

4. Does the confidence vector need to be calibrated across domains?
   A "strong" match in a sparse domain may be numerically weaker than a
   "medium" match in a dense domain. Proposed: ignore for PoC; document
   as known issue; fix with per-domain calibration post-PoC.

## Non-goals

- Probabilistic interpretation (Bayesian posterior, etc.). The vector is
  a diagnostic tool, not a probability distribution.
- ML-learned weights. Heuristics in PoC; learned calibration post-PoC if
  the architecture survives PoC.

## Note on the broader problem

Computing useful confidence over non-vector memory is an **open research
problem**. There is no well-validated formula for any of the components
above; they are reasonable guesses informed by the tick-frame ontology.

The main PoC contribution here is not "we have the right formulas" but
"we keep these components separate and visible, rather than collapsing
them into one opaque score." Even crude components, kept separate, give
the mode selector more to work with than an LLM's token-level confidence.
