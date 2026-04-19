# Task 06 — Text Stream Renderer (Read Path)

**Status:** Design
**Depends on:** Tasks 04, 05
**Blocks:** Task 08

## Goal

Given a mode selection and supporting paths, produce natural-language
output for the user. The renderer is the agent's "mouth."

## Scope

**Text only** in PoC. Voice, visual, and other modalities are renderers
that would exist in the same slot but produce different output; the
architecture accommodates them, PoC does not implement them.

## Renderer responsibilities

1. Translate structured path data into coherent prose.
2. Respect the mode — each mode has a defined output shape.
3. Preserve provenance and confidence signals visible to the user when
   warranted (not always — confident ANSWER should not drown in caveats).
4. Ask back for DISAMBIGUATE / UNKNOWN with concrete, useful prompts.
5. Surface conflicts for CONFLICTED with enough information for the user
   to resolve.

## Per-mode output shape

### ANSWER

```
Format: direct statement, one or two paragraphs.
Provenance: optionally mentioned if trust < 0.8 or source is unusual.
Confidence: implied by tone; explicit hedging omitted.
```

Example:
> "You told me on 2026-04-15 that the experiment v24 showed 35× error on
> Mercury. The linear-in-1/r scaling across planets suggested it's not a
> float artifact but a geometric effect tied to the star's angular size."

### DISAMBIGUATE

```
Format: short question with 2-N concrete options.
Provenance: mention differentiating context ("the one from X" vs
            "the one from Y").
Confidence: explicit — "I'm not sure which you mean."
```

Example:
> "A couple of things match 'apple'. Do you mean the fruit we discussed
> in the multi-modal section, or Apple Inc. from the offline-first
> architecture note? The contexts are quite different."

### PARTIAL

```
Format: offer what exists, explicit uncertainty markers.
Provenance: cite sources if trust is mixed.
Confidence: explicit — "this is partial / weak signal."
```

Example:
> "I have a partial match: something about orbital anomalies and 1/r
> scaling, but the query only caught a piece. Can you narrow down — is
> this about v24 errors or something else you mentioned?"

### UNKNOWN

```
Format: honest admission + invitation to teach.
Provenance: N/A.
Confidence: explicit and primary — "I don't know this."
```

Example:
> "I don't know this yet. If you tell me, I'll remember it for next time.
> What do you mean by [term]?"

### STALE

```
Format: answer with temporal caveat.
Provenance: date of source emphasized.
Confidence: "I knew this, but it's old" — epistemic state is key.
```

Example:
> "My record from [date] says [content]. In this topic, things change
> fast, so this may no longer be current — you might want to re-confirm."

### CONFLICTED

```
Format: surface the conflict, both versions, ask which applies.
Provenance: both sources cited with timestamps.
Confidence: "I have contradictory records" — primary message.
```

Example:
> "I have two versions: on [date1] you said X, on [date2] you said Y,
> and I never got a resolution. Which is correct, or is one of them
> out of date?"

## Interface

```
render(
  mode_selection: ModeSelection,
  context: ContextWindow
) -> string
```

Pure function over its inputs. No trie access — all data needed is
in the mode_selection.

## Implementation approaches

Two viable implementations for PoC:

**A) Template-based (simpler, less flexible):**
For each mode, a template with slots filled from mode_selection data.
Predictable, testable, fast. Prose quality is limited.

**B) Small local LLM (more flexible):**
Mode_selection serialized as context, LLM generates response in the
shape defined by a per-mode system prompt. Better prose. Requires
keeping local LLM inference fast and cheap.

PoC starts with **A (templates)** for speed of implementation and
determinism in testing. B is a post-PoC upgrade.

## Test criteria

For each mode, at least one test case producing correctly-shaped output:

- [ ] ANSWER produces direct, non-hedging prose.
- [ ] DISAMBIGUATE produces a question with 2+ concrete options.
- [ ] PARTIAL explicitly flags uncertainty.
- [ ] UNKNOWN explicitly admits ignorance and invites teaching.
- [ ] STALE includes temporal markers.
- [ ] CONFLICTED surfaces both sides with sources.

Behavioral tests:

- [ ] Never produces fabricated detail not in supporting paths.
      (This is the hallucination test; central to PoC validity.)
- [ ] Prose does not omit required provenance when trust < threshold.
- [ ] Length is appropriate to mode (ANSWER can be short; CONFLICTED
      needs enough detail for user to resolve).

## Failure criteria

- Renderer fabricates content to fill template slots when data is missing.
  This is hallucination and breaks the whole proposition. If detected,
  must be fixed before PoC passes.
- Templates produce prose so mechanical that users prefer LLM-generated
  responses even when those hallucinate. Signal: user studies prefer
  alternative over PoC responses in >70% of cases.
- Provenance output is so verbose it drowns the answer.

## Open questions

1. How to balance prose naturalness with template rigor? Approach A
   limits naturalness; approach B risks hallucination. Hybrid (LLM with
   very constrained prompts) is possible but adds complexity.

2. How to render paths that came from non-text streams (e.g., a vision-
   origin deposit recalled during a text conversation)? Proposed:
   describe rather than reproduce. "I have a record from the camera
   stream at [time] showing [description]." Post-PoC, since PoC has
   only text.

3. Should the renderer have access to the confidence vector directly,
   or only through mode_selection? Proposed: only through mode_selection.
   Keeps renderer simple and testable.

4. Language of output — hardcoded, inferred from user, or explicit config?
   Proposed: inferred from recent user exchanges; default to English;
   match user's language in PoC via simple detection. (The user in this
   project speaks Czech; test renderers in both.)

## Non-goals

- Generation of audio, images, or any non-text output.
- Persona / style tuning (casual vs formal, etc.). Post-PoC.
- Multi-turn response planning. One query, one response.

## Implementation note

The renderer is where the agent's **honesty becomes perceptible to the
user**. If the renderer smooths over uncertainty — rephrases UNKNOWN as
"I'm not sure, but perhaps..." with fabricated content — the entire
architecture's value is lost.

**Hard rule for PoC:** When the mode is UNKNOWN or PARTIAL, the renderer
MUST NOT produce content that isn't in supporting_paths. No filler, no
educated guesses dressed as memory. Better a blunt "I don't know" than
a smooth lie.
