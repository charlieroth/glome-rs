use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Counter {
    pub version: u64,
    pub value: u64,
}

/// KV store with a "Version Vector" as the storage layer
///
/// Sources:
///
/// * https://martinfowler.com/articles/patterns-of-distributed-systems/version-vector.html
/// * https://en.wikipedia.org/wiki/Version_vector
/// * https://github.com/elh/gossip-glomers/blob/main/src/4_grow_only_counter.clj
pub struct KV {
    pub counters: HashMap<String, Counter>,
}

impl KV {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }

    pub fn init(&mut self, node_ids: Vec<String>) {
        self.counters = HashMap::new();
        for node_id in node_ids {
            self.counters.insert(node_id, Counter::default());
        }
    }

    pub fn add(&mut self, node_id: String, delta: u64) {
        self.counters
            .entry(node_id)
            .and_modify(|counter| {
                counter.value += delta;
                counter.version += 1;
            })
            .or_insert_with(|| Counter {
                version: 1,
                value: delta,
            });
    }

    pub fn read(&self) -> u64 {
        let mut sum = 0;
        for (_, counter) in self.counters.iter() {
            sum += counter.value;
        }
        sum
    }

    pub fn merge(&mut self, incoming: HashMap<String, Counter>) {
        for (node_id, incoming_counter) in incoming {
            let current_counter = self.counters.get(&node_id);
            match current_counter {
                Some(c) => {
                    if incoming_counter.version > c.version {
                        self.counters.entry(node_id).and_modify(|counter| {
                            counter.version = incoming_counter.version;
                            counter.value = incoming_counter.value;
                        });
                    }
                }
                None => {
                    self.counters.insert(node_id, incoming_counter);
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.counters.is_empty()
    }
}
