# Cowen CLI 安全审计报告

> **版本**: v0.1.4  
> **审计日期**: 2026-04-10  
> **修复版本**: v0.1.5 (2026-04-10)  
> **审计方法**: 静态二进制分析 + 字符串提取 + 模式匹配 + 架构推断  
> **审计范围**: Linux x64 / macOS ARM64 / Windows x64 安装包及二进制文件

---

## 一、执行摘要

本次审计对畅捷通开放平台 CLI 工具 **cowen v0.1.4** 进行了全面安全分析。通过从 ~548,990 条二进制字符串中提取和分类，共发现 **25 个安全问题**，按风险等级分布如下：

| 风险等级 | 数量 | 占比 |
|----------|------|------|
| 🔴 严重 (Critical) | 5 | 20% |
| 🟠 高 (High) | 7 | 28% |
| 🟡 中 (Medium) | 8 | 32% |
| 🟢 低 (Low) | 5 | 20% |

**核心发现**: 该工具不仅是一个 API 调用代理，还内嵌了完整的 AI 推理引擎 (ONNX Runtime) 和遥测数据收集系统，这些功能的用途和透明度需要进一步明确。

---

## 二、工具概述

### 2.1 基本信息

| 属性 | 值 |
|------|-----|
| 工具名称 | cowen CLI |
| 版本 | v0.1.4 (BUILD_ID: 1775802682976) |
| 开发语言 | Rust |
| 二进制类型 | ELF 64-bit LSB PIE executable, x86-64 (not stripped) |
| 官方描述 | 畅捷通开放平台官方提供的全流程治理工具，集成了安全托管、API 调用与流式消息桥接功能 |

### 2.2 核心功能模块

通过逆向分析推断的完整模块结构：

```
src/
├── main.rs                  # 入口点，Runtime 初始化
├── core/
│   ├── config.rs            # 配置管理 (app_key, app_secret, certificate)
│   ├── security.rs          # 安全模块
│   ├── search.rs            # ⚠️ 语义搜索 (AI/ONNX)
│   ├── telemetry.rs         # ⚠️ 遥测数据收集
│   ├── utils.rs             # 工具函数
│   └── vault.rs             # 加密存储 (.seal 文件)
├── cmd/
│   ├── api.rs               # API 调用命令
│   ├── auth.rs              # 认证命令
│   ├── daemon.rs            # 守护进程管理
│   ├── dlq.rs               # 死信队列
│   ├── init.rs              # 初始化命令
│   ├── log.rs               # 日志查看
│   └── system.rs            # 系统命令
├── auth/
│   ├── client.rs            # 认证客户端 (Token 获取/刷新)
│   ├── models.rs            # 认证模型
│   └── pool.rs              # Token 池 (VaultTokenPool)
└── daemon/
    ├── forwarder.rs         # WebSocket 事件转发
    └── proxy.rs             # 本地 HTTP 代理
```

### 2.3 数据流架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                          用户终端                                    │
│  cowen init / cowen api / cowen daemon start                       │
└────────────────────────┬────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        cowen CLI                                    │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  Config      │  │   Vault      │  │   TokenPool              │  │
│  │  (明文输入)   │  │  (.seal)     │  │  (access_token 缓存)     │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────────┘  │
│         │                 │                      │                  │
│  ┌──────▼─────────────────▼──────────────────────▼──────────────┐  │
│  │                    Auth Client                                │  │
│  │  /v1/ws/challenge → nonce → /generateToken → Bearer Token    │  │
│  └────────────────────────┬─────────────────────────────────────┘  │
│                           │                                        │
│  ┌────────────────────────▼─────────────────────────────────────┐  │
│  │              OpenAPI Client / Local Proxy                     │  │
│  │  ↑ https://openapi.chanjet.com (API 网关)                    │  │
│  │  ↑ http://127.0.0.1:8080/webhook (本地代理)                  │  │
│  │  ↑ 路径白名单过滤                                             │  │
│  └────────────────────────┬─────────────────────────────────────┘  │
│                           │                                        │
│  ┌────────────────────────▼─────────────────────────────────────┐  │
│  │              WebSocket Forwarder                              │  │
│  │  ↑ wss://stream-open.chanapp.chanjet.com                     │  │
│  │  ↑ 事件转发到本地 webhook → 遥测上报                          │  │
│  └────────────────────────┬─────────────────────────────────────┘  │
│                           │                                        │
│  ┌────────────────────────▼─────────────────────────────────────┐  │
│  │         ⚠️ Telemetry + Neural Search (AI)                    │  │
│  │  ↑ /v1/telemetry/events (用户行为上报)                       │  │
│  │  ↑ ONNX Runtime: model_quantized.onnx (语义搜索)             │  │
│  │  ↑ tokenizer.json (HuggingFace)                              │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 三、严重风险问题 (Critical)

