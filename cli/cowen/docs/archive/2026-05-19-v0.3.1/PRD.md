# cli/cowen v0.3.1 产品需求文档 (PRD)

## 1. 版本概述 (Version Overview)
v0.3.1 版本致力于增强 `cowen` 在生产环境中的 **可观测性 (Observability)**、**运维便利性 (Maintainability)** 以及 **架构灵活性 (Architectural Flexibility)**。

## 2. 核心特性 (Core Features)

### 2.1 配置热重载 (Config Hot-Reload)
*   **需求背景**: 生产环境下，修改日志级别或 Webhook 地址时不希望重启 Daemon 进程，以避免连接中断。
*   **功能描述**: 
    *   Daemon 进程支持监听 `app.yaml` 的文件变更。
    *   支持通过 `SIGHUP` 信号触发配置重新加载。
    *   重载过程应确保已建立的 WebSocket 连接和代理请求不受影响。

### 2.2 本地监控与健康 API (Local Metrics & Health API)
*   **需求背景**: 方便 K8s 存活探针检测及本地 Prometheus 指标抓取。
*   **功能描述**:
    *   在本地管理端口（默认 127.0.0.1）暴露 `/health` 接口，返回存储连通性、Daemon 存活状态。
    *   暴露 `/metrics` 接口（Prometheus 格式），统计当前连接数、请求成功率、DLQ 堆积量、Token 剩余寿命。

### 2.3 环境自检工具 (Environment Doctor)
*   **需求背景**: 快速排查因网络、权限或中间件导致的运行故障。
*   **功能描述**:
    *   新增 `cowen system doctor` 命令。
    *   自动化检查：Redis/MySQL/Postgres 连通性、开放平台网关延迟、证书有效期、本地写权限、系统限制（ulimit）等。
    *   输出格式化的诊断报告，给出修复建议。

### 2.4 API 搜索插件化 (Pluggable Search Engine)
    *   **架构解耦**: 抽象 `SearchProvider` 接口，支持多种搜索策略。
    *   **插件自动发现机制**: 系统支持在指定的插件目录下自动扫描符合操作系统规范（macOS: `.dylib/.so`, Linux: `.so`, Windows: `.dll`）的二进制插件，并支持自动识别与分类多种不同类型、不同能力的插件（例如：多种检索算法、多种语义 Embedding 模型）。
    *   **显式插件管理与映射**: 用户必须在配置文件 `search.plugins` 部分通过映射表明确指定启用的插件及其路径。插件定义必须包含 `type` 或 `capability` 元数据以支持多插件分类管理。
        *   配置结构示例：
            ```yaml
            search:
              plugins:
                - name: "embedding_model_a"
                  type: "embedding"
                  path: "./dist_assets/libmodel_a.dll"
                - name: "keyword_ranker_b"
                  type: "ranking"
                  path: "./dist_assets/libranker_b.dll"
              enabled: ["embedding_model_a", "keyword_ranker_b"] # 支持显式启用多种能力插件
            ```
        *   系统根据 `enabled` 列表中的插件名称进行精确加载，实现多插件共存与能力组合。
    *   **索引按需触发**: 当且仅当指定的搜索插件被显式生效时，系统才会触发对应的本地 API 文档向量化索引构建。
    *   **按需分发**: 将搜索实现打包为独立的动态链接库，由用户显式管理其物理分发与存放路径。
    *   **分发优化**: 若 `enabled` 插件无法定位或加载失败，系统自动降级到内置的 `string_matching` 搜索策略并记录详细的错误日志。



### 2.5 DLQ 存储异常 Panic 防护 (DLQ Init Panic Protection)
*   **需求背景**: 避免因底层存储服务（如 SQLite/Redis）启动连接失败、写保护或死锁导致整个 `cowen` 进程异常 Panic 崩溃。
*   **功能描述 / 业务规则**:
    *   在 `Forwarder` 初始化以及 `DlqStore::new` 实例化时，如果发生存储底层故障，系统不应 Panic 崩溃。
    *   通过 `Result` 链式传递机制，安全地向调用层（Daemon 守护线程、DLQ 重试命令）传递错误，由调用层捕获异常，并在终端及日志中输出清晰的故障提示。
*   **技术设计**:
    *   修改 `Forwarder::new` 方法签名，使其返回 `CowenResult<Self>`，并利用 `?` 向上层传播 `DlqStore` 的初始化异常。
    *   在 `bridge.rs` 的 Webhook 转发器设置处，以及 `dlq.rs` 的 DLQ 重试入口处捕获错误，替换原先的 `unwrap` 导致的 Panic。
*   **影响范围评估**:
    *   仅涉及 `cowen-server` 及命令行内部的初始化逻辑（`forwarder.rs`、`bridge.rs`、`cmd/dlq.rs`）。对协议契约、SPI 契约以及与其他微服务的网络通信无任何破坏性或级联影响。
*   **关键技术选项与方案确认**:
    *   **方案选择**: 采用 Rust 惯用的 `Result` 链式向上传播机制。此方法相比内部降级机制更易于暴露早期配置故障，且遵循 Rust 错误处理最佳实践。

