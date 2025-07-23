# Gossip Glomers

Rust workspace containing solutions to [Fly.io's Gossip Glomers](https://fly.io/dist-sys/) distributed systems challenges.

## Overview

Gossip Glomers is a series of distributed systems programming challenges. These challenges use the Maelstrom platform to simulate distributed systems environments, inject network failures, and verify that implementations maintain correctness under various failure conditions.

This workspace provides Rust implementations for all core challenges, demonstrating practical distributed systems concepts including consensus algorithms, gossip protocols, fault-tolerant broadcasting, and replicated data structures.

## Architecture

The project is organized as a Cargo workspace with 8 challenge implementations and 1 core library:

- **`maelstrom`** - Core library providing message types, protocol structures, and node abstractions
- **`echo`** - Basic echo service (warmup challenge)
- **`uniqueids`** - Globally unique ID generation service
- **`single_node_broadcast`** - Single-node broadcast implementation
- **`multi_node_broadcast`** - Multi-node broadcast with gossip protocol
- **`efficient_broadcast`** - Optimized broadcast for high throughput/low latency
- **`grow_only_counter`** - Grow-only counter CRDT implementation
- **`kafka`** - Kafka-like replicated log service

## Technical Features

- **Async Runtime**: Built on Tokio with async/await patterns
- **Message Protocol**: JSON-based communication over stdin/stdout for Maelstrom integration
- **Fault Tolerance**: Designed to handle network partitions and node failures
- **Gossip Protocols**: Various gossip algorithms for distributed information dissemination
- **CRDT Support**: Conflict-free replicated data types for distributed state management

## Building and Testing

### Build
```bash
cargo build                    # Development build
cargo build --release          # Release build
```

### Run Individual Services
```bash
cargo run -p echo              # Echo service
cargo run -p uniqueids         # Unique ID generation
cargo run -p multi_node_broadcast  # Multi-node broadcast
# ... etc for other services
```

### Maelstrom Integration Tests
```bash
make echoer                    # Test echo service
make unique-id                 # Test unique ID generation
make snb                       # Test single node broadcast
make mnb                       # Test multi-node broadcast
make ftb                       # Test fault-tolerant broadcast
make eb-one                    # Test efficient broadcast (scenario 1)
make eb-two                    # Test efficient broadcast (scenario 2)
make goc                       # Test grow-only counter
make sn-kafka                  # Test Kafka implementation
```

## Protocol

All services communicate using JSON messages over stdin/stdout following the Maelstrom protocol. The core library provides strongly-typed message structures and handles serialization/deserialization automatically.

Example message flow:
1. Maelstrom sends initialization message to node
2. Node processes requests and sends responses
3. Nodes may gossip with each other for distributed coordination
4. Maelstrom verifies correctness properties and performance characteristics

## Learning Objectives

This codebase demonstrates practical implementations of:

- Distributed consensus and coordination mechanisms
- Gossip protocols for reliable message dissemination
- Fault-tolerant broadcasting under network partitions
- Conflict-free replicated data types (CRDTs)
- Replicated log systems with strong consistency guarantees
- Distributed transaction processing with various isolation levels

Each challenge builds upon previous concepts while introducing new distributed systems complexities, making this an excellent resource for understanding distributed systems through hands-on implementation.