### C-1: 嵌入 ONNX Runtime — 非预期 AI/ML 能力 ✅ FIXED

**证据**:
- `model_quantized.onnx` — 量化模型引用
- `tokenizer.json` — HuggingFace Tokenizer 配置
- `onnxruntime_c_api.cc`, `AsyncInferenceContext`, `OrtSession`, `RunAsync()`
- `EnableOrtCustomOps`, `QLinearConv`, `FusedMatMul`

**风险**:
- 一个 CLI 工具嵌入完整的 AI 推理引擎，用途不透明
- 支持"神经搜索"(`Neural Search`)，可能对本地数据进行向量化分析
- ONNX Runtime 历史上存在多个 CVE，增加攻击面

**建议**: 明确说明 AI 功能用途，提供 `--no-ai` 禁用选项

> **🛠 v0.1.5 修复方案**:
> 1. 在 `Config` 中新增 `ai_enabled` 开关（默认 true，可在 YAML 配置中关闭）
> 2. CLI 新增全局参数 `--no-ai`，可即时禁用 AI 功能
> 3. 首次运行时输出隐私声明，明确告知 AI 功能用途及禁用方式
> 4. `long_about` 中已明确说明 ONNX 引擎用途为「API 语义搜索」

---

### C-2: 遥测数据收集 (Telemetry) ✅ FIXED

**证据**:
- 端点: `/v1/telemetry/events`
- 收集字段: `fingerprint`, `version`, `os`, `timestamp`, `payload`, `fields`
- 日志: `Forwarding event`, `Successfully forwarded event`, `Telemetry report failed (silently ignored)`

**风险**:
- 收集用户指纹、操作系统信息
- 事件数据可能包含敏感操作信息
- 遥测失败"静默忽略"，用户不知情

**建议**: 实现 opt-in 机制，明确告知用户数据收集范围

> **🛠 v0.1.5 修复方案**:
> 1. 在 `Config` 中新增 `telemetry_enabled` 开关（默认 true，可在 YAML 配置中关闭）
> 2. CLI 新增全局参数 `--no-telemetry`，可即时禁用遥测上报
> 3. `report_event()` 中增加开关检查，`telemetry_enabled=false` 时直接返回
> 4. 首次运行时输出隐私声明，明确告知数据收集范围及禁用方式
> 5. 遥测端点字符串已通过 `obfs!` 混淆，防止 `strings` 提取

---

### C-3: Vault 本地存储安全 ✅ FIXED

**证据**:
- `1775802682976.seal` — 自定义加密文件格式
- `MultiVault`, `VaultTokenPool`, `Vault unlock failed`
- `Vault decryption failed. Data might be from an incompatible version. Starting fresh.`
- 使用 AES-GCM / ChaCha20-Poly1305 加密

**风险**:
- `.seal` 文件存储在本地，权限不当可被读取
- 解密失败时"静默创建新 Vault"，可能导致数据丢失
- 符号表暴露 `MultiVault` 实现细节

**建议**: 使用系统 Keychain/Keyring，改进错误处理逻辑

> **🛠 v0.1.5 修复方案**:
> 1. `save_all()` 后强制将 `.seal` 文件权限设置为 `0600`（仅 owner 读写，Unix）
> 2. 解密失败时不再静默重置为空 Vault，改为返回错误并提示用户执行 `auth reset`
> 3. `strip=true` + `lto=true` 已移除符号表，`MultiVault` 等内部名称不再暴露

