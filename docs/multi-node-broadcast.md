# Challenge #3b: Multi‑Node Broadcast

In this challenge, you’ll build on your Single‑Node Broadcast implementation
and replicate your messages across a cluster that has no network partitions.

## Specification

Your node should propagate values it sees from `broadcast` messages to the
other nodes in the cluster. It can use the `topology` passed to your node in
the topology message or you can build your own topology.

The simplest approach is to send a node’s entire dataset on every message;
however, this is not practical in a real‑world system. Instead, try to send
data more efficiently, as if you were building a real broadcast system.

Values should propagate to all other nodes within a few seconds.

## Evaluation

Build your binary and run tests with:

```bash
maelstrom test -w broadcast --bin ./target/debug/multi_node_broadcast --node-count 5 --time-limit 20 --rate 10
```

This command runs a 5‑node cluster for 20 seconds and broadcasts messages at a rate of 10 messages per second.

The test validates that all values sent by broadcast messages are present on all nodes—i.e. full propagation.
