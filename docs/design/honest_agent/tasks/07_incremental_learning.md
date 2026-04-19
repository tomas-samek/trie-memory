# Task 07 — Incremental Learning Loop

**Status:** Design
**Depends on:** Tasks 01–06
**Blocks:** Task 08

## Goal

Close the loop: when the agent admits ignorance (UNKNOWN) or partial
knowledge (PARTIAL) and the user provides information, that information
must be stored in a way that **improves the next similar recall**, without
retraining or any model-level change.

This is the primary differentiator from LLM+vector-DB: true incremental
learning from conversation.

## Success condition

After the sequence:

1. User asks about concept X → agent emits UNKNOWN.
2. User explains X.
3. Agent stores explanation as deposit.
4. User (same or different session) asks about X again.
5. Agent emits ANSWER with the explanation.

The test passes if (5) works without any offline update, retraining, or
manual intervention.

## Components involved

This task is largely **integration** of existing components:

- Task 01 (deposit schema) — stores the explanation with provenance.
- Task 02 (tokenizer) — tokenizes the explanation stream.
- Task 03 (context window) — keeps the concept "hot" so the next query
  resonates even if worded differently.
- Task 04 (confidence) — should show improved signal_strength on re-query.
- Task 05 (mode selector) — should route to ANSWER now.
- Task 06 (renderer) — produces the answer from the stored explanation.

Specific integration: **the agent itself produces deposits**, not just the
user. When the agent emits UNKNOWN and the user explains, the user's
explanation gets deposited, and the link between the original query and
the explanation is stored as relationship metadata.

## The "teaching" flow

```
User: "What's a semaphore?"
  ↓
Query routed through context_window.enrich(...)
  ↓
Trie recall — low resonance
  ↓
Confidence: signal_strength=0.05, coverage=low
  ↓
Mode: UNKNOWN
  ↓
Renderer: "I don't know. Tell me?"
  ↓
User: "A semaphore is a synchronization primitive that..."
  ↓
Tokenizer emits deposit with:
  - source_type: user_correction (or user_direct if no prior context)
  - trust_level: high (explicit teaching)
  - origin.chain: [previous UNKNOWN query deposit]
  ↓
Trie stores deposit
  ↓
Hot_concepts gets "semaphore" added
  ↓
Next similar query resonates with this new deposit
```

Key design decision: **the UNKNOWN emission itself is stored as a deposit**,
not as an ephemeral response. This creates a record: "on 2026-04-19 I was
asked about X, I didn't know, user then explained Y." This record is:

- Auditable (you can see what the agent didn't know before).
- Linkable (the explanation deposit references the UNKNOWN deposit).
- Learnable-from (the agent can, over time, track how many UNKNOWNs it
  resolves vs how many recur — a metric of learning rate).

## Deposit relationships

```
deposit_A (UNKNOWN emission) --taught_by--> deposit_B (user explanation)
deposit_B --teaches_about--> concept reference
```

These relationships are stored as metadata in the deposits. PoC does not
need graph-native edges; simple ID references suffice.

## Handling user corrections vs new teaching

Two distinct flows:

**New teaching:** user explains something the agent didn't know. Simple
case, straightforward append.

**Correction:** user says "actually, what I said before was wrong, it's
really X." The correction deposit has:

- `source_type: user_correction`
- `trust_level: 1.0` (maximum — explicit correction is top trust)
- `origin.chain: [deposit_being_corrected]`
- Metadata: `corrects: deposit_id`

The corrected deposit is **not deleted** (append-only principle). It is
marked as `revised_by` pointing to the correction. Future recall returning
the original must surface the revision (CONFLICTED or STALE mode, depending).

## Test criteria

- [ ] UNKNOWN → user explains → same question next turn → ANSWER.
- [ ] UNKNOWN → user explains → same question next **session** → ANSWER.
      (Tests that learning persists beyond context window.)
- [ ] UNKNOWN → user explains with slightly different phrasing on recall
      → ANSWER. (Tests that recall doesn't require exact wording.)
- [ ] User correction → recall of corrected topic → CONFLICTED or STALE
      mode, surfaces both versions.
- [ ] After correction, the correction becomes the preferred path (higher
      trust, more recent reactivation).
- [ ] Agent never silently replaces corrected data; always surfaces the
      fact that it was corrected.

## Failure criteria

- Learning within session works but not across sessions. Indicates trie
  snapshotting or session state is broken.
- Learning works only with exact-phrase match on recall. Indicates
  tokenizer is too strict or context enrichment is too weak.
- Corrections silently overwrite (violates append-only). Bug, must fix.
- Agent can be "gaslit" — user tells it A, then tells it "you said B,
  not A," and agent accepts without surfacing the correction record.
  This would be a serious architectural flaw in revision handling.

## Open questions

1. Should UNKNOWN emissions themselves be recallable? If user asks "what
   have you not known recently?", can the agent answer? Proposed: yes —
   UNKNOWN deposits are just deposits with a specific source_type marker.

2. How does the agent distinguish "user is correcting me" from "user is
   explaining a different but similar concept"? Heuristic: explicit
   phrases like "actually," "no," "that's wrong," "let me correct" etc.
   PoC uses simple phrase matching; robust detection is post-PoC.

3. What if user teaches something that contradicts another user's earlier
   teaching? Since PoC is single-observer, this doesn't arise. Post-PoC
   with multiple observers, it becomes a real problem.

4. Should the agent be able to refuse to learn (e.g., reject obviously
   false teaching)? Proposed: no for PoC. The agent accepts what it's
   told. "Filter" layer is post-PoC.

## Non-goals

- Active learning (agent chooses what to ask about).
- Learning from web content without explicit user direction.
- Curriculum or pedagogy — the agent doesn't structure its learning;
  the user does by asking questions.
- Forgetting due to corrections. Append-only; history is never erased.

## Implementation note

This task is where the architecture's central claim is tested: can the
system **actually learn from conversation** or does it merely appear to?

LLM+vector-DB systems often fake this: they store the new info, retrieve
it on exact-match queries, but fail on paraphrased retrieval. If PoC also
fails on paraphrased retrieval, the whole incremental-learning claim
evaporates.

The primary risk is that tokenization + trie recall are **too literal** —
exact byte matches succeed, paraphrased matches fail. Mitigating factors:

1. Context window enrichment adds keywords that may catch paraphrases.
2. Hot_concepts overlay boosts recently-mentioned concepts.
3. Word-level trie (alongside byte-level) should catch some paraphrase.

If these three together cannot handle moderate paraphrase, the PoC fails
its central test, and the architecture needs a rethink (probably adding
an embedding layer for semantic similarity, which is exactly the thing
this architecture tries to avoid).

This is the test that matters most. Build everything else to support it.