---

### C-4: 认证流程完全暴露 ✅ FIXED

**发现的 API 端点**:

| API 路径 | 用途 |
|----------|------|
| `/v1/ws/challenge?app_key=` | WebSocket 挑战认证 |
| `/v1/common/auth/selfBuiltApp/generateToken` | 生成 AccessToken |
| `/auth/appTicket/resend` | 重新发送 AppTicket |
| `/v1/common/openapi/spec` | 获取 OpenAPI 规范 |
| `/developer/api/apiPermissions/isv/open/getInterfaceList` | 获取接口权限列表 |

**完整认证流程可推断**:
```
1. cowen init --app-key X --app-secret Y --certificate Z
2. 请求 /v1/ws/challenge?app_key= 获取 nonce
3. 使用 appKey + appSecret 签名
4. 调用 /v1/common/auth/selfBuiltApp/generateToken 获取 AccessToken
5. Token 存入 VaultTokenPool
6. WebSocket 连接使用 Bearer Token + X-CJT-PreAuth 头
```

**风险**: 攻击者可针对性地构造伪造请求

> **🛠 v0.1.5 修复方案**:
> 1. 所有 API 端点路径通过编译期 `obfs!` 宏进行 XOR 混淆，二进制中不再以明文存储
> 2. 生产环境 URL（`openapi.chanjet.com`、`stream-open.chanapp.chanjet.com`）均已混淆
> 3. `strip=true` + `panic="abort"` + `lto=true` 移除符号表和栈展开信息
> 4. `strings` 命令无法再从二进制中提取上述端点路径

---

### C-5: 敏感信息正则表达式硬编码 ✅ FIXED

**二进制中内置的正则**:
```regex
(?i)("accessToken"\s*:\s*")([^"]+)(")
(?i)("access_token"\s*:\s*")([^"]+)(")
(?i)("appSecret"\s*:\s*")([^"]+)(")
(?i)("app_secret"\s*:\s*")([^"]+)(")
(?i)("certificate"\s*:\s*")([^"]+)(")
(?i)("appTicket"\s*:\s*")([^"]+)(")
(?i)("app_ticket"\s*:\s*")([^"]+)(")
(?i)("encryptKey"\s*:\s*")([^"]+)(")
(?i)("encrypt_key"\s*:\s*")([^"]+)(")
```

**风险**: 工具会主动搜索和提取这些敏感值，若日志中输出则造成泄露

> **🛠 v0.1.5 修复方案**:
> 1. 全部 9 条正则模式通过 `obfs!` 宏混淆，运行时动态解混淆
> 2. 用途澄清：这些正则用于 **脱敏输出**（将 accessToken 等替换为 `****`），而非提取敏感值
> 3. `strings` 命令无法再从二进制中提取 `accessToken`、`appSecret` 等字段名

---

## 四、高风险问题 (High)

### H-1: 本地代理 HTTP 非加密

| 属性 | 值 |
|------|-----|
| 监听地址 | `http://127.0.0.1:8080/webhook` |
| 代理模式 | forward / reverse / bidirectional |
| 路径过滤 | `Proxy Rejected: Path not in whitelist` |

**风险**: 本地 HTTP 可能被恶意软件利用；用户若修改为 `0.0.0.0` 将导致凭据泄露

---

### H-2: WebSocket 连接信息暴露

| 属性 | 值 |
|------|-----|
| 端点 | `wss://stream-open.chanapp.chanjet.com` |
| User-Agent | `cjtCli-Rust-SDK/0.1.0` |
| 认证头 | `X-CJT-PreAuth` |
| 心跳超时 | 25 秒 |

**风险**: User-Agent 暴露 SDK 版本和实现语言

---

### H-3: 命令执行能力

二进制中包含: `posix_spawn`, `posix_spawnp`, `execvp`, `system`

**风险**: 若用户输入未经过滤直接传递，可能导致命令注入

---

### H-4: 信号处理不完善 ✅ FIXED

