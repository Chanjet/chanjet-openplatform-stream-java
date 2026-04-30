# Cowen CLI 测试方法论 (Test Methodology)

本文档面向开发者，介绍如何维护和扩展 `cowen` CLI 的自动化测试资产。

## 1. 测试分层设计

### 1.1 单元测试 (Unit Tests)
- **位置**: 与源代码文件同级 (`src/**/*_test.rs`) 或在文件底部。
- **职责**: 验证纯逻辑函数、配置解析、加解密算法等。
- **运行**: `cargo test`

### 1.2 模拟探索性测试 (Mock Exploratory Tests) - **推荐**
- **位置**: `tests/exploratory_mock_test.sh`
- **职责**: 验证跨模块的业务链路，如“鉴权 -> 规约拉取 -> 代理转发”。
- **优势**: 
    - **脱离环境依赖**: 模拟开放平台 API，无需真实 AppKey/Secret。
    - **确定性**: Mock 服务可以精确控制响应延迟、过期时间、错误码。
    - **可复用**: 每次 CI 流程均可拉起临时环境进行回归。

## 2. 如何添加新的测试场景？

### 第一步：扩展 Mock Server
在 `tests/mock_server.py` 中：
1. 在 `do_GET` 或 `do_POST` 中添加新的路由匹配。
2. 定义预期的 JSON 响应。
3. 如果需要模拟异常，可以使用 `MOCK_STATE` 全局变量记录状态，并通过特定的控制接口（如 `/_control/...`）触发状态变更。

### 第二步：编写测试序列
在 `tests/exploratory_mock_test.sh` 中：
1. 定义新的 TC (Test Case) 编号。
2. 使用 CLI 命令执行操作。
3. 使用 `grep` 或 `python3 -c "import json..."` 对输出结果进行断言。
4. 确保在 `cleanup` 函数中能回收所有产生的资源。

## 3. 状态无关性 (Statelessness) 保证
- **临时主目录**: 始终通过 `export COWEN_HOME=$(mktemp -d)` 或 `./.cowen_test` 隔离运行。
- **随机 Profile**: 使用唯一的 `TMP_PROF` 名称，避免多个测试并发运行时的文件冲突。
- **自动清理**: 脚本结束（无论成功或失败）必须通过 `trap cleanup EXIT` 杀死后台进程并删除临时目录。

## 4. 常见问题排查
- **端口冲突**: 如果 9099 或 9098 端口被占用，测试会启动失败。
- **日志观察**: 如果测试失败，可以查看 `$COWEN_HOME/logs/` 下的 `sys.log` 或 `mock_server.log` 获取详细追踪。

---
© 2026 Chanjet Advanced Agentic Coding Team.
