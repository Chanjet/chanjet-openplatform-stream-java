# Specification Delta: Unified Storage & Vault Architecture

## MODIFIED [Store Trait](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/src/core/store/mod.rs)

### Requirement: Profile Management Support
The `Store` trait must support atomic operations for profile cleanup and renaming.

#### Scenario: 清空 Profile 数据 (Clear Profile)
*   **Given**: 一个存储后端包含 Profile `p1` 的多条数据
*   **When**: 调用 `store.clear_profile("p1")`
*   **Then**: 该 Profile 下的所有键值对被永久删除

#### Scenario: 重命名 Profile (Rename Profile)
*   **Given**: 一个存储后端包含 Profile `p1` 的数据
*   **When**: 调用 `store.rename_profile("p1", "p2")`
*   **Then**: 所有原本属于 `p1` 的数据现在归属于 `p2`

---

## NEW [FileStore](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/src/core/store/file.rs)

### Requirement: 本地文件存储后端 (Local File Store)
实现一个基于加密文件的存储后端，逻辑等同于原 `MultiVault`。

#### Scenario: 数据持久化与加密
*   **Given**: 配置了 `local` 存储后端
*   **When**: 写入数据并重启应用
*   **Then**: 数据能够被正确解密并读取，且物理文件是经过 AES 加密的

---

## MODIFIED [Vault Implementation](file:///Users/zhangliang/chanjet/dev/workspace/open-streaming-connector/cli/cowen/src/core/vault.rs)

### Requirement: 统一 Vault 逻辑 (Unified Vault Logic)
`Vault` 接口应当只有一个统一的实现类，负责协调底层的 `Store`。

#### Scenario: 跨后端一致性
*   **Given**: 无论底层是 `FileStore` 还是 `SqlStore`
*   **When**: 调用 `vault.rename_profile(...)`
*   **Then**: 操作必须成功执行且行为一致
