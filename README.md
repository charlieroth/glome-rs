# Gossip Glomers

Rust workspace containing solutions to
[Fly.io's Gossip Glomers](https://fly.io/dist-sys/) distributed systems
challenges.

## Overview

Gossip Glomers is a series of distributed systems programming challenges. These
challenges use the Maelstrom platform to simulate distributed systems
environments, inject network failures, and verify that implementations maintain
correctness under various failure conditions.

This workspace provides Rust implementations for all core challenges,
demonstrating practical distributed systems concepts including consensus
algorithms, gossip protocols, fault-tolerant broadcasting, and replicated data
structures.

## Learning Objectives

TODO

## Architecture

The project is organized as a Cargo workspace with challenge implementations and
1 core library (`maelstrom/src/*.rs`):

## Technical Features

TODO

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
