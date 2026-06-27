#!/bin/bash
export RUST_LOG=debug
export COWEN_BUILD_CLIENT_ID=dummy
cargo build --release -p cowen-cli -p cowen-daemon -p cowen-search-embedding -p cowen-mcp-plugin
export COWEN_BIN=$(pwd)/target/release/cowen
bash crates/app/cowen-cli/tests/e2e/scripts/case_67_init_daemon_activation.sh
