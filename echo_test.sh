#!/usr/bin/env bash
# Replace `node_pkg` with the package name in your workspace.
# If the package exposes more than one binary, also add --bin <binary-name>.
cargo run --quiet --release -p echo \
<<'JSON'
{"src":"c1","dest":"n1","body":{"type":"init","msg_id":1,"node_id":"n1","node_ids":["n1"]}}
{"src":"c2","dest":"n1","body":{"type":"echo","msg_id":2,"echo":"Hello, world!"}}
{"src":"c3","dest":"n1","body":{"type":"echo","msg_id":3,"echo":"¡Hola, planeta!"}}
{"src":"c4","dest":"n1","body":{"type":"echo","msg_id":4,"echo":"Echo number three"}}
JSON
