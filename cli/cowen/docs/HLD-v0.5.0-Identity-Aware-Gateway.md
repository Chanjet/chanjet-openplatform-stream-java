# HLD v0.5.0 - Identity-Aware Gateway (架构级概要设计)

## 1. 系统上下文视图 (System Context Topology)
系统边界与外部依赖关系明确如下：
- **Cowen Gateway (核心)**: 扮演 Identity-Aware Proxy 角色，接管 Ingress 鉴权清洗与 Egress 代理组装。
- **畅捷通开放平台 (IdP & API)**: 提供 OAuth 换票能力、下发 `code`，并作为被调用的远端业务 API 资源服务器。
- **ISV 业务后端 (ISV Backend)**:
  - 角色 1 (Pull 模式): 接收被 Cowen 放行的流量，通过读取 HTTP Header (`x-org-id`, `x-user-id`) 自省身份，并下发本地 `isv_session`。
  - 角色 2 (Sync Hook 模式): 暴露出同步的 `auth_sync_hook` 接口供 Cowen 换票成功时阻塞回调，在回调响应中注入业务侧 `Set-Cookie`。
  - 角色 3 (Data Receiver): 暴露异步的 `webhook_target` 接口，统一接收 Cowen 转发过来的开放平台长链接业务消息与事件通知。
- **Store SPI (基础设施)**: (如 Redis, MySQL, ETCD) 在保留原有 App 级后台凭证（如 `app_ticket` 和应用级 token）缓存能力的基础上，新增存储自治轮转的全局 JWKS 密钥集。对于**网关拦截到的任何浏览器端“用户会话状态”，坚决不落盘、不存储**。
- **浏览器端 (Browser)**: 需配置 Ajax `withCredentials: true`，负责持有双路 Cookie (`cowen_sess_id` 与 `isv_session`)。

## 2. 部署与物理视图 (Deployment Architecture)
Cowen 支持极致轻量的 **Sidecar (边车) 部署模式** 与 集中式 Gateway 模式。

- **同 Pod/同域名部署 (首选 Sidecar)**:
  - Cowen 与 ISV Backend 必须处于同一顶层域名下，保证 302 洗白后 `SameSite=Lax` 的 Cookie 不会跨域丢失。
  - **Ingress (入网)**: 绑定对外公开端口（如 `8080`），作为外部流量的第一道防线。
  - **Egress (出网 Native Proxy)**: 绑定本机回环地址 `127.0.0.1:8081`。依靠物理网络隔离，ISV 后端将 HTTP Proxy 指向该本地端口，实现向开放平台发送 API 请求的免配置自动签权。
- **远端集中式部署 (配合 WASM 插件)**:
  - 若 ISV 部署在异地网络无法直连 `8081`，通过配置防火墙 (Security Group) 管控网关，并挂载 `token-exporter` WASM 插件。网关在透传请求给异地 ISV 时，将底层 Token 作为 Header 远程导出。

## 3. 非功能性需求设计 (NFRs)
- **安全性 (Security & Hardening)**:
  - 废除 Session 存储：使用 A256GCM 强加密 JWT (JWE) 派发给客户端，密文外部绝对不可见。
  - 放弃 Revocation List (黑名单)：拥抱绝对无状态，通过离散滑动窗口的超短空闲时间 (如 30 分钟) 限制最大安全暴露敞口 (Blast Radius)。
  - 凭证加固机制：Cookie 强制使用 `HttpOnly`, `Secure`, `SameSite=Lax`。在 JWE 中签入基于 User-Agent 和 IP 段的 Hash 指纹 (`fp`)，发生环境剧变立刻销毁。
  - 零容忍时钟漂移：系统时间偏差校验策略设置为零 (0 leeway)，强制倒逼基建层配置高精度 NTP 服务，杜绝被时间差攻击。
- **高可用与可观测性 (HA & Observability)**:
  - 极度水平伸缩 (Scale-Out)：网关进程全无状态，任意流量漂移均可在本地利用内存 JWKS 解密自给自足。
  - WASM 沙盒动静分离：`token-exporter` 插件禁止通过配置文件获取 Secret，必须使用宿主机暴露的 `Host Vault API` 读取，保障宿主基建安全。
  - 跨域平滑：识别 `OPTIONS` 预检请求并无条件绿灯放行，不校验身份。

