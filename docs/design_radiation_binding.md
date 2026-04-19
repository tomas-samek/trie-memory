# Design: Radiation, Buffers, and Concept Binding

**Status:** Design thinking — not ready to implement  
**Date:** April 7-8, 2026  
**Supersedes:** Parts of design_fibonacci_context.md (Fibonacci window is imposed, not emergent)

## Resolved: Fibonacci Doesn't Earn Its Existence

The Fibonacci window growth (1,1,2,3,5,8...) was imposed from outside. The trie's own mechanism produces linear context growth — each depth adds one byte. For Fibonacci to emerge naturally, each node would need two parents (combining window(N-1) + window(N-2)). The current tree structure has one parent per node.

**Rule: If a structural property can't emerge from the mechanism, don't impose it.**

## Resolved: Option B — Path IS Context

Each node stores only what it contributed. The full context is the path from root to this node. `classify()` stays unchanged — still `spectrum.contains(&token)`. The meaning comes from which path you took, not from what each node compares against.

Spectrum stays `Vec<u8>` at every depth. Crystallization stays the same.

## Core Problem: The Trie Can't Form Concepts

The trie classifies and stores. 'h' goes to one node, 'e' to another. But "he" as a concept doesn't exist anywhere. It's two nodes in a parent-child relationship. No mechanism says "these go together."

## Resolved: Radiation = Self-Advertisement

Every crystallized node radiates its spectrum — broadcasts "this is what I am." From the node's perspective, this is Same — sending what it knows. From a receiver's perspective, it arrives as Unknown.

Radiation is NOT rejection. A node broadcasts what it IS (spectrum), not what it couldn't handle.

## Resolved: Interference Creates Binding

Two radiation streams arriving at the same buffer create interference. If deposits share patterns, the buffer crystallizes around the overlap. The new entity represents what multiple sources have in common.

No node needs to know about any other node. Each just broadcasts "I am this." Concepts emerge in the space between — discovered by whatever buffer receives both streams.

## Resolved: Buffers ARE Connectors

There is no space. No medium. Just buffers with unprocessed deposits.

- Buffer from one source → child node (hierarchy)
- Buffer from two+ sources → connector node (binding)

Same mechanism. Same classify/observe/crystallize cycle. The tree becomes a DAG naturally through multi-parent crystallization.

## Resolved: When Does Radiation Fire?

When the node finishes processing its whole input. A completion event, not continuous emission.

Consequence: deeper nodes fire less frequently. Root fires every tick. Depth 3 fires every few ticks. **Radiation frequency is inversely proportional to depth.** This is time dilation from RAW 113 emerging naturally.

## Resolved: Timing Is Critical for Interference

For interference to create binding, two radiations must arrive at the same buffer within a close time window. This means:

- Concepts can only form between nodes that operate at similar time scales
- Fast nodes bind with fast nodes, slow with slow
- Character-level nodes bind into character concepts, phrase-level into phrase concepts
- No nonsensical cross-scale bindings

## Resolved: Radiation Goes to the Consumer

Radiation doesn't broadcast everywhere. That's a "space exists" artifact. There is no space. Radiation goes to the buffer/consumer it's connected to. The existing structure determines flow.

New connections only form when existing paths produce Unknown — nowhere else to go, new buffer appears.

## The Lateral Buffer Problem

### UNSOLVED: How does the first lateral buffer form?

In the current trie, all buffers are children of existing nodes. Node A's radiation goes to its children. Node B's radiation goes to its children. For interference, A and B's radiation must arrive at the SAME buffer. But no mechanism creates a shared buffer between unrelated nodes.

### Partial Answer: The Agent IS the Lateral Connector (April 8)

The lateral connection data already exists in the trie — it's encoded in time, not structure. When "hello" is written, 'h' is consumed at tick 100, 'e' at tick 101. They fire on consecutive ticks. The temporal proximity IS the co-occurrence signal. It's in the consumption logs already.

The key realization: **the agent reading the trie provides the lateral binding.**

```
Trie stores raw data (write)
    ↓
Agent reads patterns (perceive over time window)
    ↓
Agent notices: "these nodes fire together repeatedly"
    ↓
Agent writes concept BACK into trie (write)
    ↓
Trie stores concept → crystallizes → becomes a node
    ↓
New node participates in future perceive calls
    ↓
Agent discovers higher-order patterns...
```

The trie stays a tree. The agent provides lateral binding by reading and writing back. The "buffer between two unrelated nodes" is the agent's own processing.

### The Chicken-and-Egg Concern

This requires an "intelligent" agent to discover patterns. But:

1. The simplest agent is just a tick-window co-occurrence counter — not intelligence, a cron job
2. "Which nodes fired together in the last N ticks?" requires no understanding
3. Writing the discovery back is just `trie_write(pair_pattern)`
4. A thermostat-level agent might be sufficient to start

### Open: Does the Universe Need an Agent?

RAW 113 says binding should emerge from the mechanism itself. Quarks don't need an external observer to form atoms. If the trie requires an external agent for concept formation, it's a useful tool but not a model of the deeper mechanism.

**For now: "it's a tool" is fine.** The trie is an MCP server. The AI is the agent. Observe what happens. Maybe the answer to "how do lateral buffers form without an agent" reveals itself through usage, not theory.

## Architecture (What We Have)

One mechanism, applied everywhere:

```
1. Crystallized node RADIATES spectrum ("this is me")
2. Radiation deposits land in BUFFERS (connected Learning nodes)
3. Buffer CRYSTALLIZES when threshold reached
4. New entity RADIATES (cycle continues)
```

Plus the agent loop:

```
5. Agent PERCEIVES activation patterns across the trie
6. Agent WRITES discovered patterns back into the trie
7. New patterns become nodes that participate in future perception
```

Same/Different/Unknown at every step:
- Same → consume, add mass
- Different → radiate onward
- Unknown → buffer, eventually crystallize

## What NOT to Do Yet

- Don't implement radiation until lateral buffer creation is solved (or proven unnecessary)
- Don't add DAG support until binding is proven necessary from observation
- Don't impose Fibonacci or any non-emergent structure
- Don't build a complex agent — start with the dumbest possible co-occurrence counter

## Next Steps

1. **Use the trie.** Feed real conversation data. Observe structure.
2. **Build the dumb agent.** Tick-window co-occurrence detector. Write discoveries back.
3. **Observe what emerges.** Does the agent loop produce anything meaningful?
4. **Identify what's missing** from actual experience, not theory.
5. If lateral binding is needed, solve it then — informed by real observations.

---

*Conversations: April 7-8, 2026*
*References: RAW 113, RAW 112, trie_stream_filtering v4-v10, model-c v0.1-v0.14*
