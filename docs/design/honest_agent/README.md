# Honest Agent over Trie — Design Directory

Speculative architecture for an agent built over the existing trie-memory.
PoC scope: single-observer, text-only, proof-of-concept.

**Read first:** [`00_ARCHITECTURE.md`](00_ARCHITECTURE.md) — overview,
scope, and critical disclaimers. In particular, the epistemic disclaimer:
this architecture is internally consistent but not empirically validated.

## Structure

```
00_ARCHITECTURE.md         Design overview, scope, failure criteria
tasks/
  01_deposit_schema.md     Extended deposit structure with provenance
  02_universal_tokenizer.md Change-based tokenization, speculative
  03_context_window.md     Observer state & query enrichment
  04_confidence_vector.md  Multi-component confidence, not scalar
  05_mode_selector.md      ANSWER / DISAMBIGUATE / UNKNOWN / STALE / ...
  06_renderer.md           Text read-path, template-based for PoC
  07_incremental_learning.md  The loop that closes; central PoC claim
  08_test_harness.md       Five required scenarios, pass/fail criteria
```

## Reading paths

**Skeptic:**
1. `00_ARCHITECTURE.md` — epistemic disclaimer + failure criteria.
2. `08_test_harness.md` — what must pass for the thing to be real.
3. Tasks 02 and 07 — the two most speculative components.

**Implementer:**
1. `00_ARCHITECTURE.md` — scope & layering.
2. Tasks in order 01 → 08.

**Critic reviewing design:**
1. `00_ARCHITECTURE.md`
2. Open questions and failure criteria in each task.
3. Look for where "speculation" is acknowledged vs hidden.

## Relationship to broader project

This design sits on top of the existing `trie-memory` MCP server. It does
not modify the core trie; it extends the deposit schema (Task 01) and
adds layers above for tokenization, recognition, and response.

It is **orthogonal to** the tick-frame physics work. Both projects share
the same underlying ontology (deposits, resonance, ticks, append-only
history, observer-relative reconstruction), but proving one does not
prove the other. The honest-agent PoC has much lower stakes: if it works,
it's a useful tool; if it doesn't, the physics model is unaffected.

## Status (2026-04-19)

- [x] Architecture draft complete (this directory)
- [ ] Task 01 — deposit schema extension
- [ ] Task 02 — universal tokenizer
- [ ] Task 03 — context window
- [ ] Task 04 — confidence vector
- [ ] Task 05 — mode selector
- [ ] Task 06 — text renderer
- [ ] Task 07 — incremental learning loop
- [ ] Task 08 — test harness execution
- [ ] PoC pass/fail decision

Implementation order is flexible but Tasks 01–03 unblock the rest.

## Provenance

Architecture emerged from dialog sessions in April 2026 between
jerry-samek and Claude (Anthropic). The human originated the ontology
and intuitions; Claude formalized, stress-tested, and drafted these
documents. See architecture's own provenance section for detail.
