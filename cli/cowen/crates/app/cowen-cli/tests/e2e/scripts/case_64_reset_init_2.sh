#!/bin/bash
set -e
export COWEN_HOME=/Users/zhangliang/.cowen
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen init -p p2 --app-mode self_built --app-key AK_P2 --app-secret AS_P2 --encrypt-key 1234567890123456 --certificate CERT
echo "Initial p2.yaml:"
cat $COWEN_HOME/p2.yaml
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen reset -p p2
echo "Init p2 again with self_built but NO keys:"
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen init -p p2 --app-mode self_built
echo "After second init p2.yaml:"
cat $COWEN_HOME/p2.yaml
