# Challenge #6a: Single-Node, Totally-Available Transactions

In this challenge, you’ll need to implement a key/value store which implements
transactions. These transactions contain micro-operations (read & write) and
the results of those operations depends on the consistency guarantees of the
challenge. Your goal is to support weak consistency while also being totally
available. We begin with a single-node service and then write a multi-node
version.

## Specification

Your node will support the `txn-rw-register` workload by implementing a key/value
store that accepts only one message. How easy, right?? This message is the `txn`
message which passes in a list of operations to perform.

Writes in this workload are unique per-key so key `100` would only ever see a write
of `1` once, a write of `2` once, etc. This helps Maelstrom to verify correctness.

## RPC: `txn`

This message passes in an array operations in the `"txn"` key. Each operation is
represented by a 3-element array containing the operation name, the integer key
to operate on, and a possibly-null integer value.

For example, your node will receive a request message body that looks like this:

```json
{
  "type": "txn",
  "msg_id": 3,
  "txn": [
    ["r", 1, null],
    ["w", 1, 6],
    ["w", 2, 9]
  ]
}
 ```

This represents three operations:

- Read from key `1`
- Write the value of `6` to key `1`
- Write the value of `9` to key `2`

In response, it should send a `txn_ok` message that contains the same operation list,
however, read (`"r"`) operations should have their value filled in with the current
value. For example, if the value of key `1` was `3` before this transaction, it should
be returned in the read operation. Non-existent keys should be returned as `null`.

```json
{
  "type": "txn_ok",
  "msg_id": 1,
  "in_reply_to": 3,
  "txn": [
    ["r", 1, 3],
    ["w", 1, 6],
    ["w", 2, 9]
  ]
}
```

## Evaluation

Build your binary and run it against Maelstrom with the following command:

```bash
maelstrom test -w txn-rw-register --bin ./target/debug/single_node_tat --node-count 1 --time-limit 20 --rate 1000 --concurrency 2n --consistency-models read-uncommitted --availability total
```

This will verify your single-node system works before we move on to
distributing our writes across nodes.
