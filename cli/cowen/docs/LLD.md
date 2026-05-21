# cli/cowen v0.3.3 详细设计 (LLD)

> **版本**: v0.3.3
> **阶段**: Implementation-Ready Blueprint
> **状态**: `DRAFT`

## 1. ProfileWorker 状态机实现细节

### 1.1 物理模型与状态展示
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerStatus {
    Created,
    Starting,
    Running,
    Backoff { 
        retry_count: u32, 
        next_retry_at: std::time::Instant,
        last_error: String 
    },
    Failed { reason: String },
    Draining,
    Stopped,
}
```

### 1.2 确定性逻辑算子 (Transition Matrix)
| 当前状态 | 触发动作 | 目标状态 | 副作用 (Side Effects) |
| :--- | :--- | :--- | :--- |
| `Created` | `Start` | `Starting` | 检查配置与资源 |
| `Starting` | `Success` | `Running` | 启动主循环 |
| `Starting` | `Error` | `Backoff` | 计算下一次重启时间 |
| `Running` | `Panic` | `Backoff` | 记录错误, retry_count++ |
| `Backoff` | `Retry` | `Starting` | 重新触发 Start |
| `Backoff` | `MaxFailed` | `Failed` | 进入熔断状态 |
| `Failed` | `ManualRestart` | `Starting` | **显式手动唤醒** |
| `Running` | `Stop` | `Draining` | 触发 CancelToken |
| `Draining` | `Finished` | `Stopped` | 释放资源 |

---

## 2. 增强型 Path Parser 与配置自治算法
... (内容省略) ...

---

## 3. FileStore 迁移与归一化

### 3.1 V2 to V3 迁移器 (Migration Logic)
... (内容省略) ...

### 3.2 归一化 I/O 与 GC 识别算法
*   **原子写入**: 写入临时文件 -> `fs::sync_all` -> `fs::rename`。
*   **list_orphans() 算法**: 
    1.  遍历 `vault/{profile}/` 下的所有子目录。
    2.  对每个 prefix (如 `plugins`)，扫描其所有 `.json` 文件。
    3.  提取 ID，并检查其是否在当前 Config 对象（由 `ConfigManager` 提供）的对应列表中。
    4.  若不在，则标记为 `Orphan`。
*   **目录自愈**: 访问不存在的 `prefix` 目录时自动创建。

---

## 4. TDD 验证契约 (Testing Strategy)
*   **Case: Deletion Collapsing**:
    *   GIVEN: `["a", "b", "c"]`
    *   WHEN: `unset 0`
    *   THEN: Array becomes `["b", "c"]` (Index 0 is now "b").
*   **Case: Backoff Visibility**:
    *   WHEN: Worker Panics
    *   THEN: `cowen status` Output contains "Retrying in X seconds".
