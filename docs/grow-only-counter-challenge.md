# Grow Only Counter Challenge

In this challenge, you’ll need to implement a stateless, grow-only counter which
will run against Maelstrom’s g-counter workload. This challenge is different than
before in that your nodes will rely on a sequentially-consistent key/value store.

## Specification

Your node will need to accept two RPC-style message types: add & read. Your service
need only be eventually consistent: given a few seconds without writes, it should
converge on the correct counter value.

Please note that the final read from each node should return the final & correct count.

## RPC: `add`

Your node should accept add requests and increment the value of a single global counter.
Your node will receive a request message body that looks like this:

```json
{
  "type": "add",
  "delta": 123
}
```

and it will need to return an "add_ok" acknowledgement message:

```json
{
  "type": "add_ok"
}
```

## RPC: `read`

Your node should accept read requests and return the current value of the global counter.
Remember that the counter service is only sequentially consistent. Your node will receive a
request message body that looks like this:

```json
{
  "type": "read"
}
```

and it will need to return a "read_ok" message with the current value:

```json
{
  "type": "read_ok",
  "value": 1234
}
```

## Evaluation

Build your Rust binary and run it against Maelstrom with the following command:

```bash
maelstrom test -w g-counter --bin ./target/debug/grow_only_counter --node-count 3 --rate 100 --time-limit 20 --nemesis partition
```

This will run a 3-node cluster for 20 seconds and increment the counter at the
rate of 100 requests per second. It will induce network partitions during the test.
