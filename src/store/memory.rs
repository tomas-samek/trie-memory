use serde::{Deserialize, Serialize};

/// Provenance source class for a deposit. Default trust levels are assigned by
/// `source_type_default_trust` but callers may override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// Typed by an identified user in an active session.
    UserDirect,
    /// Explicit correction of a prior deposit ("actually, X was wrong, it's Y").
    UserCorrection,
    /// Agent's own synthesis from other deposits.
    AgentInference,
    /// Raw sensor reading with known calibration.
    SensorDirect,
    /// Content pulled from an external URL.
    WebFetched,
    /// Result from an MCP or tool invocation.
    ToolResult,
    /// Reconstructed from an earlier deposit.
    MemoryRecall,
    /// Source cannot be determined.
    Unknown,
}

impl Default for SourceType {
    fn default() -> Self {
        SourceType::Unknown
    }
}

/// Default trust level on a 0-1000 integer scale (1000 = full trust).
/// No floats per project constraint; the scalar is just discretized to ppm-ish.
pub fn source_type_default_trust(s: &SourceType) -> u32 {
    match s {
        SourceType::UserDirect => 900,
        SourceType::UserCorrection => 1000,
        SourceType::AgentInference => 500,
        SourceType::SensorDirect => 800,
        SourceType::WebFetched => 400,
        SourceType::ToolResult => 300,
        SourceType::MemoryRecall => 500,
        SourceType::Unknown => 100,
    }
}

/// Origin block — where this deposit came from, and what deposits it derives from.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Origin {
    /// Named entity if known (e.g. user's ID, agent name).
    #[serde(default)]
    pub author: Option<String>,
    /// Wall-clock time the content was first captured (epoch seconds).
    #[serde(default)]
    pub captured_at: u64,
    /// How it entered memory — "direct", "mcp:<tool>", "web:<url>", etc.
    #[serde(default)]
    pub via: String,
    /// Prior deposit ids this was derived from (chain of derivation).
    #[serde(default)]
    pub chain: Vec<u64>,
    /// If this deposit corrects a prior one, its id. The `corrects` target
    /// also gets its `revised_by` stamped to this deposit's id on insert.
    #[serde(default)]
    pub corrects: Option<u64>,
}

/// A stored memory. Legacy entries load via `#[serde(default)]` on new fields;
/// `legacy_origin` flips to true when the entry lacks provenance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryEntry {
    // Existing fields
    pub timestamp: u64,
    pub tick_range: (u64, u64),
    pub content: String,
    pub topic: Option<String>,
    pub depth_profile: Vec<(u32, usize)>,

    // Identity — assigned monotonically by ContentStore on insertion.
    #[serde(default)]
    pub id: u64,

    // Provenance
    #[serde(default)]
    pub observer_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub stream_id: Option<String>,
    #[serde(default)]
    pub source_type: SourceType,
    #[serde(default)]
    pub trust_level: u32,
    #[serde(default)]
    pub origin: Option<Origin>,

    // Optional signal labels
    #[serde(default)]
    pub modality: Option<String>,
    #[serde(default)]
    pub language: Option<String>,

    // Recall keys — content-addressable content_ids from leaf to root (deepest-first).
    // None segments correspond to Learning nodes (no stable identity yet).
    #[serde(default)]
    pub path_content_ids: Vec<Option<u64>>,

    // True if this entry was loaded from a legacy snapshot without provenance.
    #[serde(default)]
    pub legacy_origin: bool,

    /// If a later user_correction supersedes this entry, its deposit id.
    /// Set on the corrected entry when the correction is inserted.
    #[serde(default)]
    pub revised_by: Option<u64>,
}
