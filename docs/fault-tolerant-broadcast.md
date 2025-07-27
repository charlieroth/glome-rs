# Challenge #3c: Fault Tolerant Broadcast

In this challenge, we’ll build on our Multi-Node Broadcast implementation,
however, this time we’ll introduce network partitions between nodes so they
will not be able to communicate for periods of time.

## Specification

Your node should propagate values it sees from `broadcast` messages to the other
nodes in the cluster—even in the face of network partitions! Values should
propagate to all other nodes by the end of the test. Nodes should only return
copies of their own local values.

## Evaluation

Build your binary and run it against Maelstrom with the following command:

```bash
maelstrom test -w broadcast --bin ./target/debug/multi_node_broadcast --node-count 5 --time-limit 20 --rate 10 --nemesis partition
```

This will run a 5-node cluster like before, but this time with a failing network! Fun!
