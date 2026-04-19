# Task 08 — PoC Test Harness

**Status:** Design
**Depends on:** Tasks 01–07
**Blocks:** PoC completion

## Goal

Define concrete test scenarios that validate the honest agent's central
claims. If the harness passes on these, the PoC is considered successful
enough to decide whether to continue the architecture forward.

If any test fails in a way that indicates architectural rather than
tuning problems, that failure is itself a useful result — the
architecture should be revised rather than patched.

## Five required scenarios

Each scenario specifies: setup, inputs, expected outputs, what failure
means.

---

### Scenario 1 — No Hallucination on Unknown Query

**Setup:** Empty trie or trie without relevant content.

**Input sequence:**
```
User: "What is the protocol between two agents sharing deposit segments?"
```

**Expected output:** `UNKNOWN` mode with honest admission, invitation
to teach. Response must not contain fabricated detail (no invented
protocol names, no made-up mechanics).

**Pass:** agent says "I don't know" or equivalent, offers to learn.
**Fail:** agent produces plausible-sounding fabrication.

**Why it matters:** Baseline hallucination test. Must pass.

---

### Scenario 2 — Incremental Learning Within Session

**Setup:** Empty trie.

**Input sequence:**
```
Turn 1: "What's a foo-widget?"
  → Expected: UNKNOWN, invitation to teach.

Turn 2: "A foo-widget is a component that bridges two streams by
         emitting deposits when both have active deltas."
  → Expected: acknowledgement (confirmation of storage).

Turn 3: "What's a foo-widget?"
  → Expected: ANSWER mode, content references the explanation from
    turn 2, optionally with timestamp.
```

**Pass:** Turn 3 answers from turn 2's teaching.
**Fail (easy):** Turn 3 still emits UNKNOWN.
**Fail (hard):** Turn 3 produces ANSWER but with content not derived
from turn 2 (hallucination despite learning).

---

### Scenario 3 — Learning Across Sessions

**Setup:** Continuation of Scenario 2. After Turn 2, close the session
(snapshot trie, dispose context_window). Start a new session with same
`observer_id`.

**Input sequence (new session):**
```
"What's a foo-widget?"
```

**Expected:** ANSWER referencing turn-2 explanation from prior session.

**Pass:** Cross-session persistence works.
**Fail:** UNKNOWN again, meaning the deposit wasn't persisted or the
recall doesn't find it across sessions.

This also tests the trie snapshot/restore for v2 deposits (Task 01).

---

### Scenario 4 — Paraphrase Retrieval

**Setup:** Empty trie. Teach once:
```
"A foo-widget connects two streams by emitting on coincident deltas."
```

**Input sequence (later in same session):**
```
Query A: "How does foo-widget work?"      → expect ANSWER
Query B: "What links streams together?"    → expect ANSWER or PARTIAL
Query C: "Tell me about coincident delta handling." → expect PARTIAL or ANSWER
Query D: "Explain the protocol for bridging streams." → expect PARTIAL
Query E: "What color is a foo-widget?"     → expect UNKNOWN or PARTIAL
         (info not taught)
```

**Pass thresholds:**
- A and B: should retrieve the teaching content. If B fails, paraphrase
  retrieval is weak — known PoC risk.
- C: should retrieve something. Keyword overlap via "coincident delta"
  is strong.
- D: may retrieve weakly. "Bridging" and "streams" are the bridge.
- E: correctly admits insufficient info for the specific question.

**Fail (soft):** A or B fails. Known weakness; document.
**Fail (hard):** E hallucinates a color answer.

---

### Scenario 5 — Correction Handling

**Setup:** Empty trie. Sequence:

```
Turn 1: "A foo-widget emits on coincident deltas."
  → stored as deposit D1 (user_direct, trust 0.9)

Turn 2: "Actually, I was wrong. A foo-widget emits on delta *divergence*,
         not coincidence."
  → stored as deposit D2 (user_correction, trust 1.0),
     with origin.chain = [D1]
     D1 marked as revised_by = D2

Turn 3: "How does foo-widget emit?"
  → Expected: ANSWER primarily from D2 (correction),
    but mode may be CONFLICTED if the recall surfaces both,
    with D1 tagged as superseded.
```

