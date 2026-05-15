# cli/cowen v0.3.1 概要设计 (HLD)

## 1. 架构目标
v0.3.1 在 v0.3.0 的基础上，通过增强核心引擎的可观测性和灵活性，进一步提升其在复杂生产环境下的表现。

## 2. 变更视图 (System Changes)

### 2.1 模块关系图
```mermaid
graph TD
    subgraph "Application Layer"
        Main[main.rs] --> Doctor[System Doctor]
    end

    subgraph "Service Layer"
        Daemon --> ConfigWatcher[Config Watcher]
        Daemon --> MetricsServer[Metrics & Health Server]
    end

    subgraph "Core & Infrastructure"
        ConfigWatcher --> ConfigMgr[ConfigManager]
        MetricsServer --> MetricsRegistry[Metrics Registry]
        Doctor --> Providers[All SPI Providers]
        Main --> SearchSPI[Search Provider SPI]
        SearchSPI -- "Internal" --> StringMatch[String Match Engine]
        SearchSPI -- "Dynamic" --> EmbeddingSearch[libcowen_search_embedding]
        end
        ```

        ## 3. 核心功能设计 (Feature Design)

        ### 3.1 配置热重载 (Config Hot-Reload)
        *   **架构变更**: 
        *   `ConfigManager` 增加订阅机制。
        *   Daemon 进程引入 `notify` 库监控配置文件物理变动。
        *   主循环处理 `SIGHUP` 信号。
        *   **并发策略**: 使用 `ArcSwap` 或 `tokio::sync::watch` 实现配置实体的原子化替换，确保读取配置的 Task（如 Proxy）不会因重载导致不一致。

        ### 3.2 监控与健康 API (Metrics & Health API)
        *   **架构变更**: 
        *   在 `cowen-server` 中启动一个独立的后台 Task，专门运行 Axum 管理服务。
        *   该服务仅监听 `127.0.0.1`。
        *   **指标采集**: 
        *   **Health**: 聚合检查数据库连接、配置文件读取权限。
        *   **Metrics**: 统计 `Proxy` 请求计数、`Forwarder` 流量、`DLQ` 长度。

        ### 3.3 环境自检工具 (System Doctor)
        *   **架构变更**: 
        *   定义 `Diagnostic` SPI 接口。
        *   各模块（Store, Auth, Net）注册各自的诊断逻辑。
        *   **交互逻辑**: 
        *   执行 `cowen system doctor` 时，串行或并行执行所有注册的检查项。
        *   输出包含 [OK], [WARN], [ERROR] 的详细列表及修复方案。

        ### 3.4 API 搜索插件化 (Pluggable Search Engine)
        *   **架构变更**: 
        *   核心代码移除对 `cowen-ai` 的源码依赖。
        *   引入 `SearchProvider` SPI 机制。
        *   **加载逻辑**: 
        1. 根据 `search_engine` 配置项决定加载哪种 Provider。
        2. 若为 `string_matching`，使用内置的高性能正则/字符串匹配引擎。
        3. 若为 `embedding_search`，尝试通过 `libloading` 加载外部动态库。
        4. 统一通过 `SearchProvider` Trait 暴露给 `api list` 命令。

## 4. 非功能性设计 (NFRs)
*   **性能**: 监控采集采用非阻塞方式，对业务流量（Proxy）的影响应小于 1%。
*   **隔离性**: 配置文件重载失败不应导致正在运行的 Daemon 崩溃，需回滚至旧配置。
*   **安全性**: 管理接口禁止任何跨站或外部访问。
