# AGENT.md - glome-rs Project Guide

## Commands
- **Build**: `cargo build` (dev), `cargo build --release` (release)
- **Test**: `cargo test` (all tests), `cargo test <testname>` (single test), `cargo test -p <package>` (package tests)
- **Lint**: `cargo clippy` (lints), `cargo check` (type check), `cargo fix` (auto-fix issues)
- **Format**: `cargo fmt` (format code)
- **Run specific package**: `cargo run -p echo`, `cargo run -p maelstrom`
- **Test echo service**: `./echo_test.sh` (integration test for echo service)

## Architecture
- **Workspace**: Cargo workspace with 2 packages: `echo` and `maelstrom`
- **maelstrom**: Core library providing message types and protocol structures for Gossip Glomers challenges
- **echo**: Echo service implementation using async Rust with Tokio
- **Protocol**: JSON-based message passing with stdin/stdout for Fly.io Gossip Glomers distributed systems challenges

## Code Style
- **Edition**: Rust 2024 edition across all packages
- **Async**: Uses Tokio for async runtime, prefer `async/await` patterns
- **Error handling**: Use `unwrap()` for demo code, structured error types in `maelstrom` crate
- **Serde**: JSON serialization with `serde` and `serde_json`, use `#[serde(tag = "type")]` for message enums
- **Naming**: Snake_case for variables/functions, PascalCase for types, descriptive names like `msg_id`, `node_id`
- **Imports**: Group std imports first, then external crates, then local modules
- **Channels**: Use `mpsc::channel` for async message passing between components
- **Types**: Prefer explicit types for public APIs, use generics sparingly (e.g., `Envelope<T = Body>`)
