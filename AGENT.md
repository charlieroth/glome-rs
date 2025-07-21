# AGENT.md - glome-rs Project Guide

## Commands
- **Build**: `cargo build` (dev), `cargo build --release` (release)
- **Test**: `cargo test` (all tests), `cargo test <testname>` (single test), `cargo test -p <package>` (package tests)
- **Lint**: `cargo clippy` (lints), `cargo check` (type check), `cargo fix` (auto-fix issues)
- **Format**: `cargo fmt` (format code)
- **Run specific package**: `cargo run -p <package>` where package is one of: echo, uniqueids, single_node_broadcast, multi_node_broadcast, efficient_broadcast, grow_only_counter, kafka
- **Maelstrom tests**: Use `make` targets for integration testing with Maelstrom:
  - `make echoer` - Test echo service
  - `make unique-id` - Test unique ID generation
  - `make snb` - Test single node broadcast
  - `make mnb` - Test multi-node broadcast
  - `make ftb` - Test fault-tolerant broadcast
  - `make eb-one`, `make eb-two` - Test efficient broadcast
  - `make goc` - Test grow-only counter
  - `make sn-kafka` - Test Kafka implementation

## Architecture
- **Workspace**: Cargo workspace with 8 challenge implementations + 1 core library
- **maelstrom**: Core library providing message types, protocol structures, and utilities for Gossip Glomers challenges
- **Services**: Each service implements a specific Gossip Glomers challenge:
  - **echo**: Echo service (basic message handling)
  - **uniqueids**: Unique ID generation service
  - **single_node_broadcast**: Single-node broadcast implementation
  - **multi_node_broadcast**: Multi-node broadcast with gossip protocol
  - **efficient_broadcast**: Optimized broadcast for high throughput/low latency
  - **grow_only_counter**: Grow-only counter CRDT implementation
  - **kafka**: Kafka-like messaging system
- **Protocol**: JSON-based message passing with stdin/stdout for Fly.io Gossip Glomers distributed systems challenges
- **Testing**: Integration tests via Maelstrom test harness, accessible through Makefile targets

## Code Style
- **Edition**: Rust 2024 edition across all packages
- **Async**: Uses Tokio for async runtime, prefer `async/await` patterns
- **Error handling**: Use `unwrap()` for demo code, structured error types in `maelstrom` crate
- **Serde**: JSON serialization with `serde` and `serde_json`, use `#[serde(tag = "type")]` for message enums
- **Dependencies**: All services depend on maelstrom crate, most use `rand` for unique ID generation
- **Naming**: Snake_case for variables/functions, PascalCase for types, descriptive names like `msg_id`, `node_id`
- **Imports**: Group std imports first, then external crates, then local modules
- **Channels**: Use `mpsc::channel` for async message passing between components
- **Types**: Prefer explicit types for public APIs, use generics sparingly (e.g., `Envelope<T = Body>`)
