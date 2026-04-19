# Architectural Decisions

## ADR-001: Integer-only computation

**Decision:** No floating point numbers anywhere in the system.

**Rationale:** The core mechanism (same/different/unknown) is a classification, not a calculation. Token values are bytes (u8). Visit counts are integers (u64). Depth is an integer (u32). There is no operation in the system that requires continuous values. Floats would add complexity and precision issues for zero benefit.

## ADR-002: Append-only trie

**Decision:** Nodes and their children vectors only grow. No deletion, no modification of crystallized spectra.

**Rationale:** This is a fundamental property of the model, not just an implementation choice. The trie IS the accumulated history. Deleting or modifying nodes would lose causal information. Append-only also simplifies concurrency — readers never see inconsistent state because nothing is removed or changed.

## ADR-003: Raw bytes as initial token type

**Decision:** Start with u8 tokens (single bytes). Don't implement N-gram or wider token types in Phase 1.

**Rationale:** The Python experiments (v7) showed that N-gram windows discover more structure, but single bytes are sufficient to validate the mechanism. The trie should discover multi-byte structure through its own hierarchy, not through engineered token width. If this fails, N-gram support can be added later without changing the core API.

## ADR-004: MCP over stdio

**Decision:** Use stdio transport for the MCP server.

**Rationale:** Standard for local MCP servers. No network configuration needed. Claude can connect directly. SSE transport can be added later if remote access is needed.

## ADR-005: Simple crystallization

**Decision:** Spectrum = N most frequent values observed during learning phase. Crystallization triggers after a fixed observation count.

**Rationale:** The Python experiments used similar logic. More sophisticated crystallization (e.g., coverage-based, adaptive thresholds) can be explored later. The mechanism's power comes from the routing, not from clever crystallization.

## ADR-006: Linear child scan

**Decision:** When routing a Different token through children, scan children in creation order.

**Rationale:** First-created children represent the earliest-observed patterns. This gives them priority, which mirrors the trie's "early patterns are more fundamental" property. Performance is not a concern at current scale. If it becomes one, indexing children by spectrum can be added without API changes.
