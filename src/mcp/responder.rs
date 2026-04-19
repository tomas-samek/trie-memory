//! Mode selector + template renderer (honest-agent Task 06, minimal Phase A subset).
//!
//! Full spec distinguishes ANSWER / DISAMBIGUATE / PARTIAL / UNKNOWN / STALE /
//! CONFLICTED using a confidence vector. Phase A implements only two:
//!
//! - `Unknown`  — no recall hit, or only a root-suffix "fall-through" hit
//!                where the query shares no deep subtree ancestry with any
//!                recalled memory.
//! - `Answer`   — at least one memory shares a non-root suffix with the query
//!                path (real trie co-location, not just "every memory lives
//!                under the root").
//!
//! The renderer's hard rule: never produce content that isn't in a
//! `supporting_path`. For `Unknown` that means no fabricated answer; for
//! `Answer` it means the response quotes the stored content.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::context::{ContextWindow, EnrichedQuery};
use crate::store::memory::{MemoryEntry, SourceType};
use crate::trie::tokenizer::split_words;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseMode {
    Unknown,
    Answer,
    /// Some content-word overlap but below the strong-answer threshold.
    /// The renderer explicitly flags uncertainty.
    Partial,
    /// Multiple candidate memories tied on strength and coverage.
    /// The renderer asks the user which variant they mean.
    Disambiguate,
    /// A stored memory was superseded by an explicit user correction and
    /// both surface in the recall. The renderer shows both with dates so
    /// the user sees that a correction happened.
    Conflicted,
}

// Integer ppm thresholds (0..=1000). No floats per project rule.
//
// `ANSWER` requires two-thirds (666‰) query-word overlap. At 50% the
// stress test found false positives where a short question about a topic
// ("what color is a foo-widget") shares 2 of 4 words with the memory —
// same ratio as a legitimate "how does it work" query. Since word-coverage
// cannot distinguish "valid question" from "specific answer-present", the
// honest move is to demand more overlap before flipping to Answer and let
// Partial absorb the ambiguous middle.
const MIN_COVERAGE_ANSWER: u32 = 666; // ~2/3
const MIN_COVERAGE_PARTIAL: u32 = 200; // 20%
const MIN_CLARITY_FOR_ANSWER: u32 = 250; // 25% dominance required; else DISAMBIGUATE
const MIN_TRUST_LEGACY_ANSWER: u32 = 500; // legacy + low trust → PARTIAL
/// Arbitrary PoC default. Real deployments will tune per domain.
const RECENCY_HALFLIFE_TICKS: u64 = 100_000;

/// Multi-component confidence (Task 04). Each component is an integer
/// in ppm (0..=1000) unless noted. Components left at 0 when not
/// computable on the current Phase B surface (contradiction_flag is
/// Task 07 territory).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfidenceVector {
    /// Normalized strength of the top match. Derived from shared suffix
    /// depth relative to query path length.
    pub signal_strength: u32,
    /// Dominance of top candidate over the runner-up.
    /// 1000 = top dominates; 0 = tied with second.
    pub signal_clarity: u32,
    /// Density of the matched subgraph — `supporting.len() / max_results`.
    pub domain_density: u32,
    /// Trust-weighted mean of `trust_level` across supporting memories.
    pub source_trust: u32,
    /// Integer recency score based on age of last reactivation.
    pub recency: u32,
    /// Overlap fraction of query words present in the top memory.
    pub coverage: u32,
    /// Any supporting memory loaded from a legacy-schema snapshot.
    pub legacy_origin: bool,
    /// Reserved for Task 07: any revision/correction conflict detected.
    pub contradiction_flag: bool,
}

#[derive(Debug, Clone)]
pub struct ModeSelection<'a> {
    pub mode: ResponseMode,
    pub supporting: Vec<&'a MemoryEntry>,
    /// Depth of the deepest shared suffix between the query path and the
    /// top supporting memory. 0 = nothing in common, 1 = only root, >= 2 =
    /// genuine subtree match.
    pub top_shared_suffix: usize,
    /// Integer coverage in ppm (0-1000) for the top supporting memory.
    pub top_coverage: u32,
    /// Full diagnostic confidence vector.
    pub confidence: ConfidenceVector,
    pub reasoning: String,
}

