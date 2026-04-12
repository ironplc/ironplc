//! Process-level container cache for compiled `.iplc` bytecode.
//!
//! The cache is the only cross-call state in the MCP server. It stores
//! compiled containers keyed by an opaque `container_id` so that
//! `compile → run` can hand off bytecode without routing it through the
//! LLM context.
//!
//! See REQ-ARC-070 through REQ-ARC-073 in `specs/design/mcp-server.md`.

use std::collections::{HashMap, VecDeque};
use std::fmt;

/// Default maximum number of cached containers.
pub const DEFAULT_MAX_ENTRIES: usize = 64;

/// Default maximum total bytes across all cached containers (64 MiB).
pub const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

/// A compiled container stored in the cache.
pub struct CachedContainer {
    /// Serialized `.iplc` bytes.
    pub iplc_bytes: Vec<u8>,
    /// Task metadata extracted from the compiled program.
    pub tasks: Vec<TaskMeta>,
    /// Program metadata extracted from the compiled program.
    pub programs: Vec<ProgramMeta>,
    /// Cached byte size (equal to `iplc_bytes.len()`).
    byte_size: usize,
}

impl CachedContainer {
    /// Creates a new cached container from serialized bytes and metadata.
    pub fn new(iplc_bytes: Vec<u8>, tasks: Vec<TaskMeta>, programs: Vec<ProgramMeta>) -> Self {
        let byte_size = iplc_bytes.len();
        Self {
            iplc_bytes,
            tasks,
            programs,
            byte_size,
        }
    }

    /// Returns the byte size of the cached container.
    pub fn byte_size(&self) -> usize {
        self.byte_size
    }
}

/// Task metadata for the compile tool response.
#[derive(Clone, Debug)]
pub struct TaskMeta {
    pub name: String,
    pub priority: u32,
    pub kind: String,
    pub interval_ms: Option<f64>,
}

/// Program metadata for the compile tool response.
#[derive(Clone, Debug)]
pub struct ProgramMeta {
    pub name: String,
    pub task: Option<String>,
}

/// Error returned when inserting a container that is too large.
#[derive(Debug)]
pub enum InsertError {
    /// The container's byte size exceeds the cache's total byte budget.
    TooLarge { size: usize, max: usize },
}

impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InsertError::TooLarge { size, max } => {
                write!(
                    f,
                    "container size ({size} bytes) exceeds cache byte budget ({max} bytes)"
                )
            }
        }
    }
}

/// LRU-bounded container cache.
///
/// Bounded by both entry count and total byte size. Evicts least-recently-used
/// entries when either bound would be exceeded.
pub struct ContainerCache {
    entries: HashMap<String, CachedContainer>,
    lru_order: VecDeque<String>,
    total_bytes: usize,
    next_id: u64,
    max_entries: usize,
    max_bytes: usize,
}

