#!/bin/sh
for _ in {1..13370}; do
    ./target/release/examples/client-simulator --imei="$(openssl rand -hex 30 | head -c 15)" &
done
