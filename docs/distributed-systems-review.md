## Glome-rs Distributed Systems Review

### Executive summary

- Reliability gaps: in-memory state only (no durability), frequent `unwrap()` on critical paths, no retry/timeout backoff, limited duplicate/ACK handling.
- Protocol rigor: static leader election, no epochs/terms, no per-peer watermarks, at-least-once without idempotence safeguards.
- Scalability: broadcast gossips full state every tick; no flow control; unbounded sets; full-state counter gossip.
- Consistency: TARUT/TARCT rely on local clocks; TARCT versions are not globally comparable; duplicate acks can prematurely satisfy quorum.
- Observability: no structured logging, no metrics, no trace IDs.

The codebase is a solid Maelstrom solution set; below are concrete edits to make it more production-leaning while preserving challenge semantics.

---

### Cross-cutting runtime concerns

- Stdout error handling: message loop prints with `serde_json::to_string(...).unwrap()`; a single bad message panics the process. Prefer fallible send with logging.

```rust
// maelstrom/src/node.rs
for response in handler.handle(&mut node, msg) {
    match serde_json::to_string(&response) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("serialize error: {e:?} for response: {:?}", response),
    }
}
```

- Reader task: ignores deserialization errors and backpressure signals. Consider logging malformed JSON and exiting on channel closure.

```rust
// inside stdin reader spawn
while let Ok(Some(line)) = lines.next_line().await {
    match serde_json::from_str::<Message>(&line) {
        Ok(msg) => if stdin_tx.send(msg).await.is_err() { break; },
        Err(e) => eprintln!("decode error: {e:?} line={line}"),
    }
}
```

- Backpressure: the mailbox is `mpsc(32)`. For bursts, consider a larger bounded channel and applying basic flow control (drop oldest, or NAK/retry at protocol layer).

- Observability: add `tracing` with JSON logs and request correlation (e.g., include `msg_id` in spans). Helps postmortems without altering protocol.

---

### Unique ID generation (`uniqueids`)

- Random 64-bit IDs can collide (low probability, non-zero). Partition tolerance is fine but monotonicity and per-node uniqueness not guaranteed.

Recommend a Snowflake-lite: epoch millis (42 bits) + node-id hash (10 bits) + per-ms sequence (12 bits).

```rust
// uniqueids/src/node.rs (sketch)
struct IdGen { node_bits: u64, last_ms: u64, seq: u16 }
impl IdGen {
    fn new(node_id: &str) -> Self {
        let node_hash = xxhash_rust::xxh3::xxh3_64(node_id.as_bytes()) & 0x3ff; // 10 bits
        Self { node_bits: node_hash, last_ms: 0, seq: 0 }
    }
    fn generate(&mut self) -> u64 {
        let now = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64) & ((1<<42)-1);
        if now == self.last_ms { self.seq = self.seq.wrapping_add(1); } else { self.last_ms = now; self.seq = 0; }
        (now << 22) | (self.node_bits << 12) | (self.seq as u64)
    }
}
```

Speculative: in Maelstrom’s environment, clock skew is small but not guaranteed. If you need monotonic per-node IDs across NTP jumps, gate on non-decreasing `now` and sleep/spin when sequence overflows.

---

### Broadcast

#### Single-node broadcast

- Sends to all peers on each client broadcast. OK for the challenge; in prod, avoid sending to yourself; dedup may be useful.

#### Multi-node broadcast

Findings:
- Full-state gossip every 100ms to k peers: O(N·M) traffic; no delta tracking.
- No per-peer ACK or bloom/digest to know what the peer lacks.
- `HashSet<u64>` grows unbounded; no compaction.

Recommended changes:
- Maintain per-peer high-watermark or a per-peer `seen` set; send only delta.
- Add push-pull anti-entropy: exchange digests (set of ranges or a rolling bloom) and request missing.
- Add jittered intervals and exponential backoff for slow peers.

Minimal delta-based improvement:

```rust
// multi_node_broadcast/src/node.rs (sketch)
struct MultiNodeBroadcastNode {
    messages: HashSet<u64>,
    peer_seen: HashMap<String, HashSet<u64>>, // per-peer delivered set
}
fn gossip(&mut self, node: &mut Node) -> Vec<Message> {
    let mut out = Vec::new();
    for peer in &self.gossip_peers {
        let seen = self.peer_seen.entry(peer.clone()).or_default();
        let delta: Vec<u64> = self.messages.iter().copied().filter(|m| !seen.contains(m)).take(1024).collect();
        if !delta.is_empty() {
            out.push(Message { src: node.id.clone(), dest: peer.clone(), body: MessageBody::BroadcastGossip { msg_id: node.next_msg_id(), messages: delta } });
        }
    }
    out
}
fn handle_broadcast_gossip_from(&mut self, peer: &str, msgs: Vec<u64>) {
    let seen = self.peer_seen.entry(peer.to_string()).or_default();
    for m in msgs { self.messages.insert(m); seen.insert(m); }
}
```

Speculative: for scale, replace `peer_seen` with per-peer bitmaps or roaring bitmaps keyed by message id ranges, or use HyParView + Plumtree for epidemic broadcast with dedup and repair.

---

### Grow-only counter (G-Counter)

- `KV::init(node_ids)` exists but never used; peers are lazy-added on first `add`. Either call `self.kv.init(node_ids)` on `Init` or document lazy-init.

```rust
// grow_only_counter/src/node.rs in Init handler
self.kv.init(node_ids.clone());
node.handle_init(node_id, node_ids);
```

