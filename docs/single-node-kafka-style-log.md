# Single-Node Kafka-Style Log

In this challenge, you’ll need to implement a replicated log service similar to
Kafka. Replicated logs are often used as a message bus or an event stream.

This challenge is broken up in multiple sections so that you can build out your
system incrementally. First, we’ll start out with a single-node log system and
then we’ll distribute it in later challenges.

## Specification

Your nodes will need to store an append-only log in order to handle the `"kafka"`
workload. Each log is identified by a string key (e.g. `"k1"`) and these logs
contain a series of messages which are identified by an integer offset.
These offsets can be sparse in that not every offset must contain a message.

Maelstrom will check to make sure several anomalies do not occur:

- **Lost writes**: for example, a client sees offset 10 but not offset 5.
- **Monotonic increasing offsets**: an offset for a log should always be increasing.

There are no recency requirements so acknowledged `send` messages do not need
to return in `poll` messages immediately.

## RPC: `send`

This message requests that a `"msg"` value be appended to a log identified by `"key"`.
Your node will receive a request message body that looks like this:

```json
{
  "type": "send",
  "key": "k1",
  "msg": 123
}
```

In response, it should send an acknowledge with a `send_ok` message that contains
the unique offset for the message in the log:

```json
{
  "type": "send_ok",
  "offset": 1000
}
```

## RPC: `poll`

This message requests that a node return messages from a set of logs starting
from the given offset in each log. Your node will receive a request message
body that looks like this:

```json
{
  "type": "poll",
  "offsets": {
    "k1": 1000,
    "k2": 2000
  }
}
```

In response, it should return a `poll_ok` message with messages starting from
the given offset for each log. Your server can choose to return as many messages
for each log as it chooses:

```json
{
  "type": "poll_ok",
  "msgs": {
    "k1": [
      [1000, 9],
      [1001, 5],
      [1002, 15]
    ],
    "k2": [
      [2000, 7],
      [2001, 2]
    ]
  }
}
```

## RPC: `commit_offsets`

This message informs the node that messages have been successfully processed up
to and including the given offset. Your node will receive a request message body
that looks like this:

```json
{
  "type": "commit_offsets",
  "offsets": {
    "k1": 1000,
    "k2": 2000
  }
}
```

In this example, the messages have been processed up to and including offset `1000`
for log `k1` and all messages up to and including offset `2000` for `k2`.

In response, your node should return a `commit_offsets_ok` message body to
acknowledge the request:

```json
{
  "type": "commit_offsets_ok"
}
```

## RPC: `list_committed_offsets`

This message returns a map of committed offsets for a given set of logs.
Clients use this to figure out where to start consuming from in a given log.

Your node will receive a request message body that looks like this:

```json
{
  "type": "list_committed_offsets",
  "keys": ["k1", "k2"]
}
```

In response, your node should return a `list_committed_offsets_ok` message body
containing a map of offsets for each requested key. Keys that do not exist on
the node can be omitted.

```json
{
  "type": "list_committed_offsets_ok",
  "offsets": {
    "k1": 1000,
    "k2": 2000
  }
}
```

## Evaluation

Build your binary as `kafka` and run it against Maelstrom with the following command:

```bash
maelstrom test -w kafka --bin ./target/debug/kafka --node-count 1 --concurrency 2n --time-limit 20 --rate 1000
```

This will run a single node for 20 seconds with two clients. It will validate that messages are queued and committed properly.