impl ContainerCache {
    /// Creates a new cache with the given capacity bounds.
    pub fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            total_bytes: 0,
            next_id: 0,
            max_entries,
            max_bytes,
        }
    }

    /// Inserts a container into the cache and returns its opaque ID.
    ///
    /// Returns `InsertError::TooLarge` if the container's byte size exceeds
    /// the entire byte budget.
    pub fn insert(&mut self, container: CachedContainer) -> Result<String, InsertError> {
        if container.byte_size > self.max_bytes {
            return Err(InsertError::TooLarge {
                size: container.byte_size,
                max: self.max_bytes,
            });
        }

        // Evict until there is room for the new entry.
        while self.entries.len() >= self.max_entries
            || self.total_bytes + container.byte_size > self.max_bytes
        {
            if let Some(evict_id) = self.lru_order.pop_front() {
                if let Some(evicted) = self.entries.remove(&evict_id) {
                    self.total_bytes -= evicted.byte_size;
                }
            } else {
                break;
            }
        }

        let id = format!("c_{}", self.next_id);
        self.next_id += 1;
        self.total_bytes += container.byte_size;
        self.lru_order.push_back(id.clone());
        self.entries.insert(id.clone(), container);
        Ok(id)
    }

    /// Looks up a container by ID and touches LRU (moves to most-recent).
    pub fn get(&mut self, id: &str) -> Option<&CachedContainer> {
        if self.entries.contains_key(id) {
            // Move to back of LRU order
            if let Some(pos) = self.lru_order.iter().position(|x| x == id) {
                self.lru_order.remove(pos);
                self.lru_order.push_back(id.to_string());
            }
            self.entries.get(id)
        } else {
            None
        }
    }

    /// Removes a container from the cache. Returns `true` if it existed.
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(removed) = self.entries.remove(id) {
            self.total_bytes -= removed.byte_size;
            if let Some(pos) = self.lru_order.iter().position(|x| x == id) {
                self.lru_order.remove(pos);
            }
            true
        } else {
            false
        }
    }

    /// Returns the number of entries currently in the cache.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_container(size: usize) -> CachedContainer {
        CachedContainer::new(vec![0u8; size], vec![], vec![])
    }

    #[test]
    fn insert_when_within_limits_then_returns_id() {
        let mut cache = ContainerCache::new(10, 1024);
        let id = cache.insert(make_container(100)).unwrap();
        assert!(id.starts_with("c_"));
    }

    #[test]
    fn insert_when_entry_count_at_max_then_evicts_oldest() {
        let mut cache = ContainerCache::new(2, 1024 * 1024);
        let id1 = cache.insert(make_container(10)).unwrap();
        let _id2 = cache.insert(make_container(10)).unwrap();

        // Cache is full (2 entries). Inserting a third should evict id1.
        let _id3 = cache.insert(make_container(10)).unwrap();
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&id1).is_none());
    }

    #[test]
    fn insert_when_byte_budget_exceeded_then_evicts_oldest() {
        let mut cache = ContainerCache::new(100, 200);
        let id1 = cache.insert(make_container(100)).unwrap();
        let _id2 = cache.insert(make_container(80)).unwrap();

        // total_bytes is 180. Inserting 50 more would exceed 200, so evict id1.
        let _id3 = cache.insert(make_container(50)).unwrap();
        assert!(cache.get(&id1).is_none());
    }

    #[test]
    fn insert_when_single_entry_exceeds_budget_then_error() {
        let mut cache = ContainerCache::new(10, 100);
        let result = cache.insert(make_container(200));
        assert!(matches!(result, Err(InsertError::TooLarge { .. })));
    }

    #[test]
    fn get_when_existing_then_returns_entry() {
        let mut cache = ContainerCache::new(10, 1024);
        let id = cache.insert(make_container(42)).unwrap();
        let entry = cache.get(&id).unwrap();
        assert_eq!(entry.byte_size(), 42);
    }

    #[test]
    fn get_when_missing_then_returns_none() {
        let mut cache = ContainerCache::new(10, 1024);
        assert!(cache.get("c_999").is_none());
    }

    #[test]
    fn get_when_accessed_then_updates_lru_order() {
        let mut cache = ContainerCache::new(2, 1024 * 1024);
        let id1 = cache.insert(make_container(10)).unwrap();
        let id2 = cache.insert(make_container(10)).unwrap();

        // Touch id1 so it becomes most-recently-used
        cache.get(&id1);

        // Insert a third — should evict id2 (the LRU), not id1
        let _id3 = cache.insert(make_container(10)).unwrap();
        assert!(cache.get(&id1).is_some());
        assert!(cache.get(&id2).is_none());
    }

    #[test]
    fn remove_when_existing_then_returns_true_and_frees_bytes() {
        let mut cache = ContainerCache::new(10, 1024);
        let id = cache.insert(make_container(100)).unwrap();
        assert!(cache.remove(&id));
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn remove_when_missing_then_returns_false() {
        let mut cache = ContainerCache::new(10, 1024);
        assert!(!cache.remove("c_999"));
    }

    #[test]
    fn eviction_order_when_accessed_then_lru_preserved() {
        let mut cache = ContainerCache::new(3, 1024 * 1024);
        let id1 = cache.insert(make_container(10)).unwrap();
        let id2 = cache.insert(make_container(10)).unwrap();
        let id3 = cache.insert(make_container(10)).unwrap();

        // Access id1 and id3, making id2 the LRU
        cache.get(&id1);
        cache.get(&id3);

        // Insert two more to evict id2 first, then id1
        let _id4 = cache.insert(make_container(10)).unwrap();
        assert!(cache.get(&id2).is_none(), "id2 should have been evicted");

        let _id5 = cache.insert(make_container(10)).unwrap();
        assert!(cache.get(&id1).is_none(), "id1 should have been evicted");
        assert!(cache.get(&id3).is_some(), "id3 should still be present");
    }
}
