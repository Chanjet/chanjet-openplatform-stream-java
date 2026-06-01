# Cowen 多端运行时插件架构 (Multi-Runtime Plugin Architecture) 开发计划

基于前期架构评估（支持 Dylib, RPC 双轨并存，Wasm 暂不在本期计划，且必须贯彻极致安全、OCP 和 TDD 原则），本方案将整个落地过程拆解为 4 个开发阶段（Phases）。每个阶段均可作为一个独立的里程碑交付。



## 阶段拆解与实施步骤

> [!IMPORTANT]
> **关于历史插件与运行时的核心架构决断 (Architectural Decisions)**：
> 1. **彻底放弃 Dylib 向后兼容 (Clean Break)**：不再为历史 Dylib 插件编写复杂的适配层。诸如 `cowen-search-embedding` 等官方历史插件，将**全部使用 RPC 模式进行重写**。
> 2. **坚守 Rust 稳定性**：鉴于目前基于 Rust 的插件运行极其稳定，重写 `embedding_search` 等核心官方插件时依然首选 Rust，但物理形态从 `.dylib` 转变为**基于 Stdio 交互的独立可执行二进制 (Standalone Binary)**。
> 3. **无运行时跨平台分发 (Zero-Runtime Distribution)**：无论是使用 Rust 编译的原生二进制，还是使用 **PyInstaller** 打包的 Python 产物，插件分发必须做到“无运行时依赖”。用户无需在本地预装 Python 解释器或各类 runtime，插上即用。

### Phase 1: 运行时抽象与调度器重构 (Multi-Runtime Dispatcher)

**目标**：将现有的 Dylib 强耦合加载逻辑剥离，抽象出通用的多端调度层。

*   **新增** `cli/cowen/crates/cowen-plugin/src/runtime/mod.rs`
    *   定义统一的 `PluginRuntime` Trait，包含 `start()`, `stop()`, `call_tool()`, `health_check()` 等生命周期接口。
    *   **架构防腐**：在 Trait 层面强制所有插件交互统一采用 RPC 序列化协议（如 Protobuf/JSON-RPC）。即使是 Dylib 插件，也严禁直接操作宿主内部数据结构，必须将请求序列化后发往底层代理网关，以保证多端能力注册与鉴权逻辑的绝对一致性。
*   **修改** `cli/cowen/crates/cowen-plugin/src/loader.rs`
    *   重构 `PluginManager`，引入策略模式。根据 `plugin.json` 中的 `runtime` 字段（`dylib`, `rpc`）实例化具体的 Runtime。
    *   **新增能力依赖扫描器**：在 `load` 阶段前置解析 `plugin.json` 的 `required_capabilities` 字段。拦截所有不满足当前宿主底层 API 兼容性的插件（包括使用了被废弃的底层 API 的历史 Dylib 插件），直接拒绝挂载并输出结构化告警日志。
*   **新增** `cli/cowen/crates/cowen-plugin/src/runtime/rpc.rs`
    *   实现 RPC 插件沙箱。通过 `std::process::Command` 安全拉起子进程。
    *   实现双向 Stdio 的管道流（Stream）绑定，基于 JSON-RPC 规范进行消息编解码。
    *   **强制 Stdio 通道防腐**：禁止为 MCP 协议主通道提供 TCP/HTTP 回退方案，强制将插件约束为必须通过 Stdio 交互的本地子进程，确保零信任架构下的绝对生命周期收敛与凭证注入安全。
    *   引入子进程宕机监控与优雅重启机制（EOF 侦测）。

---

### Phase 2: 安全上下文与凭证零落盘注入 (Security & Env Injection)

**目标**：在拉起子进程时，实现内部旁路网关通信凭证的动态注入，以及多租户形态支持。

*   **新增** `cli/cowen/crates/cowen-plugin/src/security/env_injector.rs`
    *   在 `spawn` 子进程的瞬间，宿主在内存中随机生成 `COWEN_BRIDGE_TOKEN`（旁路网关临时通信凭证）。
    *   **【关键安全底线】**：注入的 Token **仅限** Host 与 Plugin 之间的内部协商校验使用。**绝对禁止**将运行时真实的业务敏感数据（如 `<APP_SECRET>`, `<ACCESS_TOKEN>`, 用户密码等）通过环境变量或任何方式注入给插件。插件作为无状态空壳，若需动用真实的敏感资产，必须携带 `COWEN_BRIDGE_TOKEN` 呼叫宿主的 Native API，由宿主在安全的内存区域内代为进行签名计算或请求转发。
    *   **动态配置合并**：优先读取 `plugin.json` 的 `default_config`，再提取宿主当前活跃 `Profile` 配置文件中针对该插件的重写配置。两者在内存合并后统一注入为环境变量（如 `COWEN_PLUGIN_LOG_LEVEL`），确保插件状态随 Profile 无缝切换。
