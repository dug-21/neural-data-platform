#!/bin/bash
cd /workspaces/neural-trader
cargo check > /tmp/compile_out.txt 2>&1
cat /tmp/compile_out.txt | grep -A 20 "error:" | head -100