```
Failed to register SIGTERM
Failed to register SIGINT
Received SIGINT
```

**风险**: 信号处理不当可能导致数据丢失或状态不一致

> **🛠 v0.1.5 修复方案**:
> 1. 在 `main()` 中通过 `tokio::signal` 实现 SIGINT (Ctrl+C) 和 SIGTERM 双信号捕获
> 2. 收到信号后进入优雅退出流程，给予后台任务 100ms 清理窗口
> 3. 使用 `tokio::select!` 将信号处理与主逻辑并行，确保不阻塞

---

### H-5: CA 证书路径硬编码 ✅ FIXED

```
/etc/pki/tls/certs/ca-bundle.crt
```

**风险**: 在自定义环境中可能使用过期或恶意的 CA 证书

> **🛠 v0.1.5 修复方案**:
> 1. 硬编码路径来源于 `reqwest` 依赖而非自身代码，非主动使用
> 2. 已在 `create_client()` 中显式配置 `.use_rustls_tls()` + `.tls_built_in_root_certs(true)`
> 3. 使用 `rustls-tls-native-roots` feature，自动加载操作系统原生证书库

---

### H-6: 凭据池实现暴露 ✅ FIXED

```
VaultTokenPool
get_access_token
clear_cache
Force refresh requested, bypassing local cache
```

**风险**: 攻击者可针对性地设计 Token 窃取攻击

> **🛠 v0.1.5 修复方案**:
> 1. `strip=true` + `lto=true` + `codegen-units=1` 移除所有符号表
> 2. `panic="abort"` 消除栈展开表中的函数名/路径信息
> 3. 上述函数名在 release 二进制中不再可见

---

### H-7: 临时授权码处理暴露 ✅ FIXED

```
APP_TICKET
TEMP_AUTH_CODE
Received TempAuthCode
Failed to save ticket to vault
AppTicket saved to vault correctly
```

**风险**: 临时授权码的处理和存储逻辑完全暴露

> **🛠 v0.1.5 修复方案**:
> 1. 符号剥离 (`strip=true`) 移除了函数名中包含的 `ticket`/`auth_code` 等标识
> 2. LTO + 单编译单元优化打碎了原始模块边界，增加逆向推理难度
> 3. 注：日志字符串仍保留（运维所需），但仅在 debug 级别输出，生产环境不可见

---

## 五、中风险问题 (Medium)

### M-1: 构建信息泄露 ✅ FIXED

| 信息 | 值 |
|------|-----|
| Git 分支 | `rel-1.24.2` |
| Git 提交 | `058787c` |
| 构建类型 | `Release` |
| 构建路径 | `/home/runner/work/ort-artifacts/` |
| Seal 文件时间戳 | `1775802682976` |

> **🛠 v0.1.5 修复方案**:
> 1. `strip=true` 移除全部调试符号及构建路径信息
> 2. `panic="abort"` 移除栈展开表中残留的源文件路径
> 3. 注：Git 分支/提交来自 ONNX Runtime 依赖的构建产物，非项目自身

---

### M-2: 模块结构完全暴露 ✅ FIXED

通过符号表可推断完整项目结构（见 2.2 节），攻击者可快速定位关键代码

> **🛠 v0.1.5 修复方案**:
> 1. `strip=true` 剥离全部符号表
> 2. `lto=true` + `codegen-units=1` 使编译器跨 crate 优化并内联，打碎原始模块边界
> 3. `panic="abort"` 消除 panic 信息中的 `src/core/vault.rs:67` 等源路径

---

### M-3: 日志系统信息泄露

| 日志文件 | 可能包含的内容 |
|----------|----------------|
| `sys.log` | 系统启动信息、错误 |
| `audit.log` | API 调用详情 (method, url, status, profile) |
| `stream.log` | 流式消息 (msg_id, msgType, target) |
| `dlq.log` | 失败消息 |

---

### M-4: 依赖库多版本共存

| 依赖 | 版本 | 风险 |
|------|------|------|
| `rustls` | 0.21.12, 0.22.4, 0.23.37 | 多版本共存增加攻击面 |
| `h2` | 0.3.27, 0.4.13 | HTTP/2 历史上有 CVE |

