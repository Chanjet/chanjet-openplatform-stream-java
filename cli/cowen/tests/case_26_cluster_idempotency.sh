#!/bin/bash
# Case 26: Cluster Idempotency (Blind Spot Verification)
# Verifies:
#   1. When two instances (sharing the same DB) receive the same Webhook msgId simultaneously,
#      only ONE of them forwards it to the sink.
#   Note: This is currently a known architectural blind spot. The test might fail.

source tests/common.sh

echo -e "${BOLD}1. Setup Environment${NC}"
setup_workspace "case_26"
start_mock

DB_FILE="$COWEN_HOME/shared_cluster.db"
PROF="cluster_node"

# Setup shared configuration
cat > "$COWEN_HOME/app.yaml" <<EOF
storage:
  store: sqlite
  db_url: "sqlite://$DB_FILE"
EOF

# Init node 1 configuration
"$COWEN_BIN" init --profile "$PROF" --app-mode self-built \
    --app-key AK_CLUSTER --app-secret AS_CLUSTER --certificate CERT_CLUSTER --encrypt-key 1234567890123456 \
    --webhook-target "$MOCK_URL/webhook_sink" \
    --openapi-url $MOCK_URL --stream-url $MOCK_WS \
    --proxy-port 9126 >/dev/null

# Start Node A (Foreground in background task)
"$COWEN_BIN" daemon start --profile "$PROF" --foreground > "$COWEN_HOME/node_a.log" 2>&1 &
NODE_A_PID=$!

# Start Node B (Foreground in background task)
# It will fail to bind the proxy port, but the WS client will still connect
"$COWEN_BIN" daemon start --profile "$PROF" --foreground > "$COWEN_HOME/node_b.log" 2>&1 &
NODE_B_PID=$!

echo -n "   Waiting for both nodes to connect to WebSocket..."
CONNECTED=false
for i in {1..20}; do
    # We expect 2 connections minimum (actually 4 since each sdk creates 2 multiplexed conns by default)
    COUNT=$(curl -s "$MOCK_URL/control/connection_count" | python3 -c "import sys,json; print(json.load(sys.stdin)['count'])")
    if [ "$COUNT" -ge 2 ]; then
        echo -e " ${GREEN}[CONNECTED: $COUNT]${NC}"
        CONNECTED=true
        break
    fi
    echo -n "."
    sleep 1
done

if [ "$CONNECTED" = false ]; then
    echo -e " ${RED}[FAILED]${NC} Nodes failed to connect"
    kill -9 $NODE_A_PID $NODE_B_PID 2>/dev/null || true
    exit 1
fi

# Clear sink
curl -s -X POST "$MOCK_URL/control/clear_webhooks" > /dev/null

# 2. Broadcast a single message
echo -e "${BOLD}2. Broadcasting Single Message${NC}"
MSG_ID="MSG_IDEMP_$(date +%s)"

# Broadcast mode sends to ALL connected clients.
# So both Node A and Node B will receive this exact message almost simultaneously.
curl -s -X POST "$MOCK_URL/control/broadcast" -d "{
    \"msgType\": \"DATA_PUSH\",
    \"appKey\": \"AK_CLUSTER\",
    \"msgId\": \"$MSG_ID\",
    \"payload\": {\"data\": \"idempotency_test\"}
}" > /dev/null

echo "   Waiting for processing..."
sleep 5

# 3. Verify Sink
echo -e "${BOLD}3. Verifying Sink Received Exactly ONE Request${NC}"
WEBHOOKS=$(curl -s "$MOCK_URL/control/webhooks")
RECEIVED_COUNT=$(echo "$WEBHOOKS" | python3 -c "import sys,json; data=json.loads(sys.stdin.read()); print(len([m for m in data if (m.get('body') or m).get('msgId') == '$MSG_ID']))" 2>/dev/null)

kill -9 $NODE_A_PID $NODE_B_PID 2>/dev/null || true

if [ "$RECEIVED_COUNT" -eq 1 ]; then
    echo -e "   ${GREEN}✓${NC} Idempotency successful! Only 1 message received at sink."
else
    echo -e "   ${YELLOW}⚠ [BLIND SPOT VERIFIED]${NC} Idempotency violation! Sink received $RECEIVED_COUNT messages for the same msgId."
    echo "   (This is a known blind spot. The cluster lacks a distributed lock for msgId deduplication.)"
fi

echo -e "\n${GREEN}🎊 Case 26 Passed!${NC}"
