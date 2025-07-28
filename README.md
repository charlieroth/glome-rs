# Gossip Glomers

Rust workspace containing solutions to
[Fly.io's Gossip Glomers](https://fly.io/dist-sys/) distributed systems
challenges.

## Overview

Gossip Glomers is a series of distributed systems programming challenges. These
challenges use the Maelstrom platform to simulate distributed systems
environments, inject network failures, and verify that implementations maintain
correctness under various failure conditions.

This repository tackles every exercise in the
[Fly.io "Gossip Glomers"](https://fly.io/dist-sys/) course.

Each sub-crate is a **stand-alone binary** that speaks
[Maelstrom](https://github.com/jepsen-io/maelstrom)'s JSON protocol over
STDIN/STDOUT, while the shared `maelstrom` library provides strongly-typed
message definitions, helper data-structures, and a few pieces of reusable logic
(e.g. an append-only log and a CRDT G-Counter).

## Learning Objectives

- Solidify Rust async fundamentals with Tokio channels, tasks & `select!`.
- Implement and reason about **at-least-once** vs **exactly-once** message
  delivery and the trade-offs between them.
- Build a **gossip-based broadcast** and observe convergence under
  partitions/packet-loss.
- Design a **state-based CRDT (G-Counter)** and demonstrate its monotonic-merge
  property.
- Construct a simple **leader / follower replicated log** à la Kafka and use
  quorum acknowledgements to tolerate crash failures.
- Apply **optimistic concurrency control** to execute transactional read-write
  registers with both _read-uncommitted_ and _read-committed_ semantics.
- Practice modelling network faults, timeouts and partitions with Maelstrom.
- Gain hands-on experience mapping text-based protocols to strongly typed Rust
  enums using Serde's `#[serde(tag = "type")]` pattern.

## Architecture

The project is organized as a Cargo workspace with challenge implementations and
1 core library (`maelstrom/src/*.rs`):

• **Binary crates** – one per challenge. Each contains a tiny `Node`
state-machine and a Tokio main loop that:

1. Spawns an async task to parse JSON messages from STDIN.
2. Processes messages, producing zero or more responses.
3. Serialises responses back to STDOUT.

• **`maelstrom` library** – `lib.rs` Typed `Message`/`MessageBody` enum (all
protocol verbs in one place).\
– `kv.rs` Grow-only counter CRDT.\
– `simple_log.rs` Single-node append-only log for the _05a_ exercise.\
– `log.rs` Replicated log with sparse offsets & committed markers for _05b-c_.

Tokio channels are used internally for back-pressure and to decouple IO from the
per-message state machine, but **all network IO is still just stdio** so it
integrates cleanly with Maelstrom.

## Maelstrom Testing

See `Makefile` for how to run the Maelstrom tool against the different Rust binaries that this project produces