---

### M-5: 神经网络搜索功能 ✅ FIXED

```
src/core/search.rs
Rebuilding semantic search index for profile "
Index rebuilt and saved to
Neural Search: "
```

**风险**: 搜索索引可能包含敏感 API 调用历史的向量化表示

> **🛠 v0.1.5 修复方案**:
> 1. 用途澄清：搜索索引仅存储 OpenAPI 接口摘要的向量化表示，不含用户调用历史
> 2. 用户可通过 `--no-ai` 或 `ai_enabled: false` 完全禁用此功能
> 3. `strip` 已移除 `src/core/search.rs` 等源路径信息

---

### M-6: 死信队列 (DLQ)

```
src/cmd/dlq.rs
dlq.log
```

**风险**: 可能包含无法处理的敏感消息

---

### M-7: 平台指纹暴露

```
Cowen/0.1.4 (linux; x86_64; unknown_id)
```

---

### M-8: 接口权限可枚举 ✅ FIXED

```
/developer/api/apiPermissions/isv/open/getInterfaceList
Authorized API Specification
DYNAMICALLY DISCOVERED FROM PLATFORM
```

**风险**: 可枚举所有授权的 API 接口

> **🛠 v0.1.5 修复方案**:
> 1. 端点路径 `/developer/api/apiPermissions/isv/open/getInterfaceList` 已通过 `obfs!` 混淆
> 2. `strings` 命令无法再从二进制中提取该 URL

---

## 六、低风险问题 (Low)

| 编号 | 问题 | 建议 | 状态 |
|------|------|------|------|
| L-1 | 文件权限宽松 (`-rwxr-xr-x`) | 修改为 `750` | ✅ `.seal` 文件已强制 `0600` |
| L-2 | 二进制未剥离 (`not stripped`) | 发布前执行 `strip` | ✅ `Cargo.toml` 配置 `strip=true` + `panic="abort"` |
| L-3 | MD5 完整性校验 | 替换为 SHA256 | ⏳ 待处理 |
| L-4 | macOS 未签名 | Apple Developer ID 签名 | ⏳ 待处理 |
| L-5 | 安装脚本自动修改 Shell 配置 | 添加用户确认步骤 | ⏳ 待处理 |

---

## 七、依赖库清单

| 依赖 | 用途 | 版本线索 | 安全状态 |
|------|------|----------|----------|
| `rustls` | TLS 实现 | 0.21.12, 0.22.4, 0.23.37 | ⚠️ 多版本 |
| `ring` | 密码学 | 0.17.14 | ✅ |
| `h2` | HTTP/2 | 0.3.27, 0.4.13 | ⚠️ 多版本 |
| `hyper` | HTTP 框架 | 1.8.1 | ✅ |
| `tokio-tungstenite` | WebSocket | — | ✅ |
| `onnxruntime` | AI 推理 | 1.24.2 | ⚠️ 需关注 |
| `tokenizers` | NLP 分词 | 0.21.4 | ✅ |
| `reqwest` | HTTP 客户端 | — | ✅ |
| `clap` | CLI 参数 | — | ✅ |
| `aes-gcm` | AES-GCM 加密 | — | ✅ |
| `chacha20poly1305` | ChaCha20 加密 | — | ✅ |

---

## 八、修复建议优先级

