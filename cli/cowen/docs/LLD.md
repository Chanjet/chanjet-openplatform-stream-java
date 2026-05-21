# cli/cowen v0.3.3 详细设计 (LLD)

> **版本**: v0.3.3
> **阶段**: Implementation-Ready Blueprint
> **状态**: `DRAFT`

## 1. ProfileWorker 状态机实现细节

### 1.1 物理模型 (Replaces existing 4-state model)
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

pub struct ProfileWorker {
    pub profile: String,
    pub status: WorkerStatus,
    pub cancel_token: CancellationToken,
    pub join_handle: Option<tokio::task::JoinHandle<()>>,
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

### 2.1 寻址与解析算法 (resolve_segment)
```rust
pub fn resolve_segment(current: &mut serde_json::Value, segment: &str) -> CowenResult<&mut Value> {
    if segment == "+" {
        let arr = current.as_array_mut().ok_or(CowenError::Config("Not an array".into()))?;
        arr.push(serde_json::json!({}));
        Ok(arr.last_mut().unwrap())
    } else if segment.contains(':') {
        let parts: Vec<&str> = segment.splitn(2, ':').collect();
        let (key, val) = (parts[0], parts[1]);
        let arr = current.as_array_mut().ok_or(CowenError::Config("Not an array".into()))?;
        let idx = arr.iter().position(|item| {
            item.get(key).and_then(|v| v.as_str()) == Some(val)
        }).ok_or(CowenError::Config(format!("Locator {}:{} not found", key, val)))?;
        Ok(&mut arr[idx])
    } else if let Ok(idx) = segment.parse::<usize>() {
        let arr = current.as_array_mut().ok_or(CowenError::Config("Not an array".into()))?;
        arr.get_mut(idx).ok_or(CowenError::Config(format!("Index {} out of bounds", idx)))
    } else {
        current.as_object_mut()
            .ok_or(CowenError::Config("Not an object".into()))?
            .get_mut(segment)
            .ok_or(CowenError::Config(format!("Field {} not found", segment)))
    }
}
```

### 2.2 路径更新逻辑 (set_by_path)
1. 将路径按 `.` 拆分为片段序列。
2. 循环片段序列，调用 `resolve_segment` 递归定位。
3. 若片段不存在且非最后一段，根据下一片段类型（数字/+ vs 字符）自动补全 `Array` 或 `Object`。
4. 在最后一段执行赋值。

---

## 3. FileStore 归一化与平滑迁移

### 3.1 V2 to V3 迁移逻辑
**函数签名**: `pub async fn migrate_v2_to_v3(vault_dir: &Path, profile: &str) -> CowenResult<()>`
1. 检测 `vault/{profile}.json` 是否存在。
2. 若存在，读取并解析为 `RawVaultData`。
3. 遍历 `RawVaultData` 中的分类 (tokens, tickets, dlq)。
4. 对每个条目，调用 `FileStore::save_raw(prefix, id, data)` 写入新路径 `vault/{profile}/{prefix}/{id}.json`。
5. 完成后，重命名旧文件为 `{profile}.json.v2_bak`。

### 3.2 归一化 I/O 与 GC 识别算法
**GC 签名**: `pub async fn list_orphans(&self) -> CowenResult<Vec<OrphanItem>>`
1. 遍历 `vault/{profile}/` 下的所有子目录。
2. 对每个 prefix (如 `plugins`)，扫描其所有 `.json` 文件。
3. 提取文件名作为 ID。
4. 调用 `ConfigManager::is_id_referenced(prefix, id)` 检查配置引用。
5. 若不被引用，则加入返回列表。

---

## 4. TDD 验证契约 (Testing Strategy)

### 4.1 状态转移边验证 (9 Cases)
1. `Created -> Starting`: 验证调用 `start()` 后状态转移。
2. `Starting -> Running`: 模拟启动成功。
3. `Starting -> Backoff`: 模拟端口绑定失败，验证重试计数。
4. `Running -> Backoff`: 模拟运行中 Panic。
5. `Backoff -> Starting`: 验证等待时间到期后的自动重试。
6. `Backoff -> Failed`: 模拟连续 5 次失败，验证熔断。
7. `Failed -> Starting`: 验证通过 `ManualRestart` 唤醒。
8. `Running -> Draining`: 验证接收到 SIGTERM。
9. `Draining -> Stopped`: 验证任务清空后的退出。

### 4.2 配置自治验证
1. `Identifier Locator`: `plugins.name:p1.path` 准确定位。
2. `Strict Bounds`: 设置索引 99 报错。
3. `Append Mode`: `+` 符号成功向数组末尾添加新元素。
4. `Collapsing`: `unset` 中间元素后，后续元素索引前移。
