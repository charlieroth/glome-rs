use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub offset: u64,
    pub msg: u64,
}

pub struct Log {
    pub log: HashMap<String, Vec<LogEntry>>,
    pub commits: HashMap<String, u64>,
    pub offsets: HashMap<String, u64>,
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Log {
    pub fn new() -> Self {
        Self {
            log: HashMap::new(),
            commits: HashMap::new(),
            offsets: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: String, entry: LogEntry) {
        self.log
            .entry(key)
            .and_modify(|entries| entries.push(entry.clone()))
            .or_insert(vec![entry]);
    }

    pub fn logs(
        &mut self,
        keys_and_offsets: HashMap<String, u64>,
    ) -> HashMap<String, Vec<Vec<u64>>> {
        let mut offsets: HashMap<String, Vec<Vec<u64>>> = HashMap::new();
        for (key, offset) in keys_and_offsets.iter() {
            if !self.log.contains_key(key) {
                continue;
            }

            offsets.insert(key.into(), Vec::new());
            for entry in self.log.get(key).unwrap() {
                if entry.offset >= *offset {
                    offsets
                        .entry(key.into())
                        .and_modify(|entries| entries.push(vec![entry.offset, entry.msg]))
                        .or_insert(vec![vec![entry.offset, entry.msg]]);
                }
            }
        }
        offsets
    }

    pub fn set_commit_offsets(&mut self, keys_and_offsets: HashMap<String, u64>) {
        // iterate over (k, v) pairs; duplicate keys replace the old value
        self.commits.extend(keys_and_offsets);
    }

    pub fn inc_offset(&mut self, key: String) -> u64 {
        self.offsets
            .entry(key.clone())
            .and_modify(|e| *e += 1)
            .or_insert(1);

        *self.offsets.get(&key).unwrap_or(&1)
    }
}
