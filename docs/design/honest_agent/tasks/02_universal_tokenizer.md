# Task 02 — Universal Change-Detector Tokenizer

**Status:** Design, speculative
**Depends on:** Task 01
**Blocks:** Tasks 04, 06, 07, 08

## Goal

A single tokenization mechanism that works across arbitrary input streams
by emitting deposits only when the stream's observable value changes beyond
a per-stream tolerance.

## Why universal

The claim: a meaningful structure in any sensory stream is carried by the
**moments when the signal changes state**, not by the continuous value. A
stable tone, a still image, an unchanging text buffer — these contribute
no new information. The edge (onset, boundary, delta) is the information.

If true, tokenization generalizes across modalities: the same "wait for
signal to settle, emit delta" loop handles text, audio, vision, sensor
streams, with only the per-stream value representation changing.

**Warning:** this is speculative. A sparse-delta tokenizer may produce too
few deposits for some streams (e.g., slowly evolving text), or too many for
others (e.g., noisy audio), and per-stream tolerance tuning may dominate the
design to the point where "universal" is technically true but practically
a collection of stream-specific configs.

## Interface

```
Tokenizer<Stream>:
  init(stream_config: StreamConfig) -> TokenizerState
  ingest(state, raw_input, wall_clock) -> (new_state, [emitted_deposits])
  flush(state) -> [final_deposits]  # on stream close / session end
```

`StreamConfig`:
```
{
  stream_id:        string,
  feature_extractor: FeatureExtractor,  # stream-specific
  tolerance:         Tolerance,          # stream-specific
  min_interval_ticks: int,               # rate-limit emission
  max_silence_ticks: int,                # force-emit if stable too long
}
```

`FeatureExtractor` is the only stream-specific plug-in:

```
FeatureExtractor:
  extract(raw_input) -> Feature
  distance(feature_a, feature_b) -> float
  describe(feature) -> serialized_content
```

For text: `extract` tokenizes into words; `distance` is e.g. edit distance
or something coarser; `describe` is the original string.

For audio: `extract` computes spectral frame; `distance` is spectral
divergence; `describe` is a summary (energy, dominant frequencies, or
phoneme hypothesis if ASR is available).

For sensor data: `extract` passes the vector through; `distance` is
Euclidean or Mahalanobis; `describe` is the numeric reading.

## Tokenization state machine

```
Same:        last_feature == new_feature within tolerance
Different:   last_feature != new_feature beyond tolerance, but pattern familiar
Unknown:     new_feature does not fit any previously-seen stable state
```

Emission rule:
- `Same` → no emission, reset silence counter, increment visit count on
  existing path.
- `Different` → emit deposit, record transition edge.
- `Unknown` → emit deposit, mark for later concept binding.

Rate limiting:
- No two deposits within `min_interval_ticks` of each other on the same
  stream. Coalesce rapid changes into a single deposit.
- If `Same` holds longer than `max_silence_ticks`, emit a "still here"
  heartbeat deposit. Prevents long-silent streams from being indistinguishable
  from closed streams.

## Per-stream tolerance as empirical knob

Tolerance is the primary empirical knob. Too low → every tiny variation
emits (noise). Too high → meaningful changes missed (loss).

PoC default for text: tolerance as "the token-level edit distance exceeds
3 or the new input contains a content word not in the previous window."
Chosen arbitrarily. To be tuned.

Other streams: no default; they don't exist in PoC.

## Integration with trie

On `Different` or `Unknown` emission:
- Build `deposit` via Task 01 schema.
- Call `trie_remember_v2` with appropriate source metadata.
- Store emitted deposit ID in tokenizer state for future `origin.chain`.

## Test criteria

- [ ] Text stream: typed conversation produces one deposit per user turn,
      not per keystroke, and rate-limit prevents double-emission on rapid
      edits.
- [ ] Repeated identical user input produces **no** new deposit (hits `Same`),
      but increments visit count on existing path.
- [ ] Entirely novel input produces a deposit tagged `Unknown`.
- [ ] Silence longer than `max_silence_ticks` produces heartbeat deposit.
- [ ] `flush` on session close emits any pending buffered state.

## Failure criteria

- Tokenizer requires so much per-stream tuning that the universal abstraction
  is hollow. Signal: >80% of configuration is stream-specific.
- Tokenizer with reasonable defaults produces deposit streams so sparse that
  recall becomes unreliable. Signal: test conversations <5 deposits / 100
  user turns on default text config.
- Tokenizer with reasonable defaults floods the trie. Signal: >50 deposits
  for a single user turn.

Any of these failing forces either a different tokenization model or
accepting that "universal" is a design aspiration, not an achievement.

## Open questions

1. Is the `Same / Different / Unknown` trichotomy from tick-frame theory
   implementable as a practical runtime distinction, or does it collapse
   to `Same / Different` in practice (with `Unknown` just being a first
   occurrence of a `Different`)?

2. Should the tokenizer have memory of the last N stable states (for
   comparison) or just the most recent? Proposed: last 1 for PoC, extend
   later if needed.

3. How does the tokenizer handle streams that are legitimately continuous
   (music, video motion)? Probably needs a "windowed summary" mode that
   emits at fixed intervals regardless of change. Deferred to post-PoC.

4. Can two streams be merged at the tokenizer level (e.g., synchronized
   audio+video), or must they stay separate and merge in the trie via
   shared tick anchor? Strongly prefer the latter for architectural
   consistency.

## Notes

This is the most uncertain component of the design. The entire "modality-
agnostic storage" claim hinges on tokenization being genuinely universal,
and that claim may not survive contact with real streams. PoC tests **only
text**, so this cannot be validated in PoC — only tested for non-failure
on the single modality.

For PoC, treat this as "text tokenizer with a universal-looking interface."
Validate universality in a follow-up that adds a second stream type.
