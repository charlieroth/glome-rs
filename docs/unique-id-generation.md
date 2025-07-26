# Challenge #2: Unique ID Generation

In this challenge, you’ll need to implement a globally-unique ID generation
system that runs against Maelstrom’s unique-ids workload. Your service should
be totally available, meaning that it can continue to operate even in the
face of network partitions.

## Specification

### RPC: `generate`

Your node will receive a request message body that looks like this:

```json
{
  "type": "generate"
}
```

and it will need to return a `"generate_ok"` message with a unique ID:

```json
{
  "type": "generate_ok",
  "id": 123
}
```

The `msg_id` and `in_reply_to` fields have been removed for clarity but they
exist as described in the previous challenge. IDs may be of any type–strings,
booleans, integers, floats, arrays, etc.

## Evaluation

Build your binary and run it against Maelstrom with the following command:

```bash
maelstrom test -w unique-ids --bin ./target/debug/multi_node_broadcast --time-limit 30 --rate 1000 --node-count 3 --availability total --nemesis partition
```

This will run a 3-node cluster for 30 seconds and request new IDs at the rate
of 1000 requests per second. It checks for total availability and will induce
network partitions during the test. It will also verify that all IDs are unique.
