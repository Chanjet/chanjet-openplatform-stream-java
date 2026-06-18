# Cowen CLI 待办事项与技术债清单 (TODO & Technical Debt)


## 🧪 P0: 覆盖率提升与功能测试逃逸补充 (E2E & Coverage Gaps)
*核心目标：针对去重后仍处于低于 30% 低覆盖率的关键功能模块，补齐 E2E 集成测试与单元测试。*

### 1. 🔌 MCP 插件引擎模块 (Model Context Protocol Plugin)
* **目标文件**：
  * [openapi.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-mcp-plugin/src/schema/openapi.rs) (Lines: 313, Coverage: 0.00%)
  * [dynamic.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-mcp-plugin/src/handlers/dynamic.rs) (Lines: 127, Coverage: 0.00%)
  * [validator.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-mcp-plugin/src/schema/validator.rs) (Lines: 113, Coverage: 0.00%)
  * [initialize.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-mcp-plugin/src/handlers/initialize.rs) (Lines: 28, Coverage: 0.00%)
  * [api.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-mcp-plugin/src/handlers/api.rs) (Lines: 213, Coverage: 22.54%)
  * [tools.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-mcp-plugin/src/handlers/tools.rs) (Lines: 158, Coverage: 25.32%)
* **待补测试**：
  * 补齐 MCP 握手 `initialize` 协议单元测试。
  * 模拟 LLM 客户端发送 `tools/call` E2E 用例，触发动态工具分发 `dynamic.rs` 和 OpenAPI 校验器 `validator.rs` 的运行。
  * 提供 Mock OpenAPI Schema 用于协议转换 `openapi.rs` 静态自测。

### 2. 🧠 AI 向量引擎与语义检索常驻守护进程 (Search Embedding & AI Engine)
* **目标文件**：
  * [main.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-search-embedding/src/main.rs) (Lines: 252, Coverage: 0.00%)
  * [engine.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/plugins/cowen-search-embedding/crates/cowen-ai/src/engine.rs) (Lines: 74, Coverage: 0.00%)
* **待补测试**：
  * 新增 E2E 用例启动 `cowen-search-embedding` 常驻进程，校验进程平稳拉起与回收。
  * 提供微型 ONNX 测试权重资产，打通语义检索和向量生成推理引擎 `engine.rs`。

### 3. 🕸️ WASM 插件引擎沙箱运行时与 Host Facade 接口 (Wasm Runtime)
* **目标文件**：
  * [wasm_runtime.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/app/cowen-server/src/daemon/wasm_runtime.rs) (Lines: 379, Coverage: 21.11%)
  * [native_auth.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/adapters/cowen-wasm-facade/src/native_auth.rs) (Lines: 111, Coverage: 2.70%)
  * [native_config.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/adapters/cowen-wasm-facade/src/native_config.rs) (Lines: 67, Coverage: 4.48%)
  * [lib.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/adapters/cowen-wasm-facade/src/lib.rs) (Lines: 105, Coverage: 20.00%)
* **待补测试**：
  * 编写恶意/越界的 WASM 测试插件，强行触发沙箱内存溢出与异常终止保护分支。
  * 补齐集成 WASM 插件通过 `host_get_config()` 和宿主认证回调获取凭证的完整交互链路测试。

### 4. 🔑 Auth 多步骤授权编排与测试执行器补齐
* **目标文件**：
  * [orchestrator.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/services/cowen-auth/src/lifecycle/orchestrator.rs) (Lines: 220, Coverage: 0.00%)
  * [shared.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/services/cowen-auth/src/provider/shared.rs) (Lines: 49, Coverage: 0.00%)
  * [provider/mod.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/services/cowen-auth/src/provider/mod.rs) (Lines: 80, Coverage: 26.25%)
  * `services/cowen-auth/tests/` 集成测试代码 (共计约 320 行, Coverage: 0.00%)
* **待补测试**：
  * 修复集成测试扫描，确保 `cargo test` 在全量跑测时能执行到 `services/cowen-auth/tests/` 下的 `test_oauth2_refresh.rs`、`test_listener.rs` 等文件。
  * 引入时间 Mock (加速时钟)，测试 `orchestrator.rs` 后台循环对 Token 过期自愈刷新的状态转移。

### 5. 💾 持久化存储与版本演进迁移 (Store & Migration)
* **目标文件**：
  * [file/migration.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/services/cowen-store/src/file/migration.rs) (Lines: 108, Coverage: 0.00%)
  * [migration.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/services/cowen-store/src/migration.rs) (Lines: 102, Coverage: 0.00%)
  * [file/sealed.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/services/cowen-store/src/file/sealed.rs) (Lines: 106, Coverage: 0.00%)
  * [hybrid.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/services/cowen-store/src/hybrid.rs) (Lines: 102, Coverage: 0.00%)
* **待补测试**：
  * 提供历史旧版本 Schema 数据资产，测试数据库/文件元数据升级迁移。
  * 补齐封存归档 `sealed.rs` 单测，并新增激活混合存储 `hybrid.rs` 的集成 E2E 用例。

### 6. 🛠️ 跨平台底层与进程安全管理 (Infra & System Process)
* **目标文件**：
  * [process.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/core/cowen-infra/src/process.rs) (Lines: 35, Coverage: 17.14%)
  * [pki.rs](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/crates/core/cowen-infra/src/pki.rs) (Lines: 71, Coverage: 11.27%)
* **待补测试**：
  * 编写单元测试强杀失控子进程进程树，覆盖 `process::kill_process_tree` 中针对复杂挂起子进程的回收测试。
  * 补齐 PKI 多算法协商和异常证书链拒绝分支测试。


## 🟢 P1: 核心可靠性增强 (Reliability Enhancements)
*核心目标：提升端到端消息投递可靠性，防止消息在服务端或客户端流转中断时丢失。*

## 🗄️ 已归档完成事项 (Archived Completed Items)

所有历史已完成的待办事项与解耦重构任务均已物理搬迁，详细归档记录请参见：
- 📄 **[已完成任务归档记录表](archive/completed_tasks.md)** *(包含跨平台架构重构、历史高危安全修复、多租户令牌自愈、cowen-doctor 插件化解耦、能力体系演进及 Windows Service 企业级集成等里程碑成果)*

---

## 📂 附件 (Attachments)

- 📄 **[架构分析与核心源码审计报告](archive/ARCHITECTURE_AUDIT_REPORT.md)**：包含 12 个 Crate 层级耦合度精细审计、源码精读、SOLID 原则落地以及具体解耦建议。
