#!/bin/bash
cargo check 2>&1 | grep -A 20 "error:" | head -100