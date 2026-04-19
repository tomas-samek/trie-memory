# Task 03 — Context Window (Observer State)

**Status:** Design
**Depends on:** Task 01
**Blocks:** Tasks 04, 05, 07, 08

## Goal

Maintain an observer-local context stack that enriches every query before
it reaches the trie. Short or bare queries must not be routed to trie recall
directly — they must first be augmented with the observer's current
conversational and cognitive state.

## Why this exists

Empirical finding from 2026-04 trie testing:

Query `"spaced repetition"` failed to recall the memory explicitly tagged
with `"opakování, spaced repetition, aktivní vybavování"`. The byte-level
trie matching gave higher resonance to structurally richer memories on
unrelated topics. Recognition amplitude dominated relevance when query
byte-mass was insufficient.

Query `"komunikace mezi agenty v časově kódovaných zprávách"` (much longer,
semantically denser) successfully recalled the relevant memory at
`match_depth = 11`.

Conclusion: **queries below some byte-mass threshold resonate with
everything** (high-depth memories dominate by structural richness), while
**queries with sufficient context resonate selectively**.

Context window is the mechanism that brings bare queries above the threshold
by composing them with observer state.

## Data structure

```
context_window = {
  observer_id:          string,
  session_id:           string,
  session_start:        tick,

  recent_exchanges:     [Exchange]   # rolling, bounded
  hot_concepts:         [ConceptRef] # recently reactivated
  active_topics:        [string]    # inferred or explicitly set
  current_modality:     string       # default stream for this session
  conversation_depth:   int          # turn counter

  relevance_overlay:    { path_id -> boost_factor }  # session-local
}

Exchange = {
  user_input:           deposit_id,
  agent_response:       deposit_id,
  mode_emitted:         ResponseMode,
  confidence_vector:    ConfidenceVector,
  tick:                 int
}
```

Rolling window: last N exchanges, with N configurable. PoC default: 10.

## Enrichment function

```
enrich_query(raw_query, context_window) -> EnrichedQuery
```

Steps:
1. Extract user's raw query text.
2. Concatenate with:
   - Last 2-3 exchange summaries (topic hint, not full content).
   - Current `active_topics` as keyword suffix.
   - `hot_concepts` labels as keyword suffix.
3. If `conversation_depth > 0`, add depth marker so resonance can
   distinguish "first question about X" from "followup about X".
4. Emit `EnrichedQuery` with both original and enriched forms stored.

Important: enrichment is **transparent to the user**. They see their own
question in the agent's response; they do not see the enriched version
unless they explicitly ask.

## Relevance overlay

Beyond query enrichment, context also maintains a per-session relevance
boost for paths reactivated during the session. This is separate from
the trie's global relevance (which persists across sessions).

Mechanism:
- When a deposit/path is recalled and emitted to the user, its ID gets
  `boost_factor *= 1.2` (or similar) in the overlay.
- Overlay decays: every N ticks, all overlays multiply by 0.9.
- Overlay is applied during recall: effective_amplitude = base_amplitude
  × overlay_boost.

This gives the current conversation structural priority in recall, without
permanently biasing the trie.

Overlay is **session-scoped**: new session starts with empty overlay.
Global trie relevance updates normally (slow reinforcement from reactivation).

## Interface

```
ContextWindow:
  create(observer_id, session_id) -> ContextWindow
  record_exchange(user_deposit, agent_deposit, mode, confidence) -> void
  enrich(raw_query) -> EnrichedQuery
  get_hot_concepts() -> [ConceptRef]
  get_overlay() -> RelevanceOverlay
  dispose() -> void  # on session end, persist final state, clear overlay
```

## Test criteria

- [ ] First query in a session is enriched with empty context
      (identical or minimally different from raw).
- [ ] Second query in the session is enriched with first exchange context.
- [ ] Same second-query text in **different** first-exchange contexts
      produces **different** enriched queries.
- [ ] Relevance overlay boosts recently-mentioned paths during subsequent
      recall in the same session.
- [ ] Relevance overlay does not persist into a new session for the same
      observer.
- [ ] Observer switching (same session, different user) resets hot_concepts
      but does not wipe the trie.

## Failure criteria

- Enriched queries become so long they over-constrain resonance (hit only
  exact matches). Signal: deep match_depth but zero results on reasonable
  follow-up queries.
- Context enrichment introduces topic drift (agent latches onto early
  conversation topics and can't escape). Signal: user changes topic
  explicitly, agent keeps returning to previous topic.
- Overlay decay rate proves either too fast (no effect) or too slow
  (session contamination persists uselessly).

## Open questions

1. Should enrichment append keywords as flat text or as structured markers?
   Proposed: flat for PoC (simpler to test against trie byte matching);
   structured in post-PoC when recognition engine can consume structure.

2. Who owns context window lifecycle — agent runtime or recognition engine?
   Proposed: recognition engine, with lifecycle events emitted to runtime.

3. Is `active_topics` manually set (by agent explicit detection) or purely
   emergent (from hot_concepts clustering)? Proposed: emergent only in PoC;
   manual topic-setting adds complexity not yet justified.

4. What happens when observer returns after long absence — does hot_concepts
   rebuild from scratch or get seeded from last session's closing state?
   Proposed: fresh start in PoC; long-term memory handled by base trie
   relevance, not session overlay.

## Non-goals

- Cross-session persistence of overlay. Overlay is ephemeral by design;
  persistent relevance is the trie's job.
- Multi-observer concurrent sessions. Single observer per context window
  in PoC.
- NLP-driven topic modeling. Simple keyword-based hot_concepts in PoC;
  sophisticated topic inference post-PoC.

## Provenance note

Direct consequence of 2026-04 trie recall tests. The need for context is
not a design choice; it is a response to observed failure modes in raw
recall.
