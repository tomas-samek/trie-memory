//! Context window (honest-agent Task 03).
//!
//! Holds a per-observer, per-session rolling buffer of recent exchanges and a
//! small set of hot concepts. Queries are enriched with these before being
//! routed to the trie, giving bare queries enough byte-mass to resonate
//! selectively against deeper paths (the empirical finding from the
//! 2026-04 short-vs-long-query experiments).
//!
//! Scope for PoC:
//! - Single in-process observer.
//! - Flat text enrichment (append recent topics and hot-concept labels as
//!   suffix). Structured enrichment is post-PoC.
//! - No relevance overlay yet; that's a Phase B concern.

use std::collections::VecDeque;

const MAX_EXCHANGES: usize = 10;
const MAX_HOT_CONCEPTS: usize = 5;
const RECENT_EXCHANGES_FOR_ENRICHMENT: usize = 3;

/// One turn of interaction: what the user asked, which topic labels surfaced
/// in the resulting recall, and the trie tick when the exchange happened.
#[derive(Debug, Clone)]
pub struct Exchange {
    pub user_input: String,
    pub topics: Vec<String>,
    pub tick: u64,
}

/// Result of enrichment. `raw` is what the user typed; `enriched` is what gets
/// sent to the trie query path. `topics_used` is exposed for diagnostics.
#[derive(Debug, Clone)]
pub struct EnrichedQuery {
    pub raw: String,
    pub enriched: String,
    pub topics_used: Vec<String>,
}

#[derive(Debug)]
pub struct ContextWindow {
    pub observer_id: Option<String>,
    pub session_id: String,
    exchanges: VecDeque<Exchange>,
    /// Most-recently-reactivated topic labels, MRU-ordered (front = newest).
    hot_concepts: Vec<String>,
}

impl ContextWindow {
    pub fn new(observer_id: Option<String>, session_id: String) -> Self {
        Self {
            observer_id,
            session_id,
            exchanges: VecDeque::with_capacity(MAX_EXCHANGES),
            hot_concepts: Vec::with_capacity(MAX_HOT_CONCEPTS),
        }
    }

    /// Enrich a raw query by appending recent-topic and hot-concept keywords
    /// as a flat space-separated suffix. Deterministic given the current
    /// context state.
    pub fn enrich(&self, raw: &str) -> EnrichedQuery {
        let mut topics_used: Vec<String> = Vec::new();
        for ex in self.exchanges.iter().rev().take(RECENT_EXCHANGES_FOR_ENRICHMENT) {
            for t in &ex.topics {
                if !topics_used.iter().any(|x| x == t) {
                    topics_used.push(t.clone());
                }
            }
        }
        for h in &self.hot_concepts {
            if !topics_used.iter().any(|x| x == h) {
                topics_used.push(h.clone());
            }
        }

        let enriched = if topics_used.is_empty() {
            raw.to_string()
        } else {
            format!("{} {}", raw, topics_used.join(" "))
        };

        EnrichedQuery {
            raw: raw.to_string(),
            enriched,
            topics_used,
        }
    }

    /// Record an exchange after a recall has completed. `topics` are the topic
    /// labels surfaced by the recall (empty for UNKNOWN outcomes). Promotes
    /// those topics into `hot_concepts` and trims both buffers.
    pub fn record_exchange(&mut self, user_input: &str, topics: Vec<String>, tick: u64) {
        self.exchanges.push_back(Exchange {
            user_input: user_input.to_string(),
            topics: topics.clone(),
            tick,
        });
        while self.exchanges.len() > MAX_EXCHANGES {
            self.exchanges.pop_front();
        }

        for t in topics {
            self.hot_concepts.retain(|x| x != &t);
            self.hot_concepts.insert(0, t);
        }
        while self.hot_concepts.len() > MAX_HOT_CONCEPTS {
            self.hot_concepts.pop();
        }
    }

    /// Start a new session: clear rolling state, optionally update session_id.
    pub fn reset(&mut self, new_session_id: Option<String>) {
        self.exchanges.clear();
        self.hot_concepts.clear();
        if let Some(s) = new_session_id {
            self.session_id = s;
        }
    }

    pub fn conversation_depth(&self) -> usize {
        self.exchanges.len()
    }

    pub fn hot_concepts(&self) -> &[String] {
        &self.hot_concepts
    }

    pub fn recent_exchanges(&self) -> impl Iterator<Item = &Exchange> {
        self.exchanges.iter()
    }
}

impl Default for ContextWindow {
    fn default() -> Self {
        Self::new(None, "default".to_string())
    }
}
