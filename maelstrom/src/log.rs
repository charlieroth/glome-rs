use std::collections::BTreeMap;
use std::collections::HashMap;

pub struct Logs {
    inner: HashMap<String, Log>,
}

impl Default for Logs {
    fn default() -> Self {
        Self::new()
    }
}

impl Logs {
    pub fn new() -> Self {
        Self { inner: HashMap::new() }
    }

    fn get_or_create(&mut self, key: &str) -> &mut Log {
        self.inner.entry(key.to_string()).or_default()
    }

    /// Handle `send`: append and return offset
    pub fn append(&mut self, key: &str, msg: u64) -> u64 {
        let log = self.get_or_create(key);
        log.append(msg)
    }

    /// Handle `poll`: for each requested log, read from that offset
    pub fn poll(&self, offsets: &HashMap<String, u64>) -> HashMap<String, Vec<(u64, u64)>> {
        let mut result = HashMap::new();
        for (key, &off) in offsets {
            if let Some(log) = self.inner.get(key) {
                let entries: Vec<(u64, u64)> = log.read_from(off, None);
                result.insert(key.clone(), entries);
            }
        }
        result
    }

    /// Handle `commit_offsets`
    pub fn commit_offsets(&mut self, offsets: HashMap<String, u64>) {
        for (key, off) in offsets {
            if let Some(log) = self.inner.get_mut(&key) {
                log.commit(off);
            }
        }
    }

    /// Handle `list_committed_offsets`
    pub fn list_committed_offsets(&self, keys: &[String]) -> HashMap<String, u64> {
        let mut result = HashMap::new();
        for key in keys {
            if let Some(log) = self.inner.get(key) {
                result.insert(key.clone(), log.committed_offset());
            }
        }
        result
    }
}

/// A single append-only log
pub struct Log {
    /// `entries` - for clients to "poll" from any arbitrary offset, even if messages weren't
    /// written at every integer in between
    entries: BTreeMap<u64, u64>,
    next_offset: u64,
    committed: u64,
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Log {
    /// Create a new log starting at offset 0
     pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_offset: 0,
            committed: 0
        }
    }
    
    /// Append a message, returning its unique offset
    pub fn append(&mut self, msg: u64) -> u64 {
        let offset = self.next_offset;
        self.entries.insert(offset, msg);
        self.next_offset += 1;
        offset
    }
    
    /// Return all entries at or after `from_offset`, up to `max` items if specified
    pub fn read_from(&self, from_offset: u64, max: Option<usize>) -> Vec<(u64, u64)> {
        let mut out = Vec::new();
        for (&off, &msg) in self.entries.range(from_offset..) {
            out.push((off, msg));
            if let Some(limit) = max {
                if out.len() >= limit {
                    break;
                }
            }
        }
        out
    }

    /// Mark messages up through `offset` as committed
    pub fn commit(&mut self, offset: u64) {
        if offset > self.committed {
            self.committed = offset;
        }
    }

    /// Retrieve the highest committed offset
    pub fn committed_offset(&self) -> u64 {
        self.committed
    }
}