## 4. 架构决策记录 (ADR - Architecture Decision Records)
- **ADR-001 [Auth Mode]**: 拒绝潜规则，强制实施声明式路由。引入 `STRICT` (默认拦截+白名单 Bypass) 和 `PERMISSIVE` (默认旁路+黑名单 Require) 两种模式，适应老系统无损接入。
- **ADR-002 [Sync Hook]**: 为减少页面多次拉取，提供同步切面阻塞模式，在拦截到 `code` 时不仅内部换票，还会阻塞调用业务 Webhook 并合并业务侧返回的 `Set-Cookie`，实现一次 302 跳跃下发双重 Cookie。
- **ADR-003 [CORS Fallback]**: 处理拦截降级兜底时，根据请求头决定阻断策略：`Accept: application/json` 的数据交互直接阻断返回 HTTP 401（附带 login_url），`Accept: text/html` 的浏览器导航请求则转换为 `state` 后返回 HTTP 302 重定向免登。
- **ADR-004 [Code Precedence]**: 确立“协议握手优先级 > 路由规则优先级”。如果白名单中的路由携带了 `code`，无视放行规则，强制拦截 `code` 换票、洗白跳转，并下发身份 Session。
- **ADR-005 [Autonomous Key Rotation]**: 废除人工配 Key。网关启动及按周期通过 Store SPI 获取与生成 JWKS。使用多版本共存与 `kid` (Key ID) 寻址，达成零人工干预的平滑密钥轮转。
- **ADR-006 [Cloud-Native Configuration Strategy]**: 彻底摒弃单一环境变量注入的臃肿模式。采用”动静分离，双剑合璧”策略：复杂的网关静态路由黑白名单使用纯净 YAML 维护并通过 ConfigMap 挂载 (GitOps Friendly)；而高度敏感的机密数据 (AppKey, AppSecret, DB_URL) 以及动态微调参数，严格通过 K8s Secret 转化为 `COWEN_` 前缀的环境变量注入。环境变量具备绝对最高优先级，在内存中自动合并覆盖物理文件，实现机密信息零落盘。
- **ADR-007 [Dual-Timestamp Discrete Sliding Window]**: 在无状态 JWT 架构中，既要避免”每次请求都刷新 JWT”带来的带宽与 CPU 浪费，又要实现”用户持续操作则永不掉线”的体验。采用双时间戳 + 刷新阈值算法：
  - **双时间戳**：`abs_exp`（绝对过期，如 24h）与 `idle_exp`（空闲过期，如 30min）共存于 JWE 载荷中。
  - **安全区 (免刷新)**：若 `idle_exp - now() > 10min`，网关不消耗任何性能，直接放行。
  - **临期阈值区 (触发刷新)**：若 `idle_exp - now() ≤ 10min`，网关在内存中生成新 JWE（`idle_exp` 推至 `now() + 30min`），在响应头中静默下发 `Set-Cookie`。
  - **空闲超时失效**：若 `now() > idle_exp`，判定会话死亡，按标准执行 401/302 降级策略。
  - **架构收益**：将刷新频率从”每次点击”降维到”每 20 分钟一次”，同时保证活跃用户永不下线。
- **ADR-008 [Stateless Session Revocation & Blast Radius]**: 无状态 JWT 的核心痛点是”覆水难收”——无法像中心化 Session 那样主动吊销已被盗用的 Token。工程决断如下：
  - **拒绝黑名单**：不引入任何内存或 Redis 吊销列表（Revocation List），坚守绝对无状态。
  - **主动登出**：仅负责在浏览器侧响应 `Set-Cookie` 删除指令，清理客户端凭证。
  - **最大暴露敞口控制**：依托 ADR-007 的 `idle_exp` 机制，将 Token 物理泄漏后的理论最大暴露窗口（Blast Radius）严格控制在 **30 分钟**以内。
  - **架构收益**：在”无状态可扩展性”与”安全吊销能力”之间做出了明确的取舍，以极低概率事件换取极致的水平扩展能力。
- **ADR-009 [Native Proxy Dual-Mode Deployment]**: Cowen 内置 Egress 代理根据不同部署拓扑采取差异化的网络绑定与安全策略：
  - **Sidecar 模式（默认）**：Egress 端口强制绑定 `127.0.0.1`（Loopback），依赖物理网络绝对隔离。仅同 Pod/主机的 ISV 进程可访问，无需额外认证，实现零配置的透明正向代理。
  - **远端集中式模式**：Egress 端口绑定 `0.0.0.0`，允许跨网络访问。在此模式下，Cowen 摒弃应用层 IP 白名单配置，**全权交由基础设施级防火墙（Security Group）管控**，实现”零认知摩擦”。
  - **模式切换**：通过 `bind_address` 字段自动识别——前缀为 `127.` 则启用 Sidecar 模式，否则启用远端模式并输出安全警告日志。
  - **架构收益**：以最低的配置复杂度覆盖两种典型部署拓扑，在安全性与灵活性之间取得平衡。