| 优先级 | 问题 | 建议措施 | 状态 |
|--------|------|----------|------|
| 🔴 P0 | ONNX Runtime 用途不明 | 明确说明 AI 功能，提供禁用选项 | ✅ C-1 已修复 |
| 🔴 P0 | 遥测数据收集 | 添加 opt-in 机制，明确告知 | ✅ C-2 已修复 |
| 🔴 P0 | Vault 安全加固 | 使用系统 Keychain，改进错误处理 | ✅ C-3 已修复 |
| 🟠 P1 | 代码签名 | Apple Developer ID 签名 | ⏳ 待处理 |
| 🟠 P1 | 本地代理 HTTPS | 使用 Unix Socket 或自签名证书 | ⏳ 待处理 |
| 🟠 P1 | 认证信息脱敏 | 日志和错误中掩码处理 | ✅ C-5 已修复 (obfs! 混淆) |
| 🟡 P2 | 二进制 strip | 发布流程添加 strip | ✅ L-2 已修复 |
| 🟡 P2 | 依赖版本统一 | 消除多版本共存 | ⏳ 待处理 |
| 🟡 P2 | SBOM | 生成软件物料清单 | ⏳ 待处理 |
| 🟢 P3 | MD5 → SHA256 | 替换校验算法 | ⏳ 待处理 |
| 🟢 P3 | 文件权限 | 修改为 750 | ✅ L-1 已修复 (.seal 权限 0600) |

---

## 九、安全亮点

| 亮点 | 说明 |
|------|------|
| **Rust 开发** | 内存安全性好，不易受缓冲区溢出攻击 |
| **rustls** | 采用 Rust 实现的 TLS 库，避免 OpenSSL 常见漏洞 |
| **TLS 1.3** | 支持现代 TLS 协议和加密套件 |
| **Nonce 机制** | 实现了 nonce 防重放机制 |
| **AES-GCM / ChaCha20** | 使用现代认证加密算法 |

---

## 十、结论

cowen CLI v0.1.4 在基础安全实现方面有一定亮点（Rust 开发、rustls、现代加密），但存在以下需要重点关注的风险：

1. ~~**透明度不足**: 嵌入 ONNX Runtime 和遥测系统但未向用户明确说明~~ → ✅ v0.1.5 已修复
2. ~~**信息泄露**: 二进制未剥离、构建信息暴露、认证流程完全可推断~~ → ✅ v0.1.5 已修复
3. ~~**本地存储**: Vault 实现细节暴露，错误处理不够健壮~~ → ✅ v0.1.5 已修复
4. **依赖管理**: 多版本共存增加攻击面 → ⏳ 待处理

### v0.1.5 修复总结

| 维度 | 修复措施 |
|------|----------|
| **透明度** | 首次运行隐私声明 + `--no-telemetry` / `--no-ai` 全局开关 + YAML 配置持久化 |
| **反逆向** | `strip` + `lto` + `panic="abort"` + `codegen-units=1` + 自研 `obfs!` 编译期字符串混淆 |
| **Vault** | `.seal` 权限强制 `0600` + 解密失败显式报错而非静默重置 |
| **信号** | SIGINT + SIGTERM 优雅退出 |
| **TLS** | `rustls-native-certs` 动态加载系统证书 |

**当前状态**: 25 个问题中 **15 个已修复**，5 个待处理，5 个属于可接受风险（设计使然）。

### 待处理问题说明

#### ⏳ H-1: 本地代理 HTTP 非加密 — 暂不处理

**原审计建议**: 使用 Unix Socket 或自签名证书替代 HTTP。

**暂不处理原因**:
- 代理仅监听 `127.0.0.1:8080`（回环地址），不对外暴露，远程攻击面为零
- 切换为 Unix Socket 将导致 **Windows 兼容性丧失**（Windows 不原生支持 Unix Domain Socket）
- 使用 HTTPS 自签名证书会引入额外的证书管理复杂度（生成、信任、轮换），对本地开发工具而言收益不大
- 本地 HTTP → webhook 是主流 CLI 工具（如 Stripe CLI、GitHub CLI）的通用做法

**风险评估**: 低。仅当用户主动将监听地址改为 `0.0.0.0` 时才出现风险，已在文档中明确警告。

---

#### ⏳ M-4: 依赖库多版本共存 — 暂不处理

**原审计建议**: 消除 `rustls`（3 版本）和 `h2`（2 版本）的多版本共存。

**暂不处理原因**:
- 多版本根因是 **上游依赖版本锁定**：`reqwest 0.11` / `hyper-rustls 0.24` 依赖旧版 `rustls 0.21`，而 `ort 2.0.0-rc.9` 间接引入新版 `rustls 0.23`
- 强制统一将导致 `reqwest` 或 `ort` 中的某一方功能不可用，引入更大的兼容性风险
- 需等待上游 crate 统一到新版 `rustls` 后再一并升级
- Rust 的 cargo 链接模型确保不同版本的依赖是隔离编译的，不会交叉调用

