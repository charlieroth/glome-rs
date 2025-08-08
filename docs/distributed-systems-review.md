## Glome-rs Distributed Systems Review

### Executive summary

- Reliability gaps: in-memory state only (no durability), frequent `unwrap()` on critical paths, no retry/timeout backoff, limited duplicate/ACK handling.
- Protocol rigor: static leader election, no epochs/terms, no per-peer watermarks, at-least-once without idempotence safeguards.
- Scalability: broadcast gossips full state every tick; no flow control; unbounded sets; full-state counter gossip.
- Consistency: TARUT/TARCT rely on local clocks; TARCT versions are not globally comparable; duplicate acks can prematurely satisfy quorum.
- Observability: no structured logging, no metrics, no trace IDs.

The codebase is a solid Maelstrom solution set; below are concrete edits to make it more production-leaning while preserving challenge semantics.

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