- **ADR-010 [Bypass-Aware Identity Injection]**: 当请求命中白名单路由（免认证）但浏览器已持有合法 `cowen_sess_id` 时，网关不强制校验身份，但会**顺手将明文身份注入 HTTP Header (`x-org-id`, `x-user-id`)** 后透传给 ISV 后端。此决策确保：
  - 白名单页面的 ISV 业务代码仍可**按需读取**平台身份（如用于”已登录则显示用户头像”的体验优化）。
  - 若白名单路由携带了 `code`，ADR-004 的全局拦截优先权先于本规则执行（洗白跳转后下发 Session，再回到本规则注入身份）。
  - 不强制校验意味着：即使 Cookie 过期或缺失，白名单请求依然正常放行，只是不注入身份 Header。
  - **架构收益**：在”零信任”和”渐进式体验增强”之间取得平衡，让 ISV 可以在白名单页面上自主利用已有身份信息。
- **ADR-011 [Cookie Security Fortress]**: 网关下发的 `cowen_sess_id` Cookie 强制硬编码安全属性，杜绝前端脚本窃取与跨站伪造：
  - `HttpOnly`: 绝对防范 XSS 通过 `document.cookie` 读取 JWE 密文。
  - `Secure`: 强制仅通过 HTTPS 传输，防止中间人抓包获取 Cookie。
  - `SameSite=Lax`: 防御 CSRF 攻击，同时兼容从”畅捷通工作台/应用商店”发起的首跳免登（IdP-Initiated SSO），因为首跳属于顶层导航（Top-Level Navigation），Lax 策略允许携带 Cookie。
  - **架构收益**：以三层 Cookie 属性构建纵深防御，将凭证泄露风险降至最低，不依赖上层应用的安全编码水平。
- **ADR-012 [Cross-Origin Credential Carrying]**: 在前后端分离的跨域部署架构中，浏览器安全策略默认隔离第三方 Cookie 的收发。为打通 5.3 节”滑动窗口离散续期”中网关静默下发新 JWE 的闭环，对 ISV 前端下达硬性规范：
  - 全局网络拦截器（Axios, Fetch, XHR）必须配置 `withCredentials: true`。
  - 这允许浏览器在跨域 Ajax 请求中携带 `cowen_sess_id` Cookie，并接受网关下发的 `Set-Cookie` 刷新本地凭证。
  - 同时要求 Cowen 网关在 CORS 预检响应中返回 `Access-Control-Allow-Credentials: true`。
  - **架构收益**：ISV 后端服务端零侵入，前端仅需一行全局配置，即可打通无感知续杯的全链路。

## 5. 整体认证流程图 (End-to-End Authentication Flow)

以下流程图将 HLD 中所有 ADR 决策串联为一个完整的端到端认证链路，覆盖从首次访问到 API 代理转发的全生命周期。