*   **修改** `cli/cowen/crates/cowen-plugin/src/runtime/rpc.rs`
    *   挂载 `env_injector.rs`，利用操作系统的 `envs()` API 安全注入内存。
    *   实现 Tenant Morphing：根据 `tenant_mode` (`exclusive` 或 `shared`) 控制进程是单例复用还是多开硬隔离。

---

### Phase 3: 宿主本地 API 代理网关 (Native API Server)

**目标**：暴露微型本地服务端，让无状态的“空壳插件”能回调宿主底座进行网络代发和能力拉取。

*   **新增** `cli/cowen/crates/cowen-server/src/api/plugin_gateway.rs`
    *   在本地随机端口或 Unix Domain Socket 上启动轻量级 HTTP/RPC 服务。
    *   **命名空间与能力分组 (Namespacing)**：Native API 的路由设计必须与 `plugin.json` 中的 `required_capabilities` 能力矩阵保持严格的映射分组，避免根路径膨胀与混乱：
        *   **网络代理组 (`/v1/network/*`)**：实现 `/v1/network/registry` 和 `/v1/network/call`，负责拦截业务请求、校验 `X-Bridge-Token`、注入签名并向开放平台代发。
        *   **监控诊断组 (`/v1/doctor/*` & `/v1/monitor/*`)**：未来预留，用于插件查询宿主健康状态、排障日志或上报指标。
        *   **沙箱存储组 (`/v1/fs/*` & `/v1/kv/*`)**：未来预留，用于向插件暴露受限的文件读写或向量缓存查询能力。
    *   **全局拦截鉴权 (RBAC Middleware)**：实现统一的 HTTP Middleware。对于每一个进入分组路由的请求，Middleware 必须先提取 `X-Bridge-Token` 锁定插件实例内存上下文，再校验当前被访问的路由分组（如 `/v1/network/*`）是否已包含于该插件在加载时生成的**授权白名单 (Authz Whitelist)** 中。若越权访问，则拦截并返回 `HTTP 403 Forbidden`。
*   **新增** `cli/cowen/crates/cowen-server/src/proxy.rs`
    *   实现网关代理核心逻辑：对合法的插件请求，拼接真实 OpenAPI URL，附带正确的安全签名（RSA/HMAC），向畅捷通开放平台发起真实 HTTP 请求并透传结果。

---

### Phase 4: CLI 动态路由与回退拦截 (Dynamic CLI Fallback)

**目标**：让用户能在终端像执行原生命令一样执行插件命令。

*   **修改** `cli/cowen/src/cli/mod.rs`
    *   修改现有的 Clap (CLI 解析器) 异常处理流程。当遇到 `Unknown Command` 时不直接退出。
*   **新增** `cli/cowen/src/cli/fallback_parser.rs`
    *   快速扫描本地已安装插件的 `plugin.json`，查找匹配的 `cli_commands` 扩展节点。
    *   构建动态参数校验器（基于 JSON Schema 校验终端输入参数）。
    *   校验通过后，将终端控制权、Stdio 直接移交给 `PluginManager`，由对应的插件运行时（Dylib/RPC）代为执行。

---

## 验证与验收计划 (Verification Plan)

所有代码严格遵循 **TDD (Test-Driven Development)** 原则开发，无测试用例严禁合并。

### 1. 自动化测试 (Automated Tests)
*   **单元测试**：针对 `PluginManager` 和两种不同的 `Runtime` 编写 Mock 测试，验证 `plugin.json` 解析和 Trait 方法的路由正确性。
*   **集成测试**：编写一个简单的 Go 或 Python `echo-plugin` 作为测试 Fixture。在测试环境中拉起宿主，验证该空壳插件能否成功通过 RPC 调用宿主的本地 API 网关。
*   **安全防御测试**：针对 `/v1/api/call` 构造无 Token 或非法 Token 的伪造请求，验证代理网关的强拦截能力。

### 2. E2E 验证 (Manual/E2E Verification)
*   构建完整的 `cowen` CLI 二进制。
*   在 `~/.cowen/plugins/` 目录放置一个真实的 MCP 中继插件（需实现声明式契约）。
*   在终端执行该插件声明的动态命令，验证其是否能被成功拉起、内存环境变量注入是否准确，并最终能调用到底层的流连接器能力。