/// Depth of the longest common trailing sequence (both slices are deepest-first,
/// so a common trailing sequence = shared ancestry from root down).
pub fn shared_suffix_depth(
    mem_path: &[Option<u64>],
    query_path: &[Option<u64>],
) -> usize {
    let mut n = 0;
    let mlen = mem_path.len();
    let qlen = query_path.len();
    while n < mlen && n < qlen {
        if mem_path[mlen - 1 - n] == query_path[qlen - 1 - n] {
            n += 1;
        } else {
            break;
        }
    }
    n
}

/// Tiny stopword filter. High-frequency English function words that add
/// noise to the coverage denominator without carrying topical content.
/// Conservative list — we want to keep meaningful verbs like "work", "is",
/// "make" out of scope only when they really don't discriminate.
fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "a" | "an" | "the" | "is" | "are" | "was" | "were" | "be" | "been"
            | "of" | "in" | "on" | "at" | "to" | "for" | "with" | "by"
            | "from" | "up" | "as" | "it" | "its" | "this" | "that"
            | "these" | "those" | "and" | "or" | "but" | "if" | "then"
            | "so" | "do" | "does" | "did" | "have" | "has" | "had"
            | "i" | "you" | "he" | "she" | "we" | "they" | "me" | "us"
            | "them" | "my" | "your" | "our" | "their" | "his" | "her"
    )
}

/// Strip a trailing ASCII 's' on words longer than 3 chars. Minimal
/// morphology fix — covers plurals ("streams" → "stream") and verbal
/// -s ("counts" → "count") without the weight of a real stemmer.
/// Doesn't touch non-ASCII text, so Czech/Japanese aren't mangled.
fn normalize_morphology(w: &str) -> String {
    if w.len() > 3 && w.is_ascii() && w.ends_with('s') && !w.ends_with("ss") {
        w[..w.len() - 1].to_string()
    } else {
        w.to_string()
    }
}

/// Strip leading and trailing non-alphanumeric characters (`?`, `!`, `,`,
/// `;`, `"`, etc.). `split_words` only breaks on whitespace plus a small
/// set of structural chars, so punctuation stays attached to tokens —
/// "semaphore?" would fail to match the stored word "semaphore" without
/// this pass.
fn strip_outer_punct(w: &str) -> String {
    let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
    trimmed.to_string()
}

fn normalize_words(raw: &str) -> HashSet<String> {
    split_words(raw)
        .into_iter()
        .map(|w| w.to_lowercase())
        .map(|w| strip_outer_punct(&w))
        .filter(|w| !w.is_empty())
        .filter(|w| !is_stopword(w))
        .map(|w| normalize_morphology(&w))
        .collect()
}

/// Word-level coverage: fraction of the query's distinct content-words that
/// appear in `memory_content`, returned in ppm (0-1000).
///
/// Normalization:
///   * case-insensitive (`split_words` + lowercase);
///   * stopwords dropped (no "the / is / a" in the denominator);
///   * trailing `-s` stripped on ASCII words longer than 3 chars
///     (plurals and verb-s agreement reach baseline parity).
pub fn coverage_ppm(query: &str, memory_content: &str) -> u32 {
    let qw = normalize_words(query);
    if qw.is_empty() {
        return 0;
    }
    let mw = normalize_words(memory_content);
    let matched = qw.iter().filter(|w| mw.contains(*w)).count() as u32;
    (matched * 1000) / (qw.len() as u32)
}

/// Compute per-memory combined score used for ranking + clarity.
/// Keeps suffix as the coarse bucket and coverage as the within-bucket
/// strength. Combined so that two memories with same suffix + same
/// coverage produce identical scores (→ clarity 0 → DISAMBIGUATE).
fn combined_score(suffix: usize, coverage_ppm: u32) -> u64 {
    (suffix as u64) * 10_000 + coverage_ppm as u64
}