```mermaid
flowchart TD
    Start([“用户访问 ISV 应用页面”]) --> HasCode{“URL 中是否携带<br/>code 参数?”}

    %% ========== 阶段一：Code 拦截与换票 ==========
    HasCode -->|”是 /invoice?code=xxx”| CodeIntercept[“【ADR-004 全局拦截】<br/>无视路由规则，强制拦截 code”]
    CodeIntercept --> Exchange[“向开放平台发起<br/>后台换票请求”]
    Exchange -->|”重试: Exponential(100ms, 2s, 3次)”| IdPResult{“换票结果?”}
    IdPResult -->|”成功”| SyncHook{“配置了<br/>auth_sync_hook?”}
    IdPResult -->|”失败”| IdpFail[“HTTP 502<br/>提示用户重试”]

    SyncHook -->|”是”| SyncHookCall[“【ADR-002 Sync Hook】<br/>阻塞调用 ISV Webhook<br/>传递 org_id + user_id”]
    SyncHook -->|”否”| GenJWE[“生成 JWE 载荷<br/>idle_exp = now+30m<br/>abs_exp = now+24h<br/>fp = sha256(IP+UA)”]

    SyncHookCall -->|”重试: Linear(200ms, 2次)”| HookResult{“Hook 结果?”}
    HookResult -->|”200 OK”| MergeCookie[“提取 ISV 返回的<br/>Set-Cookie: isv_session”]
    HookResult -->|”超时/500”| HookDegrade[“【优雅降级】<br/>放弃 Hook，仅下发<br/>网关 Cookie”]

    MergeCookie --> GenJWE
    HookDegrade --> GenJWE

    GenJWE --> EncryptJWE[“JWE 加密<br/>alg: dir, enc: A256GCM<br/>kid: 当前 ACTIVE Key”]
    EncryptJWE --> Wash302[“302 Redirect 至纯净地址<br/>去除 code 参数”]
    Wash302 --> SetCookie[“Set-Cookie: cowen_sess_id=JWE<br/>HttpOnly; Secure; SameSite=Lax<br/>【ADR-011 Cookie 铁桶阵】”]
    SetCookie -->|”若 Sync Hook 成功”| SetISVCookie[“同时下发<br/>Set-Cookie: isv_session”]
    SetCookie --> BrowserRedirect[“浏览器重定向至<br/>纯净地址”]

    %% ========== 阶段二：请求拦截与路由决断 ==========
    HasCode -->|”否 (纯净 URL)”| CarryCookie[“请求携带 Cookie<br/>cowen_sess_id (浏览器自动)”]
    BrowserRedirect --> CarryCookie

    CarryCookie --> CorsCheck{“HTTP Method<br/>== OPTIONS?”}
    CorsCheck -->|”是”| CorsPass[“【CORS 预检穿透】<br/>返回 200/204<br/>Access-Control-Allow-Origin<br/>Access-Control-Allow-Credentials: true<br/>【ADR-012】”]
    CorsCheck -->|”否”| RouteCheck{“路由规则匹配<br/>【ADR-001】”}

    RouteCheck -->|”STRICT: 命中 bypass_rules<br/>或 PERMISSIVE: 未命中 require_rules”| IsBypass[“is_auth_required = false”]

    RouteCheck -->|”STRICT: 未命中 bypass_rules<br/>或 PERMISSIVE: 命中 require_rules”| IsRequire[“is_auth_required = true”]

    %% ========== 阶段三：会话自省 ==========
    IsBypass --> HasCookie{“请求中是否携带<br/>cowen_sess_id?”}
    HasCookie -->|”是”| DecryptBypass[“解密 JWE 并校验指纹”]
    DecryptBypass -->|”解密成功”| InjectBypass[“【ADR-010 顺手注入】<br/>注入 Header:<br/>x-org-id, x-user-id<br/>但 不强制校验”]
    DecryptBypass -->|”解密失败/过期”| PassBypass[“静默放行<br/>不注入身份 Header”]
    HasCookie -->|”否”| PassBypass
    InjectBypass --> ProxyToISV
    PassBypass --> ProxyToISV

    IsRequire --> HasCookie2{“请求中是否携带<br/>cowen_sess_id?”}
    HasCookie2 -->|”否”| UnAuth{“判断请求类型<br/>【ADR-003 CORS Fallback】”}
    HasCookie2 -->|”是”| DecryptJWE[“解密 JWE<br/>1. 读取 Header kid<br/>2. 从 JWKS 查找密钥<br/>3. 解密 + 验证 fp 指纹”]

    DecryptJWE -->|”fp 不匹配”| FpReject[“【指纹防盗刷】<br/>视同未登录<br/>WARN 日志记录”]
    DecryptJWE -->|”解密成功”| TimeCheck{“时间有效性校验<br/>leeway = 0 【ADR-005】”}

    TimeCheck -->|”now > abs_exp”| AbsExpired[“绝对过期<br/>超过 24h 未重新登录”]
    TimeCheck -->|”now > idle_exp”| IdleExpired[“空闲过期<br/>超过 30min 无操作<br/>【ADR-008 最大暴露敞口】”]
    TimeCheck -->|”有效”| SlideCheck{“滑动窗口判断<br/>【ADR-007】”}

    SlideCheck -->|”remaining > 10min”| SafeZone[“安全区<br/>不刷新 JWE<br/>直接放行”]
    SlideCheck -->|”0 < remaining ≤ 10min”| ThresholdZone[“临期阈值区<br/>生成新 JWE<br/>idle_exp 推至 now+30m”]
    ThresholdZone --> HookRefresh[“挂载响应拦截 Hook<br/>在 Response 中追加<br/>Set-Cookie: cowen_sess_id=新JWE”]

    FpReject --> UnAuth
    AbsExpired --> UnAuth
    IdleExpired --> UnAuth

    UnAuth -->|”Accept: application/json<br/>(Ajax/Fetch 请求)”| Return401[“返回 HTTP 401<br/>Body 附带 login_url<br/>【ADR-003】”]
    UnAuth -->|”Accept: text/html<br/>(浏览器导航)”| Return302[“302 Redirect 至<br/>开放平台登录页<br/>state = 当前页面路径<br/>【ADR-003 State 编码】”]

    SafeZone --> InjectRequire[“注入 Header:<br/>x-org-id: JWE.org_id<br/>x-user-id: JWE.user_id”]
    ThresholdZone --> InjectRequire

    InjectRequire --> ProxyToISV[“反向代理至 upstream_url<br/>(ISV 业务后端)”]

    %% ========== 阶段四：ISV 后端处理 ==========
    ProxyToISV --> ISVReceive[“ISV 后端接收请求<br/>读取 Header 中的<br/>x-org-id / x-user-id”]
    ISVReceive --> ISVSession{“本地是否有<br/>isv_session?”}
    ISVSession -->|”无”| ISVCreateSession[“关联历史账号<br/>下发 Set-Cookie: isv_session<br/>建立业务态”]
    ISVSession -->|”有”| ISVPass[“直接放行<br/>渲染业务页面”]
    ISVCreateSession --> ISVPass

    %% ========== 阶段五：Egress 代理 ==========
    ISVPass --> NeedAPI{“是否需调用<br/>开放平台 API?”}
    NeedAPI -->|”否”| Done([“返回页面给用户”])
    NeedAPI -->|”是”| EgressCall[“通过本地 Egress 代理<br/>127.0.0.1:8081<br/>Header 携带:<br/>x-org-id + x-user-id”]

    EgressCall --> EgressLookup[“hash(x-org-id + x-user-id)<br/>查 LRU 内存缓存<br/>定位 JWE 会话”]
    EgressLookup -->|”命中”| EgressDecrypt[“解密 JWE<br/>获取 open_token”]
    EgressLookup -->|”未命中”| EgressFail[“HTTP 502<br/>GW_NO_SESSION_FOR_EGRESS”]

    EgressDecrypt --> EgressInject[“组装请求:<br/>Authorization: Bearer open_token<br/>改写 Host → 开放平台网关”]
    EgressInject --> EgressSend[“通过连接池发送请求”]

    EgressSend -->|”响应 401”| EgressRefresh{“尝试 refresh_token<br/>同步刷新”}
    EgressRefresh -->|”刷新成功”| EgressRetry[“以新 Token<br/>重放请求 (最多 1 次)”]
    EgressRefresh -->|”刷新失败”| Egress401[“返回 HTTP 401<br/>GW_EGRESS_TOKEN_EXPIRED”]
    EgressRetry --> EgressSend

    EgressSend -->|”成功”| EgressReturn[“原样返回<br/>Status Code + Headers + Body”]
    EgressReturn --> Done

    %% ========== 样式 ==========
    classDef stage fill:#1a1a2e,color:#e0e0e0,stroke:#16213e
    classDef cookie fill:#0f3460,color:#e0e0e0,stroke:#16213e
    classDef decision fill:#533483,color:#fff,stroke:#3a2a5c
    classDef action fill:#16213e,color:#e0e0e0,stroke:#0f3460
    classDef error fill:#5c1a1a,color:#ffcccc,stroke:#8b0000
    classDef success fill:#1a3a1a,color:#ccffcc,stroke:#006400
```

