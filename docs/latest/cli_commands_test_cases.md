# owenc CLI 命令行全景测试用例 (Command-Line Test Cases)

> **文档目的**：规范 `owenc` (及 `owenc-test`) 命令行工具的功能验证标准，涵盖核心业务、安全凭据及边界场景。
> **测试环境**：建议在 `make build-test` 生成的测试环境下执行。

---

## 1. 初始化与凭据管理 (Initialization & Auth)

### 1.1 `init` 引导初始化
| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| CLI-INIT-01 | 首次标准初始化 | `owenc init --app-key <K> --app-secret <S> -c <CERT>` | 提示初始化成功，Vault 存储凭据，后台 Daemon 自动启动。 |
| CLI-INIT-02 | 缺失必填参数 | `owenc init --app-key <K>` | 报错提示缺失 `app-secret` 或 `certificate`。 |
| CLI-INIT-03 | 多 Profile 隔离 | `owenc init -p prod ...` | 在 `~/.owenc/prod.yaml` 产生配置，不影响 `default` 环境。 |
| CLI-INIT-04 | 证书格式错误 | `owenc init ... -c "invalid_base64"` | 校验失败，提示证书格式非法，不执行保存。 |

### 1.2 `auth` 状态与操作
| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| CLI-ATH-01 | 查看认证状态 | `owenc auth status` | 动态显示 AppKey 掩码、Vault 状态、Token 是否有效。 |
| CLI-ATH-02 | 强制手动登录 | `owenc auth login --force` | 忽略本地缓存，强制向云端发起一次新的 AccessToken 换取。 |
| CLI-ATH-03 | 获取明文 Token | `owenc auth token` | 直接在终端输出当前的 `accessToken` 字符串。 |
| CLI-ATH-04 | JSON 格式输出 | `owenc auth status --format json` | 输出标准的结构化 JSON，无任何额外干扰字符。 |

---

## 2. API 治理与探索 (API Management)

### 2.1 `api list` 接口发现
| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| CLI-API-01 | 全量列表展示 | `owenc api list` | 分页显示所有授权接口，包含 Method、Path 和中文描述。 |
| CLI-API-02 | 语义化智能搜索 | `owenc api list -s "查询用户信息"` | 基于 AI 模型返回相关度最高的 5 个接口（即使无关键词匹配）。 |
| CLI-API-03 | 离线搜索校验 | 断网状态下执行语义搜索 | AI 推理应本地完成（Zero-dependency ONNX），不应报错。 |
| CLI-API-04 | 分页边界测试 | `owenc api list --page 999` | 如果超出范围，应提示“暂无更多数据”而非崩溃。 |

### 2.2 `api spec` 规约文档
| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| CLI-SPC-01 | 标准文档查看 | `owenc api spec GET /v1/user` | 完美渲染请求参数（Header/Query）、Body 结构及响应示例。 |
| CLI-SPC-02 | Content-Type 隐藏 | 查看任意 POST 接口的 Spec | 参数列表中不应出现 `Content-Type: header`，该项已通过逻辑过滤。 |
| CLI-SPC-03 | 原始规范回吐 | `owenc api spec ... --raw` | 输出原始的 OpenAPI JSON 片段，便于复制给其他工具。 |
| CLI-SPC-04 | 路径自动补全匹配 | `owenc api spec GET v1/user` | 即使 Path 前缀缺失 `/`，也能通过模糊匹配找到规约。 |

### 2.3 `api call` 动态调用
| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| CLI-CAL-01 | 标准带参调用 | `owenc api GET "/v1/user?id=1"` | 自动注入鉴权头，返回云端真实 JSON 响应。 |
| CLI-CAL-02 | 带 Body 的 POST | `owenc api POST /v1/create -d '{"name":"test"}'` | 自动添加 `application/json` 并完成签名发送。 |
| CLI-CAL-03 | 缺失必填参数校验 | `owenc api POST /v1/create` (规约要求 Body) | 命令行前端预检报错，不发起物理请求，保护带宽。 |
| CLI-CAL-04 | 无效 JSON Body | `owenc api POST ... -d '{invalid_json}'` | 提示 JSON 格式错误，并给出修正建议。 |

---

## 3. 守护进程与系统运维 (Daemon & System)

### 3.1 `daemon` 生命周期
| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| CLI-DMN-01 | 端口冲突检测 | 已占 8080 端口后运行 `daemon start` | 提示端口占用错误，不会产生僵尸进程。 |
| CLI-DMN-02 | 多 Profile 进程共存 | 分别启动 `default` 和 `prod` 的 daemon | 产生两个独立的 `.pid` 文件，各自独立维护连接。 |
| CLI-DMN-03 | 优雅停止 | `owenc daemon stop` | 正常终止进程，清理本地 `.pid` 文件。 |
| CLI-DMN-04 | 强制重启 | `owenc daemon restart --all` | 一键重置所有环境的后台连接。 |

### 3.2 自动运维特性 (Advanced)
| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| CLI-SYS-01 | Build-ID 版本自愈 | 替换二进制文件后执行 `api list` | 检测到版本不一致，自动触发 `Daemon Restart`，随后执行业务。 |
| CLI-SYS-02 | 系统一键重置 | `owenc reset` | 停止守护进程、清空 Vault 凭据、删除缓存，环境回归纯净。 |
| CLI-SYS-03 | 动态名称注入 | 查看 `status` 标题 | 标题显示为 Makefile 注入的二进制名称大写（如 `OWENC-TEST`）。 |

---

## 4. 边界与压力测试 (Edge & Stress)

| 用例 ID | 场景描述 | 验证操作 | 预期结果 |
| :--- | :--- | :--- | :--- |
| EDGE-01 | 超长响应体处理 | 调用返回 10MB+ JSON 的接口 | CLI 应流式解析或设置合理内存上限，不应 OOM 崩溃。 |
| EDGE-02 | 高频调用签名校验 | 1 秒内连续执行 5 次 `api` 调用 | Nonce 与 Timestamp 应正确生成，不应被云端判定为重放攻击。 |
| EDGE-03 | 弱网环境行为 | 设置 1kb/s 限速下执行 `api spec` | 触发超时保护（默认 30s），抛出友好的网络异常提示。 |
| EDGE-04 | 磁盘空间满 | 在磁盘满的分区执行 `init` | 捕获文件写入异常，不应破坏现有的配置文件完整性。 |
