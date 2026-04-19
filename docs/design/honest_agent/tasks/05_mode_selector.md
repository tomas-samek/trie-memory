# Task 05 — Response Mode Selector

**Status:** Design
**Depends on:** Task 04
**Blocks:** Task 06, 08

## Goal

Given a confidence vector, select exactly one response mode that the
renderer (Task 06) will use to produce output.

## Modes

| Mode | Semantics | Renderer behavior |
|------|-----------|-------------------|
| `ANSWER` | One path dominates, trustworthy, covers query | Direct answer |
| `DISAMBIGUATE` | Multiple paths tied | Ask which variant user means |
| `PARTIAL` | Weak but non-zero resonance | Offer limited info with uncertainty |
| `UNKNOWN` | No meaningful resonance | Admit ignorance, invite teaching |
| `STALE` | Strong match, but old & in volatile domain | Answer with temporal caveat |
| `CONFLICTED` | Competing paths disagree | Surface the conflict to user |

Exactly one mode is selected per query. If multiple modes would apply,
precedence below resolves.

## Selection logic

Selection is **deterministic** — same confidence vector in, same mode out.
Logic is a decision cascade, not weighted scoring (yet).

```
if confidence_vector.signal_strength < STRENGTH_MIN:
    return UNKNOWN

if contradiction_flag:
    return CONFLICTED

if recency < STALE_THRESHOLD and domain_is_volatile:
    return STALE

if signal_clarity < CLARITY_MIN:
    return DISAMBIGUATE

if signal_strength < STRENGTH_HIGH or coverage < COVERAGE_MIN:
    return PARTIAL

if legacy_origin and source_trust < TRUST_LEGACY_MIN:
    return PARTIAL  # don't pretend confident about legacy data

return ANSWER
```

### Thresholds (PoC defaults)

All are tunable. These are **guesses**, to be revisited after first tests:

```
STRENGTH_MIN       = 0.15   # below: genuinely nothing
STRENGTH_HIGH      = 0.55   # above: confident signal
CLARITY_MIN        = 0.25   # below: ambiguity
COVERAGE_MIN       = 0.4    # below: partial match
STALE_THRESHOLD    = 0.3    # below: old enough to doubt
TRUST_LEGACY_MIN   = 0.5    # legacy data needs extra corroboration
```

### Domain volatility

`domain_is_volatile` is a classification of the query topic: news, prices,
personnel, current events → volatile. Definitions, historical facts →
stable. For PoC, this is a simple keyword heuristic or a fixed list of
volatile keywords; sophisticated topic classification is out of scope.

If unknown, default to non-volatile (don't spuriously STALE everything).

## Data returned

```
mode_selection = {
  mode:                ResponseMode,
  supporting_paths:    [PathRef],         # top paths backing the decision
  alternatives:        [PathRef] | None,   # for DISAMBIGUATE / CONFLICTED
  confidence_vector:   ConfidenceVector,   # passthrough for renderer
  reasoning:           string              # human-readable why (for audit)
}
```

`reasoning` is important. It lets the user — and future debugging — see
*why* a particular mode was selected. Transparency by design.

## Interface

```
select_mode(confidence_vector, recall_result) -> ModeSelection
```

Pure function. Deterministic.

## Test criteria

For each mode, at least one test case where that mode is correctly selected:

- [ ] `ANSWER` — well-covered query with clear winner returns ANSWER.
- [ ] `DISAMBIGUATE` — query matches "apple" (fruit) and "Apple" (company)
      equally returns DISAMBIGUATE with both alternatives.
- [ ] `PARTIAL` — query matches one keyword out of many returns PARTIAL.
- [ ] `UNKNOWN` — query on unseen topic returns UNKNOWN.
- [ ] `STALE` — query on recent-events topic with an old deposit returns
      STALE.
- [ ] `CONFLICTED` — query where two paths have explicit revision links
      pointing to each other returns CONFLICTED.

Additionally:

- [ ] `reasoning` string correctly describes the threshold that triggered
      the decision.
- [ ] Thresholds can be overridden per-session for tuning experiments.

## Failure criteria

- Decision cascade consistently routes to one or two modes regardless of
  input. Signal: across 20 test queries, >80% land on the same mode.
- Thresholds are so sensitive that tiny changes flip mode selection
  chaotically. Signal: changing a threshold by 0.05 changes >30% of
  decisions in the test set.
- Users / testers cannot understand the `reasoning` output.

## Open questions

1. Should mode selection be deterministic (as proposed) or include
   stochastic elements (e.g., occasionally ask back even when confident,
   to invite correction)? Proposed: fully deterministic in PoC. Determinism
   is testable; stochasticity adds noise that masks architectural issues.

2. Should the selector have memory across consecutive queries? E.g., if
   the previous mode was DISAMBIGUATE and the user clarified, should this
   query be biased toward ANSWER? Proposed: no explicit memory — context
   window (Task 03) already carries forward relevant state via the
   relevance overlay.

3. Is there value in a `DECLINE` mode for queries the agent refuses to
   answer (offensive, out of scope, dangerous)? Proposed: yes, but
   post-PoC. Handled at a different layer.

4. Should `STALE` fold into `PARTIAL` to simplify? Proposed: keep separate.
   Staleness is a distinct epistemic state ("I knew this, it's just old")
   vs. partial knowledge ("I never had this fully").

## Non-goals

- Weighted scoring / learned thresholds. Simple cascade in PoC.
- User-configurable mode behavior. Fixed mapping mode → renderer behavior.
- Multi-mode output ("ANSWER but also PARTIAL for sub-question"). One
  query, one mode.

## Implementation note

This is the component where the **honest agent philosophy becomes visible**.
Everything upstream could be hidden implementation; this is where the
agent's character is defined. A bug here — selecting ANSWER when
DISAMBIGUATE is warranted — is a hallucination.

The PoC must never silently fall back to ANSWER when signals suggest
otherwise. If in doubt, prefer admitting uncertainty. This preference
should be hard-coded into the thresholds, not optional.
