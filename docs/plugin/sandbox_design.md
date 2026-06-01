# 畅捷通 Stream Gateway 动态协商安全沙箱方案设计规范 (Technical Specification)

本规范旨在阐述如何融合 **MCP 动态 Workspace `roots` 协商协议** 与 **操作系统的底层内核级沙箱机制（macOS Seatbelt, Linux Landlock, Windows Low Integrity/ACL）**，建立一套“默认安全隔离 + CLI 动态授权越界”的零信任（Zero-Trust）双防线插件沙箱方案。

---

## 1. 核心架构拓扑 (Architecture Overview)

方案通过两层防御线来拦截与控制插件的文件读写特权：
*   **第一道防线 (应用层柔性对齐)**：双端通过标准 JSON-RPC 初始化消息协商 `workspace://` 与 `cache://` 虚拟 URI 到物理绝对路径的插值映射。
*   **第二道防线 (系统级刚性红线)**：宿主通过跨平台沙箱命令加载器（macOS `sandbox-exec`、Linux `Landlock`、Windows `Low-IL/ACL`）将翻译出来的真实物理工作区绝对路径锁死在操作系统的进程隔离区。

```
                              [CLI 运行时授权: workspace_dir]
                                            |
                                            v
+------------------+     1. Verify Signature     +--------------------------------+
|  Plugin Manifest | --------------------------> |       宿主网关 (Host Gateway)   |
| (声明虚拟 intent) |                            | - 动态将虚拟 URI 翻译为绝对路径 |
+------------------+                             +--------------------------------+
                                                                |
                                             +------------------+------------------+
                                             | (A. 内核沙箱隔离)                    | (B. 协议层 roots 下发)
                                             v                                     v
                           +-----------------------------------+         +-------------------+
                           | 操作系统内核沙箱处理器            |         | JSON-RPC (MCP)    |
                           | - macOS: sandbox-exec -p          |         | initialize 帧     |
                           | - Linux: Landlock rules           |         +-------------------+
                           | - Windows: Low-IL Token & ACL     |                   |
                           +-----------------------------------+                   |
                                             \                                    /
                                              \                                  /
                                               v                                v
                                   +-----------------------------------------------+
                                   |         Sidecar 插件子进程 (Sandboxed)         |
                                   | - 系统调用越界: 内核直接 Blocked/Kill 进程     |
                                   | - 正常调用: 依据 roots 映射优雅读写           |
                                   +-----------------------------------------------+
```

---

## 2. 静态证书声明与默认安全区 (Phase 1: Signing & Default Home Scope)

### 2.1 开发者签名意图声明 (Intent Declaration)
开发者在打包并对插件二进制进行数字签名时，仅允许在 `PluginManifest` 声明虚拟的访问意图（Intent）以及最小的默认本地存储配额。禁止在此阶段写入任何具体开发机的绝对物理路径。

```json
{
  "name": "cowen-search-embedding",
  "version": "0.4.0",
  "required_privileges": ["LocalFileRead", "LocalFileWrite"],
  "capabilities": {
    "requested_roots": [
      { "scheme": "cache://", "permission": "read-write", "desc": "插件私有高频缓存区" },
      { "scheme": "workspace://", "permission": "read-only", "desc": "开发机工程代码工作区" }
    ]
  }
}
```

### 2.2 宿主默认物理信任边界 (`COWEN_HOME` 派生)
为了保证插件即装即用（Out of the box），无需用户频繁参与命令行确认，宿主在安装插件时，自动在 `COWEN_HOME` 的插件归宿文件夹下生成**默认安全写目录**。
*   默认只读存储：`$COWEN_HOME/plugins/<plugin_name>/dist`
*   默认读写存储：`$COWEN_HOME/plugins/<plugin_name>/cache`

---

## 3. CLI 侧运行时物理路径插值映射 (Phase 2: CLI Runtime Mapping)

在部署及启动阶段，宿主（CLI 侧网关守护进程）负责动态地将虚拟的 URI `schemes` 解析为本地主机的绝对物理路径：

### 3.1 路径插值矩阵

| 虚拟 URI 意图 | 在 zhangliang 的 macOS 上翻译为 | 在 Bob 的 Windows 上翻译为 |
| :--- | :--- | :--- |
| **`workspace://`** | `/Users/zhangliang/chanjet/workspace` | `D:\Projects\my-java-connector` |
| **`cache://`** | `/Users/zhangliang/.gemini/antigravity-ide/search/cache` | `C:\Users\Bob\AppData\Local\cowen\cache` |
| **`home://`** | `/Users/zhangliang` | `C:\Users\Bob` |

### 3.2 CLI 授权来源
宿主获取运行时物理路径有三种途径：
1.  **自动上下文捕获**：CLI 自动获取执行当前二进制文件时的当前目录（Cwd），将其视作活动 `workspace://`。
2.  **配置文件授权 (`cowen.yaml`)**：用户在本地网关配置文件中通过 `workspaces` 配置静态白名单列表。
3.  **动态交互询问（交互授权）**：对于敏感或超限的外部目录，宿主在 CLI 控制台通过 `y/N` 交互询问，用户确认后方可把该物理路径加塞至沙箱 Profile 白名单。

