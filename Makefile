echoer:
	maelstrom test -w echo --bin ./target/debug/echo --node-count 1 --time-limit 10

unique-id:
	maelstrom test -w unique-ids --bin ./target/debug/uniqueids --time-limit 30 --rate 1000 --node-count 3 --availability total --nemesis partition

snb:
	maelstrom test -w broadcast --bin ./target/debug/single_node_broadcast --node-count 1 --time-limit 20 --rate 10

mnb:
	maelstrom test -w broadcast --bin ./target/debug/multi_node_broadcast --node-count 5 --time-limit 20 --rate 10

ftb:
	maelstrom test -w broadcast --bin ./target/debug/multi_node_broadcast --node-count 5 --time-limit 20 --rate 10 --nemesis partition

eb-one:
	maelstrom test -w broadcast --bin ./target/debug/efficient_broadcast --node-count 25 --time-limit 20 --rate 100 --latency 100

eb-two:
	maelstrom test -w broadcast --bin ./target/debug/efficient_broadcast --node-count 25 --time-limit 20 --rate 100 --latency 100

goc:
	maelstrom test -w g-counter --bin ./target/debug/grow_only_counter --node-count 3 --rate 100 --time-limit 20 --nemesis partition
