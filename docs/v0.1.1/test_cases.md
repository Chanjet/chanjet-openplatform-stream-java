# v0.1.1 核心质量保证：全景测试用例库 (Test Case Library)

> **版本定位**：v0.1.1 开发侧质量红线  
> **验证原则**：TDD 并行、100% 接口 Mock 化、100% JSON 输出校验、100% 恢复建议校验。

---

## 1. 核心底盘领域 (Core Framework)

### 1.1 全息配置引擎 (`core/config`)
| 用例 ID | 场景描述 | 预期行为 | 验证级别 |
| :--- | :--- | :--- | :--- |
| CF-CFG-01 | 多 Profile 覆盖合并 | `default` 配置与 `test-env` 指定参数合并，Flag > Profile > File。 | Unit |
| CF-CFG-02 | 参数热监听 (`Watch`) | 手动修改 `config.yaml` 触发回调，不重启 CLI 更新日志等级。 | Integration |
| CF-CFG-03 | 非法类型读取 | 读取 Bool 键为 String 时，需抛出带 `Suggestion` 的 JSON 错误。 | Unit |

### 1.2 物理秘钥冷库 (`core/vault`)
| 用例 ID | 场景描述 | 预期行为 | 验证级别 |
| :--- | :--- | :--- | :--- |
| CF-VLT-01 | OS Keyring 完整存取 | 调用系统 Keychain 存储解密成功，不产生本地明文。 | Unit |
| CF-VLT-02 | 降级回切 (.seal) | 模拟 `keyring` 编译环境缺失或由于 Headless 拒绝访问，自动回切至 AES-GCM 硬盘封印。 | Unit |
| CF-VLT-03 | 篡改检测 | 硬改 `.seal` 文件内容，尝试 `Load` 需触发 `Panic` 拦截并回吐非法签名 suggestion。 | Security |

### 1.3 降压轮询状态机 (`core/auth`)
| 用例 ID | 场景描述 | 预期行为 | 验证级别 |
| :--- | :--- | :--- | :--- |
| CF-ATH-01 | Single-Flight 惊群抑制 | 100 个并发 `RequireToken` 仅触发 1 次 `OpenCloud` 物理网络调用。 | Mock / Stress |
| CF-ATH-02 | 异步票据注入 (`Inject`) | 收到 Stream 底座推流后，内存池秒级更新且后续请求取到新票。 | Integration |
| CF-ATH-03 | 指数退避刷新 | 刷新 Token 失败时，执行带 Jitter 的指数退避，不持续轰炸云端。 | Unit |
| CF-ATH-04 | 恶意 Token 注入检测 | 向 `InjectAppTicket` 注入已过期或格式错误的凭证，状态机应维持旧票并告警。 | Security |
| CF-ATH-05 | 并发竞争压力测试 | 10 个线程同时尝试触发刷新且其中 5 个中途取消 Context，验证锁释放安全性。 | Stress |

---

## 2. 守护生命线领域 (Daemon Domain)

### 2.1 长效流信道守望者 (`daemon/stream`)
| 用例 ID | 场景描述 | 预期行为 | 验证级别 |
| :--- | :--- | :--- | :--- |
| DN-STM-01 | 自动闪连重试 | TCP 断开后，按照 1s, 2s, 4s... 阶梯自动重连并打印系统日志。 | Integration |
| DN-STM-02 | 报文脱壳与合法性校验 | 处理非法格式 WebSocket 帧不崩溃，丢弃并记录 `access.log`。 | Unit |
| DN-STM-03 | Profile 动态切流 | `stream stop` 时平滑关闭连接，退出后台常驻。 | Unit |
| DN-STM-04 | 握手超时心跳保护 | 连接长时间无 Ping 帧回应，主动断开并触发重新连接逻辑。 | Integration |

### 2.2 双轨靶场与死信拦截闸 (`daemon/proxy`)
| 用例 ID | 场景描述 | 预期行为 | 验证级别 |
| :--- | :--- | :--- | :--- |
| DN-PRX-01 | 指向性倒灌 (Static) | 接收消息后，精准 PUT 到 `webhook.target` 指向的本地靶机。 | Integration |
| DN-PRX-02 | 瞬态网络抢救 (Retry) | 靶机 503 时，立即执行 3 次内爆发式连射重试（间隔 < 100ms）。 | Mock |
| DN-PRX-03 | 死信沉降 (.sqlite) | 重试耗尽后，Payload 存入 `dlq.sqlite`，状态变更为 `buried`。 | Integration |
| DN-PRX-04 | 人工起死回生 (`Retry`) | `dlq retry <uuid>` 后，报文重新进入 Proxy 发射管并成功送达。 | E2E |
| DN-PRX-05 | 磁盘满额拒绝策略 | `dlq.sqlite` 因磁盘满写入失败，需向 `system.log` 抛出 FATA 级高亮并优雅退出。 | Chaos |
| DN-PRX-06 | 零字节 Payload | 收到空消息体报文，不应触发重试逻辑，直接记录为非法请求。 | Edge Case |

---

## 3. 面向 Agent 与命令行交互 (Interface & Agent)

### 3.1 格式回吐与恢复建议
| 用例 ID | 场景描述 | 预期行为 | 验证级别 |
| :--- | :--- | :--- | :--- |
| UI-AGI-01 | 全局 JSON 防污染 | 所有子命令开启 `--format json` 后，绝对禁止出现任何 ASCII 转义符。 | CLI Test |
| UI-AGI-02 | 错误自愈建议 | `vault Load` 失败时，JSON 必须包含字段 `recover_suggestion` 且内容具备实操性。 | CLI Test |
| UI-AGI-03 | 非交互模式安全 | 在 `stdin` 关闭时，所有需交互地确认场景默认选 `No` 并报错，永不死锁。 | CLI Test |
| UI-AGI-04 | 向量搜索格式化检查 | `api search` 返回的 Markdown 片段在 JSON 包装下仍保留正确的换行符逸码。 | Agent Test |
| UI-AGI-05 | 环境变量热覆盖 | 通过 `CJT_AUTH_CERT` 覆盖磁盘 Profile 证书，验证配置加载优先级顺序。 | CLI Test |
