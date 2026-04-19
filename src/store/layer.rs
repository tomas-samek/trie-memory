use serde::{Deserialize, Serialize};

pub type LayerId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLayer {
    pub id: LayerId,
    pub label: String,
    pub domain: Option<String>,
    pub origin_timestamp: u64,
    pub tick_start: u64,
    pub tick_end: u64,
    pub word_count: usize,
    pub concept_ids: Vec<u64>,
}

impl MemoryLayer {
    /// Relative tick for a given absolute tick within this layer.
    pub fn relative_tick(&self, absolute_tick: u64) -> Option<u64> {
        if absolute_tick >= self.tick_start && absolute_tick <= self.tick_end {
            Some(absolute_tick - self.tick_start)
        } else {
            None
        }
    }

    /// Duration in ticks.
    pub fn tick_duration(&self) -> u64 {
        self.tick_end - self.tick_start
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LayerStore {
    layers: Vec<MemoryLayer>,
    active: Option<MemoryLayer>,
}

impl LayerStore {
    pub fn new() -> Self {
        Default::default()
    }

    /// Begin a new layer. All writes between begin and commit belong to this layer.
    pub fn begin(&mut self, label: String, domain: Option<String>, tick_start: u64) -> LayerId {
        let id = self.layers.len() as LayerId;
        self.active = Some(MemoryLayer {
            id,
            label,
            domain,
            origin_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            tick_start,
            tick_end: tick_start,
            word_count: 0,
            concept_ids: Vec::new(),
        });
        id
    }

    /// Update word count and tick_end on the active layer (called during writes).
    pub fn record_write(&mut self, words: usize, current_tick: u64) {
        if let Some(ref mut layer) = self.active {
            layer.word_count += words;
            layer.tick_end = current_tick;
        }
    }

    /// Record a concept created during this layer.
    pub fn record_concept(&mut self, concept_id: u64) {
        if let Some(ref mut layer) = self.active {
            layer.concept_ids.push(concept_id);
        }
    }

    /// Commit the active layer — freeze it, add to history.
    pub fn commit(&mut self) -> Option<&MemoryLayer> {
        if let Some(layer) = self.active.take() {
            self.layers.push(layer);
            self.layers.last()
        } else {
            None
        }
    }

    /// Get all committed layers.
    pub fn layers(&self) -> &[MemoryLayer] {
        &self.layers
    }

    /// Get layer by ID.
    pub fn get(&self, id: LayerId) -> Option<&MemoryLayer> {
        self.layers.get(id as usize)
    }

    /// Find layers that contain a given absolute tick.
    pub fn layers_at_tick(&self, tick: u64) -> Vec<&MemoryLayer> {
        self.layers
            .iter()
            .filter(|l| tick >= l.tick_start && tick <= l.tick_end)
            .collect()
    }

    /// Find layers by domain prefix.
    pub fn layers_by_domain(&self, domain: &str) -> Vec<&MemoryLayer> {
        self.layers
            .iter()
            .filter(|l| l.domain.as_deref() == Some(domain))
            .collect()
    }

    /// Find layers committed at or before a given wall-clock timestamp.
    pub fn layers_at_time(&self, timestamp: u64) -> Vec<&MemoryLayer> {
        self.layers
            .iter()
            .filter(|l| l.origin_timestamp <= timestamp)
            .collect()
    }

    /// Get the active (uncommitted) layer, if any.
    pub fn active(&self) -> Option<&MemoryLayer> {
        self.active.as_ref()
    }

    /// Total number of committed layers.
    pub fn count(&self) -> usize {
        self.layers.len()
    }

    pub fn save(&self, path: &str) -> std::io::Result<usize> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let len = json.len();
        std::fs::write(path, &json)?;
        Ok(len)
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let store: Self = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(store)
    }

    pub fn exists(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
}