/// Integer recency decay with halflife. age=0 → 1000; age=halflife → 500;
/// age=2*halflife → 250; etc. Clamped to [0,1000].
fn recency_ppm(age_ticks: u64) -> u32 {
    if age_ticks == 0 {
        return 1000;
    }
    let mut val: u64 = 1000;
    let mut remaining = age_ticks;
    while remaining >= RECENCY_HALFLIFE_TICKS && val > 0 {
        val /= 2;
        remaining -= RECENCY_HALFLIFE_TICKS;
    }
    // Linear within the last partial halflife.
    if val > 0 && RECENCY_HALFLIFE_TICKS > 0 {
        let decay_within = (val * remaining) / (2 * RECENCY_HALFLIFE_TICKS);
        val = val.saturating_sub(decay_within);
    }
    val.min(1000) as u32
}

/// Full confidence vector. `current_tick` is used only for recency; pass
/// the trie's current tick at query time.
pub fn compute_confidence<'a>(
    raw_query: &str,
    query_path: &[Option<u64>],
    memories: &'a [&'a MemoryEntry],
    max_results: usize,
    current_tick: u64,
) -> (ConfidenceVector, Vec<(usize, u32, u64, &'a MemoryEntry)>) {
    // Score everything: (suffix, coverage, combined, ref).
    let mut scored: Vec<(usize, u32, u64, &MemoryEntry)> = memories
        .iter()
        .map(|m| {
            let suf = shared_suffix_depth(&m.path_content_ids, query_path);
            let cov = coverage_ppm(raw_query, &m.content);
            (suf, cov, combined_score(suf, cov), *m)
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then(b.3.timestamp.cmp(&a.3.timestamp))
    });

    if scored.is_empty() {
        return (ConfidenceVector::default(), scored);
    }

    let (top_suf, top_cov, top_score, top_mem) = scored[0];

    // signal_strength: shared-suffix depth scaled by query path length.
    let path_len = query_path.len().max(1) as u32;
    let signal_strength = ((top_suf as u32) * 1000 / path_len).min(1000);

    // signal_clarity: dominance of the top over the runner-up.
    // Rule:
    //   * If the runner-up has a *shallower* suffix than the top, clarity is
    //     already 1000 — different buckets, no ambiguity.
    //   * If same suffix, clarity is measured on coverage gap. This is the
    //     signal we actually want — within the same trie bucket, is one
    //     memory clearly the better content match?
    let signal_clarity: u32 = if scored.len() < 2 {
        1000
    } else {
        let (second_suf, second_cov, _, _) = scored[1];
        if second_suf < top_suf {
            1000
        } else if top_cov == 0 {
            if second_cov == 0 { 0 } else { 0 }
        } else {
            let ratio = ((second_cov as u64) * 1000) / top_cov as u64;
            1000u32.saturating_sub(ratio.min(1000) as u32)
        }
    };
    let _ = top_score; // retained in signature for future use

    // domain_density: how many of the recalled memories tied on suffix
    // at the top depth, normalized against max_results.
    let same_suffix = scored.iter().filter(|s| s.0 == top_suf).count() as u32;
    let domain_density = if max_results == 0 {
        0
    } else {
        ((same_suffix * 1000) / max_results as u32).min(1000)
    };

    // source_trust: mean trust_level across the supporting cluster
    // (memories tied at top suffix).
    let trust_values: Vec<u32> = scored
        .iter()
        .filter(|s| s.0 == top_suf)
        .map(|s| s.3.trust_level)
        .collect();
    let source_trust = if trust_values.is_empty() {
        0
    } else {
        let sum: u32 = trust_values.iter().sum();
        sum / trust_values.len() as u32
    };

    // recency: age of top memory's last reactivation.
    let (_tick_start, tick_end) = top_mem.tick_range;
    let age = current_tick.saturating_sub(tick_end);
    let recency = recency_ppm(age);

    let legacy_origin = scored.iter().filter(|s| s.0 == top_suf).any(|s| s.3.legacy_origin);

    // contradiction_flag: true when two scored memories reference each other
    // via revised_by / origin.corrects AND both land in the top suffix bucket.
    // This is the Task 07 conflict signal that routes to Conflicted.
    let contradiction_flag = detect_contradiction(&scored, top_suf);

    let vec = ConfidenceVector {
        signal_strength,
        signal_clarity,
        domain_density,
        source_trust,
        recency,
        coverage: top_cov,
        legacy_origin,
        contradiction_flag,
    };
    (vec, scored)
}

fn detect_contradiction(
    scored: &[(usize, u32, u64, &MemoryEntry)],
    top_suf: usize,
) -> bool {
    let top_bucket: Vec<&MemoryEntry> = scored
        .iter()
        .filter(|s| s.0 == top_suf)
        .map(|s| s.3)
        .collect();
    for a in &top_bucket {
        if let Some(corrector_id) = a.revised_by {
            if top_bucket.iter().any(|b| b.id == corrector_id) {
                return true;
            }
        }
        if let Some(o) = a.origin.as_ref() {
            if let Some(target_id) = o.corrects {
                if top_bucket.iter().any(|b| b.id == target_id) {
                    return true;
                }
            }
        }
    }
    false
}

/// Task 05 cascade (Phase B subset — STALE and CONFLICTED deferred until
/// volatility classifier and revision layer exist).
///
/// Decision order:
/// 1. No memories or root-only suffix → Unknown.
/// 2. Coverage below PARTIAL floor → Unknown (word-coincidence guard).
/// 3. Legacy origin with low source trust → Partial (never Answer).
/// 4. Clarity below threshold with multiple real candidates → Disambiguate.
/// 5. Coverage below Answer threshold → Partial.
/// 6. Otherwise → Answer.
pub fn select_mode<'a>(
    raw_query: &str,
    query_path: &[Option<u64>],
    memories: &'a [&'a MemoryEntry],
) -> ModeSelection<'a> {
    select_mode_with_tick(raw_query, query_path, memories, memories.len().max(5), 0)
}

