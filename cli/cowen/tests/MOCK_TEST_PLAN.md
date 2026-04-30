# Cowen CLI Mock 探索性测试计划 (Exploratory Test Plan)

## 1. 测试目标
通过模拟远程开放平台环境，对 `cowen` CLI 的核心链路进行全自动化的闭环验证。确保在脱离真实生产环境下，系统的鉴权、动态规约发现、守护进程、代理转发及异常处理逻辑依然稳健。

## 2. 测试资产说明
- **`tests/mock_server.py`**: 核心 Mock 服务。
    - **HTTP (9099)**: 提供 Token 生成、OpenAPI 规约、应用凭据同步接口。
    - **WebSocket (9098)**: 模拟 Stream 推送通道。
    - **Webhook 推送器**: 能够主动向 CLI 守护进程推送 AppTicket。
- **`tests/exploratory_mock_test.sh`**: 自动化编排脚本。负责拉起 Mock 服务、隔离环境、执行测试序列并进行结果断言。

## 3. 测试用例矩阵 (Test Case Matrix)

| ID | 模块 | 测试场景 | 预期结果 |
| :--- | :--- | :--- | :--- |
| TC-01 | Init | 使用 Mock URL 初始化 Profile | 成功创建配置文件且 Vault 存储正确。 |
| TC-02 | Api | 动态拉取远程 Spec 并刷新缓存 | `api list` 能正确显示 Mock 接口。 |
| TC-03 | Api | 基于语义搜索 Mock 接口 | AI 搜索能精准命中 Mock 定义。 |
| TC-04 | Auth | Oauth2 令牌刷新流程 | 令牌过期后，Daemon 自动触发换票。 |
| TC-05 | Auth | Self-Built 模式 Ticket 采集 | 通过 Webhook 成功接收 Mock 推送的 Ticket。 |
| TC-06 | Proxy | 经过 Daemon 代理的 API 调用 | 自动注入 Mock 令牌，流量转发至 Mock 服务。 |
| TC-07 | DLQ | 模拟转发失败进入死信队列 | `dlq list` 可见异常记录。 |
| TC-08 | System | 全量状态诊断诊断 | `status --all` 能正确报告 Mock 环境状态。 |

## 4. 执行方法
```bash
cd cli/cowen
# 确保已构建二进制产物
make build
# 运行 Mock 探索测试
bash tests/exploratory_mock_test.sh
```

## 5. 隔离性设计
- **`COWEN_HOME`**: 所有测试均在 `./tests/.cowen_test` 目录下运行，不干扰用户本地配置。
- **端口隔离**: 使用非标准端口 (9099, 9098) 避免干扰本地开发环境。

---
© 2026 Chanjet Advanced Agentic Coding Team.