### 2.6 智能动态 Token 检查与刷新策略 (Intelligent Dynamic Token Check Strategy)
*   **需求背景**: 目前后台凭证自愈监控和刷新引擎采用硬编码的 10 分钟检测间隔。针对短有效期 Token 容易出现刷新空窗期；针对长有效期 Token（如自建模式 2 小时、OAuth2 模式 7 天）频繁的磁盘/网络检查造成性能和日志冗余，且易触发请求风暴。
*   **功能描述 / 业务规则**:
    *   **自适应检测步长**: 检查间隔不应硬编码，应根据当前 AccessToken 的剩余生存周期自适应计算：`next_check = (expires_at - now) * 0.8`。
    *   **安全区间保护**: 设置检测间隔下限为 `30` 秒（避免短周期或过期状态下的快速自旋），上限为 `3600` 秒（确保状态的最终一致性与故障及时恢复）。
    *   **引入随机抖动 (Jitter)**: 计算出的下一次延迟增加 `±rand(0..60)` 秒的随机偏移，防止分布式/多实例环境下的惊群效应引发对开放平台网关的突发性网络请求。
*   **技术设计**:
    *   重构 `renewer.rs` 中的主动检测逻辑 and `bridge.rs` 中的维护检查逻辑，动态计算下一次 Sleep 周期。
    *   引入 `rand` 库实现抖动随机偏置。
*   **影响范围评估**:
    *   主要涉及 `cowen-server` 内的后台轮询调度策略，不影响 `cowen-auth` 底层凭证服务和外部认证物理模型。
*   **关键技术选项与方案确认**:
    *   **比例系数**: 选择 80% 生存期触发（`* 0.8`）能够最大化利用 Token 有效期，并在出现网络抖动时保留 20% 生存期的重试窗口。
    *   **上限保护与抖动**: 1小时上限与 60s 随机抖动能在高并发微服务场景下天然平滑流量。

### 2.7 核心依赖去上帝化重构 (Decoupling & Splitting cowen-common)
*   **需求背景**: 现有 `cowen-common` 模块过度臃肿，承载了系统配置、网络 I/O、安全加密、底层工具及核心数据模型，成为“上帝模块”，极易引起后续新 Crate 的跨域源码级循环依赖，阻碍物理隔离架构的实施。
*   **功能描述 / 业务规则**:
    *   **职责重定义**: `cowen-common` 仅保留最基础的核心数据模型（Models）与最小化的接口契约/SPI（Traits），退化为完全稳定的契约层。
    *   **公共工具沉降**: 将原 `cowen-common` 中的系统底层工具、加密组件、辅助网络函数等与具体业务无关的逻辑，剥离并下沉至独立的 `cowen-infra` 或 `cowen-utils` 工具级 Crate 中。
    *   **独立编译与隔离**: 确保重新划分后的 `cowen-common` 不包含任何可能引起逆向引用的网络或高层模块逻辑，能够独立被各业务 Crate 引用，满足编译层面的完全隔离。
*   **技术设计**:
    *   在 Cargo Workspace 中规划底层的工具 Crate，移动通用辅助函数。
    *   清理 `cowen-common` 依赖树，仅保留基础依赖（如 `serde`, `chrono` 等），移除对复杂网络客户端等高层库的非必要强绑定。
*   **影响范围评估**:
    *   作为底层架构级重构，此变动将触及依赖 `cowen-common` 的所有上层 Cargo Crates（如 `cowen-server`, `cowen-auth`, `cowen-store` ）。这需要我们在编译层仔细重构依赖声明和 `use` 导入，但对上层业务功能行为无任何破坏性改变。
*   **关键技术选项与方案确认**:
    *   **方案选择**: 采用“公共工具库下沉 + 契约层极简化”策略。此方案是微服务与多 Crate 架构中解耦上帝模块的标准演进路径，能提供最纯净的底层隔离保障。

## 3. 技术约束 (Technical Constraints)
*   **物理隔离架构 (Physical Crate Isolation)**: 为防止代码腐化和模块越界调用，v0.3.1 的所有新特性必须封装在独立的 Cargo Crate 中（如 `cowen-config`, `cowen-monitor`, `cowen-doctor`, `cowen-search`）。核心程序通过最小化 Trait SPI 引用这些 Crate，严格禁止跨域的源码级循环依赖。
*   **稳定性**: 配置热重载严禁引起内存泄漏或进程崩溃。
*   **安全性**: 管理 API 必须严格绑定在 `127.0.0.1`，禁止外网访问。
*   **兼容性**: 插件加载机制需适配 Linux、macOS 和 Windows。

## 4. 验收标准 (Acceptance Criteria)
*   修改日志级别后，Daemon 日志输出立即生效而无需重启。
*   `curl 127.0.0.1:<port>/metrics` 能返回正确的监控数据。
*   `cowen system doctor` 能准确识别出错误的数据库配置。
*   当 `search_engine` 设为 `string_matching` 时，`api list --search` 能够快速返回关键词匹配结果.
*   当安装了高级搜索插件并设为 `embedding_search` 时，`api list --search` 支持自然语言语义理解。
*   在模拟存储故障（如 SQLite 文件锁死或删除）时启动 `cowen` 系统及 `bridge` 进程，系统不发生 Panic 闪退崩溃，能优雅报错打印存储初始化异常。
*   在运行 `cowen daemon` 后台服务或 `renewer` 时，凭证定时维护日志间隔符合基于 Token 寿命自适应计算的步长，并附带不规则的秒级抖动差值。
*   分拆后的 `cowen-common` 仅保留稳定的数据模型和 SPI 契约定义，其 Cargo.toml 中不再依赖复杂的网络或业务级 Crates，且工程各模块可正常编译通过，无依赖循环风险。