/// Full version — callers wanting recency and domain_density accurate pass
/// `max_results` (same as passed to recall) and `current_tick` (trie tick).
pub fn select_mode_with_tick<'a>(
    raw_query: &str,
    query_path: &[Option<u64>],
    memories: &'a [&'a MemoryEntry],
    max_results: usize,
    current_tick: u64,
) -> ModeSelection<'a> {
    if memories.is_empty() {
        return ModeSelection {
            mode: ResponseMode::Unknown,
            supporting: Vec::new(),
            top_shared_suffix: 0,
            top_coverage: 0,
            confidence: ConfidenceVector::default(),
            reasoning: "no memories recalled".to_string(),
        };
    }

    let (confidence, scored) =
        compute_confidence(raw_query, query_path, memories, max_results, current_tick);
    let (top_suf, top_cov, _, _) = scored[0];

    // Supporting cluster = memories tied at top suffix depth that also clear
    // the partial-coverage floor. Without the coverage filter, path-suffix
    // siblings with zero word overlap show up in "Also matched" and mislead
    // the user (Phase A / B step-1 finding E.1).
    let supporting: Vec<&MemoryEntry> = scored
        .iter()
        .filter(|(s, c, _, _)| *s == top_suf && *c >= MIN_COVERAGE_PARTIAL)
        .map(|(_, _, _, m)| *m)
        .collect();

    // 1. Explicit revision conflict — fires before path-fall-through rules.
    // A correction is a first-class signal: we must never silently hide a
    // revision even when the trie routing for the current query is weak.
    if confidence.contradiction_flag {
        let conflict_cluster: Vec<&MemoryEntry> = scored
            .iter()
            .filter(|(_, c, _, m)| {
                *c >= MIN_COVERAGE_PARTIAL
                    && (m.revised_by.is_some()
                        || m.origin.as_ref().and_then(|o| o.corrects).is_some())
            })
            .map(|(_, _, _, m)| *m)
            .collect();

        // Also pull in any entry directly referenced by a correction so the
        // user sees both sides even if one lands with weaker coverage.
        let mut full: Vec<&MemoryEntry> = conflict_cluster.clone();
        for m in &conflict_cluster {
            if let Some(tid) = m.origin.as_ref().and_then(|o| o.corrects) {
                if let Some(target) = scored.iter().map(|s| s.3).find(|e| e.id == tid) {
                    if !full.iter().any(|x| x.id == target.id) {
                        full.push(target);
                    }
                }
            }
            if let Some(rid) = m.revised_by {
                if let Some(target) = scored.iter().map(|s| s.3).find(|e| e.id == rid) {
                    if !full.iter().any(|x| x.id == target.id) {
                        full.push(target);
                    }
                }
            }
        }

        return ModeSelection {
            mode: ResponseMode::Conflicted,
            supporting: full,
            top_shared_suffix: top_suf,
            top_coverage: top_cov,
            confidence: confidence.clone(),
            reasoning: "revision chain detected — surfacing both versions".to_string(),
        };
    }

    // 2. Root-only fall-through.
    if top_suf < 2 {
        return ModeSelection {
            mode: ResponseMode::Unknown,
            supporting: Vec::new(),
            top_shared_suffix: top_suf,
            top_coverage: top_cov,
            confidence,
            reasoning: format!(
                "query only shares {} segment(s) with any memory (root-only fall-through)",
                top_suf
            ),
        };
    }

    // 3. Word-coincidence guard.
    if top_cov < MIN_COVERAGE_PARTIAL {
        return ModeSelection {
            mode: ResponseMode::Unknown,
            supporting: Vec::new(),
            top_shared_suffix: top_suf,
            top_coverage: top_cov,
            confidence,
            reasoning: format!(
                "path matches but coverage {}‰ is below {}‰ — treating as unknown",
                top_cov, MIN_COVERAGE_PARTIAL
            ),
        };
    }

    // 3. Legacy + low trust → demote to Partial regardless of coverage.
    if confidence.legacy_origin && confidence.source_trust < MIN_TRUST_LEGACY_ANSWER {
        return ModeSelection {
            mode: ResponseMode::Partial,
            supporting,
            top_shared_suffix: top_suf,
            top_coverage: top_cov,
            confidence: confidence.clone(),
            reasoning: format!(
                "legacy_origin and source_trust {}‰ below {}‰ — partial",
                confidence.source_trust, MIN_TRUST_LEGACY_ANSWER
            ),
        };
    }

    // 4. Two or more genuine candidates tied on score → ask which.
    let tied_candidates = scored
        .iter()
        .filter(|(s, c, _, _)| *s == top_suf && *c >= MIN_COVERAGE_PARTIAL)
        .count();
    if tied_candidates >= 2 && confidence.signal_clarity < MIN_CLARITY_FOR_ANSWER {
        return ModeSelection {
            mode: ResponseMode::Disambiguate,
            supporting,
            top_shared_suffix: top_suf,
            top_coverage: top_cov,
            confidence: confidence.clone(),
            reasoning: format!(
                "clarity {}‰ below {}‰ with {} tied candidates — disambiguate",
                confidence.signal_clarity, MIN_CLARITY_FOR_ANSWER, tied_candidates
            ),
        };
    }

    // 5. Moderate coverage → Partial.
    if top_cov < MIN_COVERAGE_ANSWER {
        return ModeSelection {
            mode: ResponseMode::Partial,
            supporting,
            top_shared_suffix: top_suf,
            top_coverage: top_cov,
            confidence,
            reasoning: format!(
                "suffix={} coverage={}‰ — partial (below {}‰ answer threshold)",
                top_suf, top_cov, MIN_COVERAGE_ANSWER
            ),
        };
    }

    // 6. Answer.
    let clarity_for_msg = confidence.signal_clarity;
    ModeSelection {
        mode: ResponseMode::Answer,
        supporting,
        top_shared_suffix: top_suf,
        top_coverage: top_cov,
        confidence,
        reasoning: format!(
            "suffix={} coverage={}‰ clarity={}‰ — answer threshold met",
            top_suf, top_cov, clarity_for_msg
        ),
    }
}

