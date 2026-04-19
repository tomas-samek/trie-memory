# Design: Inherited Context Windows (Fibonacci N-gram)

**Status:** Design thinking — not ready to implement yet  
**Date:** April 7, 2026

## The Problem

Byte-level classification produces a flat hierarchy. English uses ~70 distinct bytes. Root (spectrum=35) and depth 1 (spectrum=30) cover the entire alphabet. Nothing meaningful reaches depth 2+. The trie can't learn words, phrases, or structure — only character frequencies.

## The Insight

Each depth level should look at a *wider window* of the stream. But not by re-reading bytes — by **inheriting what its parent already classified** and extending it.

The atom doesn't re-discover quarks. It inherits that classification.

## Fibonacci Window Growth

Window width at each depth follows the Fibonacci sequence:

```
depth:  0   1   2   3   4   5    6    7    8
width:  1   1   2   3   5   8   13   21   34
```

What each depth "sees":

```
depth 0: 1 byte   → "e"                          (character)
depth 1: 1 byte   → "e" (same width, smaller spectrum — refines)
depth 2: 2 bytes  → "ed"                         (pair)
depth 3: 3 bytes  → "edh"                        (morpheme)
depth 4: 5 bytes  → "edh'a"                      (short word)
depth 5: 8 bytes  → "edh'ama "                   (word + boundary)
depth 6: 13 bytes → "from edh'ama"               (phrase)
depth 7: 21 bytes → "distance from edh'ama"      (clause)
depth 8: 34 bytes → sentence fragment             (sentence)
```

Why Fibonacci over powers of 2: gentle growth. Powers of 2 reach 256 at depth 8 (paragraph-sized tokens — nothing matches). Fibonacci reaches 34 (sentence — realistic).

Why Fibonacci over linear: it accelerates naturally. Early depths make fine distinctions (1,1,2,3). Later depths make broader compositional leaps (8,13,21,34). This matches how composition works — letters combine into short morphemes (small step), morphemes into words (medium step), words into phrases (bigger step).

## Option C: Inherited Context

A node at depth N does NOT re-read bytes from the stream. It inherits context from its parent chain and extends it.

### How it works

When a token is being routed from root toward leaves:

1. **Root (depth 0):** sees 1 byte, classifies it. Passes classification result + the byte to child.
2. **Depth 1:** sees 1 byte (the same one — width is still 1), classifies with a narrower spectrum. Passes both bytes' context to child.
3. **Depth 2:** sees 2 bytes (the current byte + 1 byte of inherited context from parent). Classifies.
4. **Depth 3:** sees 3 bytes (current + 2 inherited).
5. **Depth N:** sees fib(N) bytes (current + inherited context from the path above).

The "token" at each depth is assembled from the classification path, not re-read from the stream.

### What gets inherited

This is the key design question. Options:

**A. Raw bytes accumulate:** Each depth passes down the raw byte values. Depth 2 literally sees `[104, 101]` = "he". Simple. But the spectrum comparison becomes a multi-byte lookup.

**B. Classification results accumulate:** Each depth passes down {same, different, unknown}. Depth 2 sees "root said same, depth 1 said different". The token is a *path through classification space*, not raw bytes. Ternary encoding: 3^N possible paths at depth N.

**C. Hybrid:** Pass down the raw bytes for spectrum matching, but also the classification path for routing decisions.

### The timing problem

A node at depth 3 needs a 3-byte window. Where do the 3 bytes come from?

**Option 1: Consecutive stream bytes.** The node buffers 3 consecutive bytes from the stream, then classifies once. Problem: depth 8 waits 34 ticks between classifications. Slow.

**Option 2: Sliding window.** Every tick, classify using the last fib(N) bytes. Problem: overlapping windows create redundancy.

**Option 3: The parent provides it.** This is option C. The byte flows down from root. Root processes it and passes it plus context to child. Child processes using its wider window (parent's context + current byte). This means:
- Every tick, root classifies one byte
- Root's child gets the result and builds a 1-byte context
- Every time the child accumulates fib(depth) bytes of context, IT classifies
- Its child accumulates further...

So deeper nodes classify less frequently. Root classifies every tick. Depth 1 also every tick (width=1). Depth 2 every 2 ticks. Depth 3 every 3 ticks. Depth 8 every 34 ticks.

**This is natural.** Big bang ticks fast. Stars tick slower. Planets slower still. Depth IS time scale.

### Connection to tick-frame-space theory

This maps directly to:
- RAW 113: depth = time (branch depth is the only clock)
- v8 causal window: deeper entities learn from longer observation windows
- The universe metaphor: big bang sees everything, galaxies see categories, stars see atoms, planets see molecules

### Connection to visit_count as mass

Deeper nodes classify less frequently, so they accumulate visits slower. But when they DO match, it means a *longer pattern* repeated. A visit at depth 8 (34-byte match) is far more meaningful than a visit at depth 0 (single byte match). Mass at depth = pattern significance.

## Open Questions

1. **What exactly is the "spectrum" at depth 2+?** At depth 0, spectrum = list of known byte values. At depth 2, spectrum = list of known byte PAIRS? That's a Vec<Vec<u8>> instead of Vec<u8>. Or is it a hash of the pair?

2. **How does crystallization work for multi-byte tokens?** Counting frequency of byte pairs is much sparser than counting single bytes. Threshold may need to scale differently.

3. **Memory cost.** Multi-byte spectra are larger. A depth 8 spectrum entry is 34 bytes. If spectrum_size=4 at that depth, that's 136 bytes per node. Still small, but grows.

4. **Should classification at depth N consider the FULL inherited context, or just the NEW bytes?** If depth 2 sees "he", does it compare "he" against its spectrum? Or does it only look at "e" knowing that root already approved "h"?

5. **The Fibonacci redundancy at depths 0 and 1 (both width=1).** Is this a feature (two-pass filtering of single bytes) or a waste? Could depth 1 start at width=2? (That would be: 1,2,3,5,8,13... — Lucas-like sequence.)
a\
6. **How does `perceive()` change?** Multi-leaf activation still works — walk leaf to root, aggregate hits. But the meaning of "hit" now depends on depth (depth 8 hit = 34-byte pattern matched).

## Recommendation

Don't implement yet. The spectrum representation for multi-byte tokens is the critical design decision. Get that wrong and the whole thing needs rewriting. Think about question 1 and 4 first.

The simplest starting point might be: **spectrum at depth N = set of fib(N)-byte sequences this node recognizes.** Just Vec<Vec<u8>> instead of Vec<u8>. Crystallization = find most common fib(N)-grams in the buffer. Everything else stays the same.

But that might be naive. The point of inherited context is that you DON'T need to store the full N-gram — you already know the prefix was classified by your ancestors. You only need to store what YOU added. That's more elegant but harder to implement.

## References

- v7: N-gram stream filtering (proved N-gram depth discovers structure)
- v8: Causal window (deeper entities learn better with longer observation)
- RAW 113 §4: Branch depth is time
- This conversation: April 7, 2026
