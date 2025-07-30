# Gossip Glomers

A comprehensive Rust workspace containing complete solutions to all
[Fly.io's Gossip Glomers](https://fly.io/dist-sys/) distributed systems
challenges, demonstrating advanced distributed systems patterns, unified
abstractions, comprehensive testing, and production-ready CI/CD.

## Skills Demonstrated

This project showcases:

### Distributed Systems Architecture

- Gossip protocols and eventual consistency
- Leader-follower replication patterns
- Conflict-free replicated data types (CRDTs)
- Optimistic concurrency control
- At-least-once vs exactly-once delivery semantics
- Network partition tolerance and fault recovery

### Advanced Rust Programming

- Async/await with Tokio runtime and channels
- Complex enum modeling with Serde for protocol definitions
- Workspace organization and dependency management
- Generic trait-based abstractions
- Comprehensive unit testing patterns

### Software Engineering Best Practices

- Unified node abstraction across all implementations
- Comprehensive test coverage for all challenges
- Automated CI/CD pipeline with matrix testing
- Clean architectural separation of concerns

## Challenge Implementations

All challenges implement a unified `MessageHandler` trait and leverage shared infrastructure:

| Challenge | Binary | Description | Key Concepts |
|-----------|--------|-------------|--------------|
| **01** | `echo` | Echo service | Basic message handling, JSON protocol |
| **02** | `uniqueids` | Unique ID generation | Distributed ID generation, node identity |
| **03a** | `single_node_broadcast` | Single-node broadcast | Message broadcasting, topology |
| **03b** | `multi_node_broadcast` | Multi-node broadcast | Gossip protocols, network partitions |
| **03c** | *fault-tolerant broadcast* | Fault-tolerant broadcasting | Partition tolerance, message replay |
| **03d** | *efficient broadcast* | Efficient broadcast | Throughput optimization, batching |
| **04** | `grow_only_counter` | G-Counter CRDT | State-based CRDTs, monotonic merge |
| **05a** | `single_node_kafka` | Single-node Kafka | Append-only logs, offset tracking |
| **05b** | `multi_node_kafka` | Multi-node Kafka | Replicated logs, leader election |
| **05c** | *efficient kafka* | Efficient Kafka | Quorum acknowledgments, consistency |
| **06a** | `single_node_tat` | Totally-available transactions | Read-uncommitted isolation |
| **06b** | `tarut` | Read-uncommitted transactions | Optimistic concurrency control |
| **06c** | `tarct` | Read-committed transactions | Transaction isolation levels |

## Architecture

### Unified Node Abstraction

The project features a unified architecture built around core abstractions:

```rust
// Unified base node providing common functionality
pub struct Node {
    pub id: String,
    pub peers: Vec<String>,
    msg_id: u32,
}

// Unified message handler trait implemented by all challenges
pub trait MessageHandler {
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message>;
}

// Common runtime for all challenges
pub fn run_node<H: MessageHandler>(mut handler: H) -> Result<(), Box<dyn Error>>
```

### Project Structure

```
glome-rs/
├── maelstrom/          # Core library with shared abstractions
│   ├── src/
│   │   ├── lib.rs      # Message types, Node struct, traits
│   │   ├── node.rs     # Core node implementation
│   │   ├── kv.rs       # G-Counter CRDT implementation
│   │   └── log.rs      # Replicated log utilities
│   └── Cargo.toml
├── echo/               # Challenge 01: Echo service
├── uniqueids/          # Challenge 02: Unique ID generation
├── single_node_broadcast/  # Challenge 03a: Single-node broadcast
├── multi_node_broadcast/   # Challenge 03b: Multi-node broadcast
├── grow_only_counter/      # Challenge 04: G-Counter CRDT
├── single_node_kafka/      # Challenge 05a: Single-node Kafka
├── multi_node_kafka/       # Challenge 05b: Multi-node Kafka
├── single_node_tat/        # Challenge 06a: Totally-available transactions
├── tarut/                  # Challenge 06b: Read-uncommitted transactions
├── tarct/                  # Challenge 06c: Read-committed transactions
├── .github/workflows/      # CI/CD pipeline
└── Makefile               # Maelstrom test automation
```

Each challenge implementation follows the same pattern:

1. **Custom node struct** implementing domain-specific state
2. **MessageHandler trait** for processing protocol messages
3. **Comprehensive unit tests** for core functionality
4. **Integration tests** via Maelstrom test harness

## Testing Strategy

### Unit Testing

Every challenge includes comprehensive unit tests covering:

- Message handling logic
- State transitions
- Edge cases and error conditions
- Protocol compliance

### Integration Testing

Automated Maelstrom testing via CI/CD:

- Network partition simulation
- Fault injection testing
- Performance benchmarking
- Correctness verification

### Test Execution

```bash
# Run all unit tests
cargo test --workspace

# Run specific challenge tests
cargo test -p echo
cargo test -p multi_node_broadcast

# Run Maelstrom integration tests
make echoer              # Test echo service
make unique-id           # Test unique ID generation
make mnb                 # Test multi-node broadcast
make goc                 # Test G-Counter CRDT
```

## CI/CD Pipeline

GitHub Actions workflow featuring:

- **Matrix Strategy**: Parallel testing of all 14+ challenge implementations
- **Dependency Caching**: Optimized Rust compilation and Maelstrom installation
- **Integration Testing**: Automated Maelstrom test execution
- **Quality Gates**: Build verification, linting, and correctness checks

The CI pipeline runs comprehensive tests on every push and pull request,
ensuring reliability and correctness across all distributed systems
implementations.

## Development Commands

```bash
# Build all challenges
cargo build --workspace

# Build specific challenge
cargo build -p echo

# Run challenge locally
cargo run -p echo

# Format and lint
cargo fmt
cargo clippy

# Run Maelstrom tests (see Makefile for full list)
make echoer unique-id snb mnb goc sn-kafka mn-kafka
```

## Learning Outcomes

This project demonstrates:

### Distributed Systems Fundamentals

- CAP theorem trade-offs in practice
- Consistency models and their implementations
- Fault tolerance and recovery strategies

### Advanced Rust Programming

- Complex async programming patterns
- Trait-based architectural design
- Workspace management and code organization

### Software Engineering Excellence

- Test-driven development practices
- CI/CD pipeline implementation
- Clean code and architectural principles

### Protocol Design and Implementation

- JSON-based message protocols
- State machine design patterns
- Network communication abstractions

## Key Achievements

- **Complete Implementation**: All Gossip Glomers challenges solved
- **Unified Architecture**: Consistent abstractions across all challenges  
- **Comprehensive Testing**: Unit tests for every challenge implementation
- **Production CI/CD**: Automated testing with matrix strategy
- **Clean Code**: Well-organized, documented, and maintainable codebase
- **Performance Optimized**: Efficient async implementations using Tokio

This project represents a complete journey through distributed systems
programming, from basic message handling to complex distributed
transactions, all implemented with production-quality engineering practices.
