# Challenge #6b: Totally-Available, Read Uncommitted Transactions

In this challenge, we’ll take our key/value store from the Single-Node
Totally-Available Transactions challenge and replicate our writes across
all nodes while ensuring a Read Uncommitted consistency model.

Read Uncommitted is an incredibly weak consistency model. It prohibits only a single anomaly:

- G0 (dirty write): a cycle of transactions linked by write-write dependencies. For
  instance, transaction T1 appends `1` to key `x`, transaction T2 appends `2` to `x`, and T1
  appends `3` to `x` again, producing the value `[1, 2, 3]`.

## Specification

Replicate writes from a node that receives a `txn` message to all other nodes.

## Evaluation

Build your binary and run it against Maelstrom with the following command:

```bash
maelstrom test -w txn-rw-register --bin ./target/debug/tarut --node-count 2 --concurrency 2n --time-limit 20 --rate 1000 --consistency-models read-uncommitted
```

Also, ensure that your transactions are totally-available in the face of network partitions:

```bash
maelstrom test -w txn-rw-register --bin ./target/debug/tarut --node-count 2 --concurrency 2n --time-limit 20 --rate 1000 --consistency-models read-uncommitted --availability total --nemesis partition
```

There’s currently an issue in the Maelstrom checker that prohibits detection of
G0 anomalies. Shout out to Ivan Prisyazhnyy for finding the issue!

However, Read Uncommitted allows almost any state to be valid so it’s likely
your system is ok and you now have a distributed transaction system ready for
the next challenge: Totally-Available, Read Committed Transactions.
