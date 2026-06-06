#!/bin/bash
set -e
export COWEN_HOME=/Users/zhangliang/.cowen
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen init -p p1 --app-mode self_built --app-key AK_P1 --app-secret AS_P1 --encrypt-key 1234567890123456 --certificate CERT
echo "Before reset:"
ls -l $COWEN_HOME/p1* || true
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen reset -p p1
echo "After reset:"
ls -l $COWEN_HOME/p1* || true
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen profile list | grep p1 || echo "p1 completely removed from list"