/// Render a mode selection as plain text. Templates only — no model calls,
/// no generation. Content for `Answer` comes verbatim from the supporting
/// memories. `Unknown` never fabricates.
pub fn render(
    mode_selection: &ModeSelection<'_>,
    raw_query: &str,
    context: &ContextWindow,
) -> String {
    match mode_selection.mode {
        ResponseMode::Unknown => render_unknown(raw_query, context),
        ResponseMode::Answer => render_answer(mode_selection, context),
        ResponseMode::Partial => render_partial(mode_selection, context),
        ResponseMode::Disambiguate => render_disambiguate(mode_selection, context),
        ResponseMode::Conflicted => render_conflicted(mode_selection, context),
    }
}

fn render_conflicted(mode_selection: &ModeSelection<'_>, _context: &ContextWindow) -> String {
    // Sort so the correction (the one that *corrects* something else, or is
    // referenced by revised_by chains) shows up as the current version.
    let mut ordered: Vec<&MemoryEntry> = mode_selection.supporting.iter().copied().collect();
    ordered.sort_by_key(|m| std::cmp::Reverse(m.timestamp));

    let mut out = String::from(
        "I have conflicting records and never got a resolution. Which is correct?",
    );
    for m in ordered.iter().take(4) {
        let topic = m.topic.as_deref().unwrap_or("(no topic)");
        let snippet: String = m.content.chars().take(120).collect();
        let marker = if m.origin.as_ref().and_then(|o| o.corrects).is_some() {
            " (marked as correction)"
        } else if m.revised_by.is_some() {
            " (superseded)"
        } else {
            ""
        };
        out.push_str(&format!(
            "\n  - [{} @ ts={}]{} {}",
            topic, m.timestamp, marker, snippet
        ));
    }
    out
}

