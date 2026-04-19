# Task 01 — Deposit Schema Extension

**Status:** Design
**Depends on:** existing trie-memory
**Blocks:** Tasks 02–08

## Goal

Extend the existing `trie_remember` deposit structure with provenance, source
type, and trust metadata so every stored event carries enough context to be
trusted or distrusted at recall time.

## Why now

Two observations from 2026-04 sessions:

1. Trie currently stores `content + topic + timestamp + depth_profile`. No
   origin, no trust, no source discrimination.

2. When a `<note>` prompt-injection artifact entered chat context from the UI,
   detection was only possible because the *conversation* context gave Claude
   enough signal to judge it against expected behavior. If that artifact had
   entered the trie directly, later recall would have returned it as trusted
   content indistinguishable from user-authored memory.

Without provenance, stored memory **is** an attack surface.

## Proposed deposit schema

```
deposit = {
  # Existing
  content:        string,
  topic:          string | null,
  timestamp:      int64 (tick),
  depth_profile:  [...],

  # New: provenance core
  observer_id:    string,         # who perceived this
  stream_id:      string,         # which input channel
  source_type:    enum,           # see taxonomy below
  trust_level:    float [0.0, 1.0],

  # New: provenance chain
  origin: {
    author:       string | null,  # named entity if known
    captured_at:  int64 (wall clock seconds),
    via:          string,         # "direct", "mcp:xxx", "web:url", ...
    chain:        [prior_deposit_ids]  # if derived from other deposits
  },

  # New: optional signals
  modality:       string | null,   # "text", "audio", "vision", "sensor:imu", ...
  language:       string | null,   # for text streams
  confidence:     float | null,    # source-reported confidence if any
}
```

## Source type taxonomy (initial)

| source_type           | default trust | notes |
|-----------------------|---------------|-------|
| `user_direct`         | 0.9           | typed by identified user in active session |
| `user_correction`     | 1.0           | explicit "the previous X was wrong, it's Y" |
| `agent_inference`     | 0.5           | agent's own synthesis from other deposits |
| `sensor_direct`       | 0.8           | raw sensor reading with known calibration |
| `web_fetched`         | 0.4           | content pulled from external URL |
| `tool_result`         | 0.3           | result from MCP / tool invocation |
| `memory_recall`       | inherit       | reconstructed from earlier deposit |
| `unknown`             | 0.1           | source cannot be determined |

Trust is **default only** — real trust is computed at recall time as
`base_trust × domain_familiarity × recency_factor × corroboration_bonus`.

## Interface

Extend `trie-memory` MCP with:

```
trie_remember_v2(
  content: str,
  topic: Optional[str],
  observer_id: str,
  stream_id: str,
  source_type: SourceType,
  trust_level: Optional[float],   # defaults from source_type
  origin: OriginBlock,
  modality: Optional[str],
  language: Optional[str],
  confidence: Optional[float]
) -> DepositId
```

Existing `trie_remember` stays as v1 wrapper that fills defaults:
- `observer_id = "legacy"`
- `source_type = "unknown"`
- `trust_level = 0.1`
- `origin.via = "legacy_remember"`

Migration path: existing deposits are readable but flagged as legacy-origin
during recall. They are **not silently upgraded**; any recall drawing from
legacy deposits carries a `legacy_origin` flag in the confidence vector.

## Test criteria

- [ ] `trie_remember_v2` accepts all new fields and persists them.
- [ ] `trie_recall` returns new fields alongside content.
- [ ] Legacy deposits are recallable and flagged `legacy_origin`.
- [ ] Trust_level default works from source_type if trust is omitted.
- [ ] Source_type is a validated enum — unknown values rejected.
- [ ] Round-trip through `trie_snapshot` / `trie_restore` preserves metadata.

## Non-goals

- Cryptographic signing of provenance. Future work. PoC uses declared origin
  on trust; this is sufficient for single-user single-agent scenario.
- Automatic provenance inference when the calling code does not supply it.
  The caller must be explicit. Unspecified source = `unknown` = very low trust.
- Graph-level trust propagation. If deposit A is built from B+C, it inherits
  via `origin.chain`, but computed trust merging is a separate task (post-PoC).

## Open questions

1. Should `trust_level` be mutable after write, or always append as revision?
   Proposed: immutable at the deposit level; revisions create new deposits
   in the Revision Layer (Task 07).

2. `observer_id` — global or per-session? Proposed: global, with session as
   a separate context concept. A user's memories persist across sessions.

3. For deposits derived from other deposits (`origin.chain`), is the stored
   content the derivation output or a link to the sources? Proposed: both —
   content is the synthesis, chain lets audit trace it back.

## Failure criteria

This task fails if:

- Adding metadata causes measurable slowdown of `trie_remember` beyond
  acceptable (PoC threshold: < 2x write time).
- Snapshot/restore drops any field.
- The taxonomy proves insufficient within first 100 real deposits and
  requires redesign rather than extension.

## Provenance note

This task exists because of a concrete near-miss documented in session
transcripts 2026-04 (the `<note>` injection incident). It is not speculative;
the attack surface is demonstrated.
