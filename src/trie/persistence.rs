use std::fs;
use std::io;
use std::path::Path;

use super::Trie;

#[derive(Debug)]
pub struct SnapshotResult {
    pub path: String,
    pub bytes_written: usize,
}

#[derive(Debug)]
pub struct RestoreResult {
    pub path: String,
    pub nodes_restored: usize,
}

impl Trie {
    /// Serialize trie to disk using bincode.
    pub fn snapshot(&self, path: &str) -> io::Result<SnapshotResult> {
        let bytes = bincode::serialize(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let len = bytes.len();
        fs::write(path, &bytes)?;
        Ok(SnapshotResult {
            path: path.to_string(),
            bytes_written: len,
        })
    }

    /// Deserialize trie from disk.
    pub fn restore(path: &str) -> io::Result<RestoreResult> {
        let bytes = fs::read(path)?;
        let trie: Trie =
            bincode::deserialize(&bytes).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let count = trie.nodes.len();
        // We return the result; caller replaces their trie with this one
        // To actually use the restored trie, use restore_into
        Ok(RestoreResult {
            path: path.to_string(),
            nodes_restored: count,
        })
    }

    /// Restore from disk, returning the full Trie.
    pub fn restore_from(path: &str) -> io::Result<Trie> {
        let bytes = fs::read(path)?;
        let trie: Trie =
            bincode::deserialize(&bytes).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(trie)
    }

    /// Check if a snapshot file exists at the given path.
    pub fn snapshot_exists(path: &str) -> bool {
        Path::new(path).exists()
    }
}
