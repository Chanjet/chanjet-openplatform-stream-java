# Cowen CLI 深度验收测试报告

## 一、 验收基础信息

| 验收项 | 详细信息 |
| :--- | :--- |
| **验收时间** | 2026-06-11 |
| **验收版本** | cowen v0.4.0 (Build: 700e504) |
| **测试环境** | 集成测试环境 (inte) |
| **执行人** | 张亮 |

---

## 二、 验收参数配置说明

本次验收涵盖以下三组核心应用场景配置：

1. **商店应用 (Cli商店应用 - 集测)**
   - **企业 ID (orgId)**: `90001123021`
   - **沙箱环境 AppKey**: `ugtEQwms` / **Secret**: `<SANDBOX_APP_SECRET>`
   - **正式环境 AppKey**: `eMDiqlzR` / **Secret**: `<APP_SECRET>`

2. **自建应用配置 (cli自建应用配置)**
   - **AppKey**: `dqOk3anb`
   - **AppSecret**: `<APP_SECRET>`

3. **消息及证书配置 (OAuth2/自建应用共用加密)**
   - **消息秘钥 (AES Key)**: `<AES_KEY>`
   - **授权证书 (Certificate)**: `<CERTIFICATE_BLOB>`
   - **关联产品**: 马嘟嘟中心三 - 好业财应用

---

## 三、 测试用例执行与验收结果

### 1. 环境初始化与 Profile 隔离及生命周期管理

- **测试目的**：验证 Profile 隔离创建、重命名、切换及物理重置的生命周期。
- **执行命令**：
  ```bash
  # 初始化
  cowen init -p store_sandbox --app-mode store_app --app-key ugtEQwms --app-secret <SECRET>
  cowen init -p store_prod --app-mode store_app --app-key eMDiqlzR --app-secret <SECRET>
  cowen init -p self_built --app-mode self_built --app-key dqOk3anb --app-secret <SECRET> --encrypt-key <KEY> -c "<CERT>"
  cowen init -p oauth2_app --app-mode oauth2 --app-key 3NWdEbmu --encrypt-key <KEY>
  
  # 临时Profile生命周期测试
  cowen init -p temp_test --app-mode store_app --app-key tempkey --app-secret tempsecret
  cowen profile rename temp_test temp_test_renamed
  cowen profile use temp_test_renamed
  cowen profile current
  cowen reset -p temp_test_renamed --no-telemetry
  cowen profile use self_built
  cowen profile list
  ```
- **预期结果**：所有环境成功初始化；临时 Profile 能够成功重命名、激活并彻底 reset 清理，工作环境不受 any 干扰。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：所有命令回显完全正常，`profile list` 中仅留存我们定义的核心 4 套 Profile，无脏配置。

---

### 2. 配置精细化管理与读写测试 (`cowen config`)

- **测试目的**：验证单个 Profile 配置项 of 精细读取、动态改写与恢复。
- **执行命令**：
  ```bash
  cowen config list -p self_built
  cowen config get webhook_target -p self_built
  cowen config set webhook_target http://127.0.0.1:9999 -p self_built
  cowen config get webhook_target -p self_built
  cowen config set webhook_target http://localhost:8080 -p self_built
  ```
- **预期结果**：能够获取当前所有项；修改后立即生效并返回最新值；且能正确恢复原值。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：配置文件的读写操作由 Daemon 主进程协调完成，响应速度快，未发生锁冲突。

---

### 3. 身份认证与凭据状态管理 (`cowen auth`)

- **测试目的**：验证自建应用自动获取 Token、状态寿命检测与共享存储同步（reload）能力。
- **执行命令**：
  ```bash
  # 获取并展示 Token
  cowen auth token -p self_built
  # 从共享存储强制同步
  cowen auth reload -p self_built
  # 检查整体凭据剩余寿命
  cowen auth status -p self_built
  ```
- **预期结果**：
  1. 正确调用开放平台并返回遮蔽后的 AccessToken。
  2. `reload` 在多进程环境下能够重新对齐最新的本地连接数据库状态。
  3. `status` 完美列出包括 User ID、Org ID、App ID 在内的具体有效凭据。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：自建应用通过证书正常获得 `[VALID]` Token；OAuth2 (好业财) 在已登录的 Chrome 控制台下使用 CDP 顺利捕获带 PKCE 特征的完整授权链接，并顺利换取 Token。

---

### 4. API 智能发现、规约解析与接口调用 (`cowen api`)

- **测试目的**：验证 AI 本地语义搜索、特定端点的 OpenAPI 数据格式规约提取，及带签名注入的真实请求。
- **执行命令**：
  ```bash
  # AI 语义搜索
  cowen api list --search "获取部门列表" -p self_built
  # 提取 OpenAPI 规约
  cowen api spec POST /accounting/openapi/cc/department/list/{bookid} -p self_built
  # 真实接口调用测试 (--force 绕过客户端规约校验，直接访问平台)
  cowen api POST /accounting/openapi/cc/department/list/123456789 -p self_built -d '{}' --force
  ```
