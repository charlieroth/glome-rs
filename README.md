# Gossip Glomers

Rust workspace containing solutions to
[Fly.io's Gossip Glomers](https://fly.io/dist-sys/) distributed systems
challenges.

## Overview

Gossip Glomers is a series of distributed systems programming challenges. These
challenges use the Maelstrom platform to simulate distributed systems
environments, inject network failures, and verify that implementations maintain
correctness under various failure conditions.

This repository tackles every exercise in the [Fly.io "Gossip Glomers"](https://fly.io/dist-sys/)
course. Each sub-crate is a **stand-alone binary** that speaks
[Maelstrom](https://github.com/jepsen-io/maelstrom)'s JSON protocol over
STDIN/STDOUT, while the shared `maelstrom` library provides
strongly-typed message definitions, helper data-structures, and a few pieces
of reusable logic (e.g. an append-only log and a CRDT G-Counter).

```
┌───────────────────────────── workspace ─────────────────────────────┐
│ echo/                    # 01 – Echo                                │
│ uniqueids/               # 02 – Unique IDs                          │
│ single_node_broadcast/   # 03a – Broadcast (single-node)            │
│ multi_node_broadcast/    # 03b-e – Broadcast (multi-node, fault-tol)│
│ grow_only_counter/       # 04 – G-Counter CRDT                      │
│ single_node_kafka/       # 05a – Log (single-node)                  │
│ multi_node_kafka/        # 05b-c – Replicated log / quorum acks     │
│ single_node_tat/         # 06a – Txn, totally-available, R-U        │
│ tarut/                   # 06b-c – Txn with read-uncommitted + net   │
│ tarct/                   # 06d – Txn with read-committed            │
│ maelstrom/               # Shared library                           │
└─────────────────────────────────────────────────────────────────────┘
```

Every binary is intentionally **dependency-free** beyond Tokio & Serde so
that the core algorithmic ideas are easy to study.

## Learning Objectives

* Solidify Rust async fundamentals with Tokio channels, tasks & `select!`.
* Implement and reason about **at-least-once** vs **exactly-once** message
  delivery and the trade-offs between them.
* Build a **gossip-based broadcast** and observe convergence under
  partitions/packet-loss.
* Design a **state-based CRDT (G-Counter)** and demonstrate its
  monotonic-merge property.
* Construct a simple **leader / follower replicated log** à la Kafka and
  use quorum acknowledgements to tolerate crash failures.
* Apply **optimistic concurrency control** to execute transactional
  read-write registers with both *read-uncommitted* and *read-committed*
  semantics.
* Practice modelling network faults, timeouts and partitions with Maelstrom.
* Gain hands-on experience mapping text-based protocols to strongly typed
  Rust enums using Serde's `#[serde(tag = "type")]` pattern.

## Architecture

The project is organized as a Cargo workspace with challenge implementations and
1 core library (`maelstrom/src/*.rs`):

• **Binary crates** – one per challenge. Each contains a tiny `Node` state-machine
  and a Tokio main loop that:
  1. Spawns an async task to parse JSON messages from STDIN.
  2. Processes messages, producing zero or more responses.
  3. Serialises responses back to STDOUT.

• **`maelstrom` library**
  – `lib.rs`   Typed `Message`/`MessageBody` enum (all protocol verbs in one place).  
  – `kv.rs`    Grow-only counter CRDT.  
  – `simple_log.rs`  Single-node append-only log for the *05a* exercise.  
  – `log.rs`   Replicated log with sparse offsets & committed markers for *05b-c*.  

Tokio channels are used internally for back-pressure and to decouple IO from
the per-message state machine, but **all network IO is still just stdio** so it
integrates cleanly with Maelstrom.

## Technical Features

* **Serde-tagged enums** give zero-copy (de)serialisation of the entire
  Maelstrom protocol with exhaustive pattern-matching in every node.
* **Deterministic logical clocks** (`msg_id`) created per-node, avoiding the
  need for mutexes or atomics.
* **Gossip helpers** (`gossip()` functions) that periodically fan-out
  state using `tokio::time::interval`, allowing loss-tolerant propagation.
* **CRDT merge logic** lives in the library and is shared by counter nodes and
  transaction replicas, reducing copy-paste.
* **Quorum write & ACK tracking** in the Kafka replica (`multi_node_kafka`)
  keeps opaque `Pending` structs indexed by offset for O(1) completion checks.
* **Optimistic transaction engine** in *tarct* keeps per-key versions and
  uses conflict-checking plus write-set replication for *read-committed*
  semantics while remaining totally available.
* **Makefile test-targets** wire up the official Jepsen/Maelstrom harness so
  `make tarct` etc. spin everything up, run the Jepsen workload and assert
  linearizability / causal-consistency automatically.

## Building and Testing

### Maelstrom Integration Tests

```bash
make echoer                    # Challenge 1: Echo 
make unique-id                 # Challenge 2: Unique ID Generation 
make snb                       # Challenge 3a: Single-Node Broadcast 
make mnb                       # Challenge 3b: Multi-Node Broadcast 
make ftb                       # Challenge 3c: Fault-Tolerant Broadcast
make eb-one                    # Challenge 3d: Efficient Broadcast (scenario 1)
make eb-two                    # Challenge 3e: Efficient Broadcast (scenario 2) 
make goc                       # Challenge 4: Grow-Only Counter 
make sn-kafka                  # Challenge 5a: Single-Node Kafka-Style Log
make mn-kafka                  # Challenge 5b: Multi-Node Kafka-Style Log 
make e-kafka                   # Challenge 5c: Efficient Kafka-Style Log 
make sn-tat                    # Challenge 6a: Single-Node Totally-Available Transactions
make tarut                     # Challenge 6b: Totally-Available, Read Uncommitted Transactions
make tarut-partition           # Challenge 6c: Totally-Available, Read Uncommitted Transactions with Network Partitioning
make tarct                     # Challenge 6d: Totally-Available, Read Committed Transactions
```
