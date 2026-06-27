#!/bin/bash
cd crates/app/cowen-cli/tests/e2e/rust
for file in $(grep -rl "\.kill()" .); do
  if [ "$file" = "./common.rs" ]; then continue; fi
  # Use perl to properly handle the generic case
  # Replace `.kill()` with graceful_kill_child
  perl -pi -e 's/([a-zA-Z0-9_\.]+)\.kill\(\)/crate::e2e::rust::common::graceful_kill_child(\&mut $1)/g' $file
done