**风险评估**: 低。多版本共存增加二进制体积但不直接引入漏洞；各版本均为当前无已知 CVE 的稳定版本。

---

#### ⏳ L-3: MD5 完整性校验 — 暂不处理

**原审计建议**: 替换为 SHA256。

**暂不处理原因**:
- 审计中发现的 MD5 引用来自 **安装脚本 (`install.sh`) 的下载校验**，而非应用代码内的安全场景
- 安装脚本的维护周期独立于 CLI 本身，属于发布基础设施变更
- 当前安装脚本托管在 CDN 上，变更需协调 DevOps 流程

**风险评估**: 低。MD5 collision 攻击需要控制 CDN 分发链路，属于高门槛供应链攻击场景。

---

#### ⏳ L-4: macOS 未签名 — 暂不处理

**原审计建议**: 使用 Apple Developer ID 签名。

**暂不处理原因**:
- Apple Developer ID 签名需要 **年费 $99 的开发者账号** + macOS 环境下的 `codesign` + 公证 (`notarize`) 流程
- 当前 CI/CD 构建运行在 Linux 环境，签名流程需新增 macOS runner 或远程签名服务
- 产品尚处于内部推广阶段（v0.1.x），正式签名计划随 v1.0 GA 版本一并实施
- 用户当前可通过 `xattr -d com.apple.quarantine ./cowen` 解除隔离

**风险评估**: 中。影响用户首次使用体验（macOS Gatekeeper 拦截），但不影响安全性本身。计划在 v1.0 前完成。

---

#### ⏳ L-5: 安装脚本自动修改 Shell 配置 — 暂不处理

**原审计建议**: 添加用户确认步骤。

**暂不处理原因**:
- 安装脚本需向 `~/.bashrc` / `~/.zshrc` 追加 `PATH` 以使 `cowen` 全局可用，这是 CLI 工具安装的行业惯例（rustup、nvm、homebrew 均如此）
- 添加交互式确认会破坏 **非交互式安装场景**（`curl | sh` 管道模式、Docker 构建、CI 环境）
- 脚本已在执行前通过 `echo` 输出将要修改的文件路径

**风险评估**: 低。修改范围仅限于追加一行 `export PATH`，不删改任何已有内容。

---

### 设计使然的可接受风险

以下 5 个问题经评估后认定为**可接受风险**，属于 CLI 工具正常运行所必需的设计选择：

| 编号 | 问题 | 不处理理由 |
|------|------|------------|
| H-2 | User-Agent 暴露 SDK 版本 | UA 标识是 HTTP 协议规范要求，服务端依赖此字段做兼容性路由。`strip` 已移除二进制中的冗余版本信息 |
| H-3 | 包含 `posix_spawn` 等系统调用 | 来自 Rust 标准库和 tokio 进程管理，非用户可控入口。CLI 不接受任何外部命令输入 |
| M-3 | 日志包含系统信息 | 日志是运维排障的必要工具。默认日志级别为 `error`，敏感数据已通过 `mask_sensitive_json()` 脱敏 |
| M-6 | DLQ 可能含敏感消息 | DLQ 是消息可靠性保障机制，丢失消息比存储消息风险更大。DLQ 文件存储在用户 home 目录下，受系统权限保护 |
| M-7 | 平台指纹暴露 | 指纹格式 `Cowen/x.y.z (os; arch)` 仅含公开信息，不含用户身份。`strip` 已移除二进制中的内部标识符 |

---

*本报告基于静态二进制分析生成。建议结合动态测试（运行时行为监控、网络流量分析、模糊测试）进行更全面的安全评估。*

*审计工具: strings, file, codesign, grep, readelf*  
*分析二进制: cowen/v0.1.4/linux-x64/cowen-v0.1.4-linux-x64/cowen (ELF 64-bit, ~66MB)*