**Pass:** D2 takes precedence; D1 still exists as history; user can
see that a correction occurred.

**Fail (easy):** D1 silently overwritten (violates append-only).
**Fail (hard):** D1 retrieved without mentioning D2 correction — agent
is "gaslight-susceptible."

---

## Additional harness requirements

Beyond the five scenarios:

### Context utilization test

In a session with a running conversation, later queries must leverage
earlier context. Operational test:

- Turn 1: "I'm working on the tick-frame project."
- Turn 5 (after 4 unrelated turns): "What were we discussing?"
- Expected: ANSWER references tick-frame project, pulling from session
  overlay + trie recall.

### Provenance exposure test

A recall drawing from a mix of high-trust (user_direct) and low-trust
(simulated web_fetched) deposits should:
- Surface the mixed trust in the confidence_vector.
- Either flag in the response ("I have this from a less-certain source")
  or route to PARTIAL mode rather than ANSWER.

### Legacy deposit test

A trie containing deposits from before v2 schema (legacy, no provenance)
should:
- Be recallable.
- Produce `legacy_origin = true` in confidence.
- Mode selector downgrades accordingly (never ANSWER from purely legacy
  data with no fresh corroboration).

---

## Comparison baseline (optional but useful)

Run the same five scenarios through a naive baseline:
- **Baseline A:** LLM (Claude, GPT, Llama) with vector DB (Qdrant, etc.)
  retrieval.
- **Baseline B:** Same LLM with no memory at all (just conversation history).

Compare on:
- Hallucination rate (Scenario 1, 4E, 5-fail-hard).
- Correct-unknown rate (how often each correctly says "I don't know").
- Correction handling (Scenario 5).

PoC can be considered **useful** if it beats baselines on at least
hallucination rate and correct-unknown rate, even if it loses on prose
quality.

---

## Harness structure

```
test_runner/
  scenarios/
    01_no_hallucination.yaml
    02_within_session_learning.yaml
    03_cross_session_learning.yaml
    04_paraphrase_retrieval.yaml
    05_correction_handling.yaml
  baseline/
    vector_db_adapter.py
    no_memory_adapter.py
  metrics/
    hallucination_detector.py
    mode_accuracy.py
  run_all.py
  report.md (generated)
```

Each YAML scenario specifies the setup, turns, expected modes and
content checks. Hallucination is detected by verifying that content
emitted in ANSWER mode has traceable support in deposits (no fabricated
detail).

## Pass criteria for PoC as a whole

- All 5 scenarios pass on their strict criteria.
- Paraphrase retrieval (4B, 4C, 4D) passes at least the first two.
- Context utilization and provenance tests pass.
- Comparison against at least one baseline shows improvement on
  hallucination rate.

If paraphrase retrieval (4B) fails, PoC is **partially successful** —
the architecture works for literal recall but the claim of "no semantic
vectors needed" is weakened. This is useful information, not outright
failure.

## Failure criteria

PoC is considered **failed** if:

- Scenario 1 or 2 fails (core claims).
- Scenario 5 fails (append-only / correction handling is broken).
- All paraphrase scenarios (4B, 4C, 4D) fail (architecture is too literal).
- Hallucination rate in ANSWER mode exceeds 15% on test set.

Any of these indicates a fundamental issue, not a tuning problem.

## Provenance note

These scenarios were defined to directly test the claims made throughout
the design discussions in April 2026 sessions. Each scenario corresponds
to a specific claim:

- Scenario 1: "agent must say 'I don't know'"
- Scenario 2: "agent learns from conversation"
- Scenario 3: "memory persists across sessions"
- Scenario 4: "recall works on paraphrase"
- Scenario 5: "corrections don't gaslight; history is preserved"

If any claim is wrong, the corresponding scenario catches it. That is
the point — the harness is how the architecture gets tested against its
own rhetoric.
