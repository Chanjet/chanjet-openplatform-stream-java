#!/bin/bash
# 验证脚本：输出重定向至 verify.log
LOG_FILE="verify.log"
exec > >(tee -a "$LOG_FILE") 2>&1

set -e
export JAVA_HOME=/Users/zhangliang/Library/Java/JavaVirtualMachines/graalvm-jdk-21.0.8/Contents/Home
ROOT_DIR=$(pwd)
MVN="mvn -q -DskipTests"

echo "[$(date)] Starting P2P Patch Verification..."
cd $ROOT_DIR/services/gateway-java/connector-common && $MVN install
cd $ROOT_DIR/services/gateway-java/connector-api && $MVN install
cd $ROOT_DIR/services/gateway-java/connector-core && $MVN install
cd $ROOT_DIR/sdk/java && $MVN install

echo "[$(date)] Running TCK-03..."
cd $ROOT_DIR/services/gateway-java/connector-server
mvn test -q -Dtest=TckIntegrationTest#tck03_shouldForwardToExactClientInMultiClientScenario

echo "[$(date)] SUCCESS: P2P Exact Addressing Verified."
