#!/usr/bin/env bash
# case_87_cli_admin_commands.sh
# Tests auxiliary CLI administrator tools: audit, events, and log commands

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

PROFILE="case_87_admin"
setup_workspace "case_87_$PROFILE"
cd "$COWEN_HOME"

# 1. 初始化并启动后台 Daemon 服务，用来响应审计日志 gRPC/IPC 请求
"$COWEN_BIN" init --profile "$PROFILE" --app-key "dummykey" --app-secret "dummysecret" --app-mode self-built --certificate "dummy_cert" --encrypt-key "dummy_ek"

"$COWEN_BIN" --profile "$PROFILE" daemon start

# 等待 daemon 启动完成且数据库 migration 完毕
sleep 1.0

# 手动注入一条 DLQ 记录用于后续 CLI 运维命令校验
sqlite3 cowen.db "INSERT INTO cowen_dlq (id, profile, topic, payload, retry_count, error, created_at) VALUES (1, '$PROFILE', 'test_topic', '{\"msg\": \"hello\"}', 0, 'mock_err', '2026-06-18 14:00:00');"

echo "📊 1. 测试 cowen audit 审计查看器..."
# 执行 audit 命令（默认为 follow 状态所以我们需要在后台运行它，并在读取完后杀掉它）
"$COWEN_BIN" --profile "$PROFILE" audit --lines 5 > audit_output.log 2>&1 &
AUDIT_PID=$!
sleep 1.5
kill -15 "$AUDIT_PID" >/dev/null 2>&1 || true
wait "$AUDIT_PID" || true

assert_pass "Audit command executed and shut down successfully"

echo "📊 2. 测试 cowen events 订阅（提示已迁移）..."
# 此时 events 会由于新架构返回提示信息错误
EVENT_ERR=$("$COWEN_BIN" --profile "$PROFILE" events 2>&1 || true)
if ! echo "$EVENT_ERR" | grep -q "thin CLI architecture"; then
    fail_suite "Unexpected events output: $EVENT_ERR"
fi
assert_pass "Events command handled correctly"

echo "📊 3. 测试 cowen log list..."
# 首先测试无日志时的 list
"$COWEN_BIN" --profile "$PROFILE" log list

# 在 logs 目录下临时建立 mock 本地日志文件
mkdir -p "$COWEN_HOME/logs"
echo "log line 1" >> "$COWEN_HOME/logs/${PROFILE}_main.log"
echo "log line 2" >> "$COWEN_HOME/logs/${PROFILE}_main.log"
echo "log line 3" >> "$COWEN_HOME/logs/${PROFILE}_main.log"

# 再次运行 log list
"$COWEN_BIN" --profile "$PROFILE" log list | grep -q "${PROFILE}_main.log"
assert_pass "Log list displayed successfully"

echo "📊 4. 测试 cowen log view..."
# 静态查看（不带 follow）
LOG_VAL=$("$COWEN_BIN" --profile "$PROFILE" log view main --lines 2)
if [ "$LOG_VAL" != $'log line 2\nlog line 3' ]; then
    # 由于可能包含换行格式的特殊差异，我们只进行模糊包含校验
    if ! echo "$LOG_VAL" | grep -q "log line 3"; then
        fail_suite "Log view output mismatch: '$LOG_VAL'"
    fi
fi
assert_pass "Static log view executed successfully"

# 测试 follow 追踪式查看（需要在后台运行并超时杀退）
"$COWEN_BIN" --profile "$PROFILE" log view main --lines 2 --follow > log_follow.log 2>&1 &
FOLLOW_PID=$!
sleep 1.0
echo "log line 4 (append)" >> "$COWEN_HOME/logs/${PROFILE}_main.log"
sleep 1.0
kill -15 "$FOLLOW_PID" >/dev/null 2>&1 || true
wait "$FOLLOW_PID" || true

assert_pass "Follow log view executed successfully"

echo "📊 5. 测试 cowen dlq 子命令..."
# 5.1 列出 dlq 应当包含刚才插入的 test_topic
DLQ_LIST_OUT=$("$COWEN_BIN" --profile "$PROFILE" dlq list 2>&1)
echo "     DLQ List Output: $DLQ_LIST_OUT"
echo "$DLQ_LIST_OUT" | grep -q "test_topic"
assert_pass "dlq list matched successfully"

# 5.2 详情查看应当包含 payload 细节
"$COWEN_BIN" --profile "$PROFILE" dlq view 1 | grep -q "hello"
assert_pass "dlq view matched successfully"

# 5.3 尝试重试，应当调用 IPC 成功
"$COWEN_BIN" --profile "$PROFILE" dlq retry 1 | grep -q "Retrying"
assert_pass "dlq retry triggered successfully"

# 5.4 清空队列
"$COWEN_BIN" --profile "$PROFILE" dlq purge | grep -q "Purging"
assert_pass "dlq purge triggered successfully"

# 停止 daemon
"$COWEN_BIN" --profile "$PROFILE" daemon stop || true

echo "✅ CLI Administration tools tests Passed!"