- Full-state gossip of all counters each tick: switch to versioned per-peer delta. Track last seen version per peer and send only entries with higher versions.

---

### Kafka-style log

Findings:
- Leader is first in lexicographic order; no epochs/terms; no re-election; no fencing tokens.
- Replication is at-least-once; acks are counted but not de-duplicated per follower.
- No persistence; follower state and leader state are in-memory. Restart loses data.
- No commit index or durability barrier; client `send_ok` is returned once quorum acks, but logs are volatile.

Immediate correctness fix: deduplicate follower acks per offset to prevent a single follower’s duplicate ack from prematurely counting towards quorum.

```rust
// multi_node_kafka/src/node.rs
struct Pending { client: String, client_msg_id: u64, acks: usize, from: HashSet<String> }
// on replicate send: keep track of replica ids; seed with leader itself
self.pendings.insert(offset, Pending { client: message.src.clone(), client_msg_id: msg_id, acks: 1, from: HashSet::from([node.id.clone()]) });
// on ReplicateOk:
if let Some(p) = self.pendings.get_mut(&offset) {
    if p.from.insert(message.src.clone()) { // new replica ack
        p.acks += 1;
        if p.acks >= self.quorum(node) { /* reply & remove */ }
    }
}
```

Speculative but recommended:
- Add `term`/`leader_id` and reject appends from stale leaders (basic Raft-like fencing).
- Add per-partition write-ahead logs with fsync-on-commit for durability (e.g., `mmap` + page-aligned fsync, or `sled`/`sqlite` for simplicity).
- Replace `insert_at` with Raft-style log entries (index, term, payload) + match index negotiation; repair divergent followers.
- Implement leader lease with periodic heartbeats; followers reject client `send` but accept `forward_send`.

Persistence sketch using `sled` per key:

```rust
// maelstrom/src/log_persist.rs (sketch)
struct DurableLog { db: sled::Tree }
impl DurableLog {
    fn append(&self, key: &str, off: u64, msg: u64) -> sled::Result<()> {
        let k = (off).to_be_bytes();
        self.db.insert([key.as_bytes(), &k].concat(), msg.to_be_bytes())?;
        self.db.flush()?; Ok(())
    }
}
```

---

### Transactions

#### TARUT (totally-available, read-uncommitted)

- Design matches RU semantics. Replication is best-effort; duplicates are idempotent for identical values, but interleavings across writers can be non-deterministic. OK for RU, but consider including a logical timestamp to enable last-writer-wins merges.

#### TARCT (read-committed with optimistic checks)

Findings:
- `commit_ts` is local to a node. The `KV` uses per-key last version as a single `u64`. Concurrent commits from different nodes with the same version (e.g., both `1`) resolve by arrival order, not by a deterministic total order.

Recommended: use version tuples `(ts, node_id)` and lexicographic ordering; optionally Lamport clocks.

```rust
// maelstrom/src/kv.rs (sketch of change)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
struct Version { ts: u64, node: u64 } // node = stable hash of node_id
struct KV { entries: HashMap<u64, Option<u64>>, versions: HashMap<u64, Version> }
fn apply(&mut self, key: u64, val: Option<u64>, v: Version) {
    if self.versions.get(&key).map(|&cur| cur < v).unwrap_or(true) {
        self.entries.insert(key, val);
        self.versions.insert(key, v);
    }
}
// on commit: bump lamport = max(local, observed)+1 and use (lamport, node_hash)
```

- Conflict check should compare against the highest observed committed version tuple, not just scalar `u64`.

- Replication should carry the version tuple; merges should pick max tuple.

---

### Memory growth and compaction

- `HashSet<u64>` in broadcast and `Logs` keep all data forever. Add retention policies:
  - Broadcast: after all peers confirm watermarks ≥ X, compact older messages.
  - Kafka: implement log segments + delete/compact policies (size- or time-based).

---

### Flow control and retry policy

- Add exponential backoff and jitter for retries (broadcast gossip, replication), and per-peer circuit breaking on repeated failures.
- Bound batches (`take(1024)`) and prefer small frames to avoid head-of-line blocking.

---

### Testing gaps to add

- Property tests: unique IDs (no duplicates over large N), monotonicity per node.
- Broadcast: convergence under message loss and reordering; ensure no livelock.
- Kafka: ack de-dup, follower restart/replay; pending cleanup on leader change (introduce a synthetic `term` to test fencing behavior).
- TARCT: two-writer race with same/different versions resolves deterministically with version tuples.

---

### Security & hardening

- Validate message fields (avoid panics on unexpected shapes when enums expand).
- Rate-limit client requests; guard against amplification (e.g., broadcast storms).
- Optional: message authentication (HMAC) if used beyond Maelstrom.

---

### Small correctness nits

- `multi_node_broadcast/src/main.rs`: printing with `unwrap()` in both select branches—same panic risk; apply the safe serialize pattern shown above.
- `Node::handle_init` clones vectors unnecessarily; fine at this scale, but can take by reference and move where possible.
- Consider `FxHashMap`/`ahash` for hot-path maps/sets to reduce CPU.

---

### Suggested incremental roadmap

1) Eliminate `unwrap()` on hot paths; add minimal logging everywhere serialization/deserialization happens.
2) Add ack de-dup in Kafka and per-peer delta gossip in broadcast and counter.
3) Introduce simple epochs/terms and leader lease in Kafka; add durable storage behind `Logs`.
4) Change TARCT versions to version tuples; propagate through replication.
5) Add basic metrics (requests/sec, gossip sizes, replication latency) with `metrics` + `metrics-exporter-prometheus`.