---

## 4. 跨平台操作系统级沙箱强拦截设计 (Phase 3: Cross-Platform Execution)

宿主网关提供统一的系统级抽象层：
```rust
pub fn create_sandboxed_command(
    binary_path: &Path,
    default_cache_path: &Path,
    allowed_roots: &[PathBuf]
) -> std::process::Command;
```

### 4.1 macOS 平台 (Seatbelt 内核隔离)
通过动态生成 Seatbelt 沙箱声明 Profile，包裹 `sandbox-exec` 启动子进程。不允许在此白名单之外的任何目录发生写入：

```scheme
(version 1)
(allow default)
(deny file-write*)

;; 允许系统级临时写入目录
(allow file-write* (subpath "/private/var"))
(allow file-write* (subpath "/var/folders"))
(allow file-write* (subpath "/tmp"))

;; A. 允许对 COWEN_HOME 的缓存目录进行完全读写 (方案三静态特权)
(allow file-write* (subpath "{default_cache_path}"))

;; B. 动态编译并注入 CLI 侧授权的外部物理工程 Workspace 绝对路径
(allow file-write* (subpath "{allowed_roots[0]}"))
(allow file-write* (subpath "{allowed_roots[1]}"))
```

### 4.2 Linux 平台 (Landlock 进程路径裁减)
对于现代 Linux 内核（>= 5.13），宿主使用 Rust `landlock` crate 动态设置系统调用（syscall）级别的文件目录规则。
*   默认拦截所有的文件写操作。
*   仅对 `default_cache_path` 注入 `LANDLOCK_ACCESS_FS_WRITE_FILE` 读写特权规则。
*   仅对 `allowed_roots` 注入由 CLI 动态授信的只读/读写规则。

### 4.3 Windows 平台 (Low-IL 降权与 ACL 动态授权)
在 Windows 下采用 **“安全降权 + ACL 动态白名单”** 的生产隔离组合：
1.  **降权启动**：宿主使用 `CreateProcessAsUser` 将子进程的安全完整性令牌设置为 `Low Integrity Level`。
2.  **默认阻断**：低完整性进程天然在内核中被剥夺了向 `C:\Users\<User>\` 的 Desktop、Documents 等普通 Medium 级路径的改写可能。
3.  **动态 ACL 赋权**：宿主在通过 `CreateProcess` 启动前，动态调用 Windows API，修改 `allowed_roots` 物理文件夹的安全属性，为该低完整性安全标识符（SID）赋予读/写访问权限。

---

## 5. JSON-RPC (MCP) 应用层握手协商 (Phase 4: Protocol Handshake)

在进程成功拉起后，首先由宿主网关发起标准的 **MCP `initialize`** 初始化事件，进行应用层的柔性协商：

### 5.1 宿主初始化发送帧 (`initialize`)
宿主网关将已经注入沙箱并安全翻译好的物理路径列表，通过标准握手帧下发给子进程：

```json
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "id": 1,
  "params": {
    "capabilities": {},
    "roots": [
      {
        "uri": "workspace://",
        "physical_path": "/Users/zhangliang/chanjet/workspace"
      },
      {
        "uri": "cache://",
        "physical_path": "/Users/zhangliang/.gemini/antigravity-ide/search/cache"
      }
    ]
  }
}
```

### 5.2 插件接收与内部路径映射
插件子进程接收到 `initialize` 帧后，提取 `roots` 字典建立内部映射管理器 `VirtualFileSystem`：
```rust
pub struct VirtualFileSystem {
    mappings: HashMap<String, PathBuf>,
}

impl VirtualFileSystem {
    /// 插件在内部处理请求时，若涉及本地读取，先通过映射定位物理绝对路径
    pub fn resolve(&self, virtual_path: &str) -> Result<PathBuf, String> {
        for (scheme, physical) in &self.mappings {
            if virtual_path.starts_with(scheme) {
                let relative = &virtual_path[scheme.len()..];
                return Ok(physical.join(relative));
            }
        }
        Err("Permission Denied: Unmapped Virtual Path".to_string())
    }
}
```

---

## 6. 异常与恢复保护矩阵 (Security & Recovery Matrix)

| 场景 | 操作系统内核态行为 | 网关应用层表现 | 恢复与防灾行为 |
| :--- | :--- | :--- | :--- |
| **插件在开发期尝试非法硬编码物理路径** | 内核直接拦截系统调用（Syscall），返回 `Permission Denied` | 插件进程遭遇系统文件操作异常，调用失败 | 插件只应面向虚拟 `scheme` 进行寻址 |
| **插件中途被注入恶意木马劫持扫描 SSH 密钥** | 沙箱拦截子进程读取 `home://.ssh` 的一切系统调用 | 子进程触发段错误或被迫中止运行 | 宿主网关检测到子进程 EOF，自动清理缓存并安全销毁子进程 |
| **CLI 授权的工作区文件夹不存在** | 沙箱将此物理白名单规则作废，内核默认全阻断 | 网关初始化检测时拦截，拒绝下发此 `roots` | 网关在日志中提示配置目录不存在，降权为零特权运行 |
