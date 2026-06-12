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
- **ADR-006 [Cloud-Native Configuration Strategy]**: 彻底摒弃单一环境变量注入的臃肿模式。采用“动静分离，双剑合璧”策略：复杂的网关静态路由黑白名单使用纯净 YAML 维护并通过 ConfigMap 挂载 (GitOps Friendly)；而高度敏感的机密数据 (AppKey, AppSecret, DB_URL) 以及动态微调参数，严格通过 K8s Secret 转化为 `COWEN_` 前缀的环境变量注入。环境变量具备绝对最高优先级，在内存中自动合并覆盖物理文件，实现机密信息零落盘。