- **预期结果**：
  1. 语义搜索调用本地 embedding 推理机，返回最相似的 API Path。
  2. `spec` 能够打印出参数（Header/Path）、Request Body 的详细字段类型与成功响应体结构。
  3. API 请求被物理发送给云端，并返回由平台鉴权通过后但因为数据问题产生的业务级错误 (例如 `"code": "saas.app.e0001", "msg": "系统错误"`。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：直接调用的返回结果完全证实了 CLI 已经在底层完成合法的 OpenToken 与 appKey Header 注入，云端物理连通性完美。

---

### 5. 存储后端健康管理 (`cowen store`)

- **测试目的**：诊断主存储和缓存引擎后端的健康状态与连接性。
- **执行命令**：
  ```bash
  cowen store status -p self_built
  ```
- **预期结果**：输出当前 Profile 所关联 the 数据库类型（如 `innerdb`）以及健康信息。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：环境显示为 `innerdb` 且运行状态良好。

---

### 6. 系统事件流与诊断回溯 (`cowen events / doctor`)

- **测试目的**：检查网络、监控端口及平台网络层连通性的自检，以及事件轨迹查询。
- **执行命令**：
  ```bash
  cowen doctor
  cowen events -n 10 -p self_built
  ```
- **预期结果**：
  1. `doctor` 输出关于系统、配置、凭据、网络连通的 6 个全绿 [OK] 项。
  2. `events` 输出当前客户端的状态信息。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：新架构下 events 查询由主守护进程常驻处理，客户端端执行会抛出相应的轻量客户端指引并成功降级。

---

### 7. 死信队列管理与物理清除 (`cowen dlq`)

- **测试目的**：验证在长时间未正常重试或异常情况下的 DLQ 检测与一键物理清除。
- **执行命令**：
  ```bash
  # 列出 DLQ 
  cowen dlq list -p self_built
  # 物理清除
  cowen dlq purge -p self_built
  ```
- **预期结果**：显示当前队列积压（为空）；执行 purge 输出物理抹除事件数。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：DLQ 重试系统底层连接正常。

---

### 8. 扩展插件扫描与签名管理 (`cowen plugins`)

- **测试目的**：验证在沙箱与生产环境对第三方扩展包与内置插件的安全加固和权限扫描。
- **执行命令**：
  ```bash
  cowen plugins list
  ```
- **预期结果**：列出 `cowen-mcp-plugin` 与 `libcowen_search_embedding` 插件，并标明权限级别及 `Signed`（已签名）安全标识。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：插件系统的安全完整性通过。

---

### 9. 运行日志与业务操作审计跟踪 (`cowen log / audit`)

- **测试目的**：验证结构化审计日志尾部追踪，以及 CLI 运行日志的查看。
- **执行命令**：
  ```bash
  cowen audit tail -n 10 -p self_built
  cowen log view -n 10 -p self_built
  ```
- **预期结果**：能够直接在控制台输出最后 N 行日志流，便于开发者跟踪调试。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：输出内容规整、包含正确的时间戳与日志级别。

---

### 10. 代理鉴权、Webhook推送拦截与多租户换票联动测试 (`cowen daemon proxy / webhook`) [NEW]

- **测试目的**：重点验证在本地反向代理（Proxy）模式下的高级连通特征，包括自动鉴权头生成、自建应用免密调用、商店应用 Webhook 推送拦截解析，以及异步后台临时授权码换票业务流。
- **执行命令**：
  ```bash
  # 10.1 自建应用免密代理调用测试
  curl -s http://127.0.0.1:<PORT_SELF_BUILT>/accounting/openapi/cc/department/list/123456789 -H "Content-Type: application/json" -d '{}'
  
  # 10.2 商店应用 Webhook 拦截 (推送 AppTicket)
  curl -s -X POST http://127.0.0.1:<PORT_SANDBOX>/webhook -H "Content-Type: application/json" -d '{"type": "APP_TICKET", "app_ticket": "test_ticket_value_123456"}'
  cowen auth status -p store_sandbox
  
  # 10.3 商店应用 Webhook 拦截 (多租户 TempAuthCode 换票)
  curl -s -X POST http://127.0.0.1:<PORT_STORE_PROD>/webhook -H "Content-Type: application/json" -d '{"type": "TEMP_AUTH_CODE", "temp_auth_code": "temp_code_999", "state": "state_123"}'
  ```
- **预期结果**：
  1. **免密调用**：无需手动准备 token/sign，本地 curl 通过代理端口直接调通云端，云端正确识别并回显业务级错误（如 `账套信息错误`）。
  2. **Ticket 推送与状态同步**：向代理推送 `APP_TICKET` 后，代理正确回显 `{"code":"200","message":"success"}`，且 `cowen auth status` 显示 AppTicket 变为 `[CACHED]`。
  3. **多租户换票**：向代理推送 `TEMP_AUTH_CODE` 后，Daemon 后台进程瞬间截获，解密并提取，自动向平台发起换取 `appAccessToken` 与企业 `Token` 的调用链，由于是测试虚拟码，最终顺利触发云端 `appKey不正确` (401) 报错。
- **实际结果**：[x] 通过 / [ ] 未通过
- **备注说明**：代理端口由 Daemon 异步启动时随机申请。在脚本中通过动态流过滤端口号完成闭环验证。所有推送与异步换票流程均完全跑通。

---

## 四、 验收结论

[x] **完全通过**：所有核心功能均符合预期，性能及安全性符合要求。
[ ] **部分通过**：主要业务功能正常，但存在部分非核心功能异常（详见备注说明）。
[ ] **不通过**：关键链路（如认证/代理）不通，需打回重构。

**审批人签字：**                           **日期：** 2026年 6 月 11 日
