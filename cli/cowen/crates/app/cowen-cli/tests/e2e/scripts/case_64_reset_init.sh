#!/bin/bash
set -e
export COWEN_HOME=/Users/zhangliang/.cowen
echo "1. Initialize p1 with self_built mode"
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen init -p p1 --app-mode self_built --app-key AK_P1 --app-secret AS_P1

echo "2. Check if p1 config exists"
cat $COWEN_HOME/p1.yaml

echo "3. Reset p1"
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen reset -p p1

echo "4. Init p1 again without app_key/secret"
/Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/target/debug/cowen init -p p1

echo "5. Check if it restored AK_P1"
cat $COWEN_HOME/p1.yaml