fn render_disambiguate(mode_selection: &ModeSelection<'_>, _context: &ContextWindow) -> String {
    let mut out = String::from(
        "I have more than one match that could apply. Which one did you mean?",
    );
    for m in mode_selection.supporting.iter().take(4) {
        let topic = m.topic.as_deref().unwrap_or("(no topic)");
        let snippet: String = m.content.chars().take(100).collect();
        out.push_str(&format!("\n  - [{}] {}", topic, snippet));
    }
    out
}

fn render_unknown(raw_query: &str, context: &ContextWindow) -> String {
    let session_ref = context
        .observer_id
        .as_deref()
        .map(|o| format!(" ({})", o))
        .unwrap_or_default();
    format!(
        "I don't have this stored{session_ref}. You asked: {:?}. If you tell me, I'll remember it for next time.",
        raw_query
    )
}

fn render_answer(mode_selection: &ModeSelection<'_>, _context: &ContextWindow) -> String {
    let top = mode_selection.supporting.first().expect("Answer must have supporting memory");
    let mut out = String::new();

    // Lead: topic + provenance if non-default.
    let topic_prefix = top
        .topic
        .as_deref()
        .map(|t| format!("[{}] ", t))
        .unwrap_or_default();

    let source_tag = match top.source_type {
        SourceType::UserDirect => " (from you)",
        SourceType::UserCorrection => " (from your correction)",
        SourceType::AgentInference => " (agent-synthesized)",
        SourceType::SensorDirect => " (sensor reading)",
        SourceType::WebFetched => " (from the web — lower trust)",
        SourceType::ToolResult => " (tool output — lower trust)",
        SourceType::MemoryRecall => " (recalled from earlier)",
        SourceType::Unknown => "",
    };

    let legacy_note = if top.legacy_origin {
        " [legacy entry, no provenance]"
    } else {
        ""
    };

    out.push_str(&format!(
        "{topic_prefix}{}{source_tag}{legacy_note}",
        top.content
    ));

    // If multiple memories tie on suffix depth, mention them briefly.
    if mode_selection.supporting.len() > 1 {
        out.push_str("\n\nAlso matched:");
        for m in mode_selection.supporting.iter().skip(1).take(2) {
            let snippet: String = m.content.chars().take(80).collect();
            let t = m.topic.as_deref().unwrap_or("untagged");
            out.push_str(&format!("\n  - [{}] {}…", t, snippet));
        }
    }

    out
}

fn render_partial(mode_selection: &ModeSelection<'_>, _context: &ContextWindow) -> String {
    let top = mode_selection
        .supporting
        .first()
        .expect("Partial must have supporting memory");
    let topic_prefix = top
        .topic
        .as_deref()
        .map(|t| format!("[{}] ", t))
        .unwrap_or_default();
    let mut out = String::new();
    out.push_str(&format!(
        "Partial match — I have related content but not a confident answer. {}{}",
        topic_prefix, top.content
    ));
    out.push_str(&format!(
        "\n\n(word overlap {}‰; you may need to ask more specifically.)",
        mode_selection.top_coverage
    ));
    out
}

/// Compute the query path once and pull the raw query out of the enriched form.
/// Convenience for the tool layer.
pub fn decompose_query(q: &EnrichedQuery) -> (&str, &str) {
    (&q.raw, &q.enriched)
}