**流程说明**：

| 阶段 | 触发条件 | 关键决策 | 涉及 ADR |
|:---|:---|:---|:---|
| **阶段一** | URL 携带 `code` 参数 | 全局拦截换票 → 可选 Sync Hook → 302 洗白下发 Cookie | ADR-002, ADR-004, ADR-011 |
| **阶段二** | 纯净 URL 请求到达 | 路由规则匹配（STRICT/PERMISSIVE）→ 确定是否需要认证 | ADR-001, ADR-012 |
| **阶段三** | 路由决断完成 | JWE 解密 → 指纹校验 → 时间校验 → 滑动窗口判断 → 注入 Header | ADR-003, ADR-005, ADR-007, ADR-008, ADR-010 |
| **阶段四** | 请求抵达 ISV 后端 | 读取 x-org-id/x-user-id → 建立或复用 isv_session | — |
| **阶段五** | ISV 需调用开放平台 API | 通过 Egress 代理 → 缓存定位 → Token 注入 → 转发 → 401 自动刷新 | ADR-009 |

**关键数据流**：

| 通信边界 | 传递方式 | 携带内容 |
|:---|:---|:---|
| 浏览器 → Gateway (Ingress) | `Cookie: cowen_sess_id` (HttpOnly, 浏览器自动) | JWE 密文 |
| Gateway → ISV 后端 | HTTP Header 注入 | `x-org-id`, `x-user-id` |
| ISV 后端 → Gateway (Egress) | HTTP Header 回传 | `x-org-id`, `x-user-id` |
| Gateway (Egress) → 开放平台 | HTTP Header 注入 | `Authorization: Bearer <open_token>` |
