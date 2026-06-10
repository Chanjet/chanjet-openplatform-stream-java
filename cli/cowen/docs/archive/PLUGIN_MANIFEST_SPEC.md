# 插件清单规范 (Plugin Manifest Spec)

本文档详细说明 `plugin.json` 中各个核心概念的语义界限，特别是**能力（Capabilities）**、**权限（Permissions）**与**扩展点（Contributions）**的区别，以指导开发者正确声明插件行为。

## 1. 核心概念对比图谱

在 Cowen 插件体系中，我们通过严格的声明式契约来分离**兼容性、安全性与扩展性**：

| 概念维度 | 对应 JSON 字段 | 解决的核心问题 | 谁来校验 | 表现形式 |
| **传输层协议** (Transport) | `transport` | **怎么通信**？(协议层) | 插件管理器 (启动时) | 宿主唤起插件并与之通信的数据通道方式（例如：`stdio`, `grpc`, `wasm_bindgen`）。注意：这**仅仅是数据包传递的物理方式**，不是业务能力。 |
| **能力依赖** (Capabilities) | `required_capabilities` | **宿主能不能**？(兼容性) | 插件管理器 (加载时静态校验) | **宿主提供给插件，供插件主动调用**的底层 API 和基础设施（例如：调用宿主的 HTTP 代理隧道）。 |
| **安全权限** (Permissions) | `requested_permissions` | **插件可不可**？(安全性) | 安全网关 (运行时动态校验) | 插件是否有权触碰用户敏感资产或外围物理环境（文件、网络）。 |
| **扩展点注入** (Contributions) | `contributes` / `cowen_extension` | **插件扩展了什么**？(扩展性) | 宿主控制面 (注册与调用时) | **插件提供给宿主，供宿主识别并主动调用**的业务能力（例如：暴露一个子命令、提供一个嵌入检索算法）。 |

---

## 2. 能力依赖 (`required_capabilities`)

**定义**：声明插件运行时**必须要宿主提供哪些特定的基础设施或内部 API**。这是插件能够被正常加载的“物理法则”。

*   **本质**：这是**宿主提供给插件的功能范围**（Host -> Plugin）。如果宿主版本过低、被裁剪或用户禁用了该功能，插件将由于不满足前置条件而被直接拒绝加载，从而防止运行时崩溃。
*   **交互方向**：插件调用宿主。
*   **安全属性**：无。这里定义的是内部 API 连通性协议，不对用户敏感资产构成直接威胁，因此不需要弹窗授权。

**典型声明示例**：
```json
"required_capabilities": {
  "native.api.registry": "v1",     // 允许插件调用宿主的 api_list / api_spec 内部元数据接口
  "native.api.proxy": "^1.0.0"     // 依赖宿主提供的 HTTP 代理隧道能力
}
```

---

## 3. 安全权限 (`requested_permissions`)

**定义**：声明插件需要触碰**用户的敏感资产、隐私数据或执行高危物理操作**的意图。这是构建“安全沙箱”的基石。

*   **本质**：这是基于用户信任的**安全拦截沙箱**。宿主可能具备删除文件、访问公网的能力，但除非插件申请了对应权限并获得用户（或管理员）同意，否则宿主的安全网关会在运行时（Runtime）强行阻断这些调用。
*   **交互方向**：插件尝试穿越沙箱访问外部资源。
*   **安全属性**：强用户授权。必须接受安全审计或明确的授权管控。

**典型声明示例**：
```json
"requested_permissions": {
  "native.api.registry:search": true,      // 允许插件调用本地检索引擎（可能泄露本地代码隐私）
  "sys.fs:write": false,                   // 允许插件直接修改本地磁盘文件
  "sys.network:outbound": true             // 允许插件脱离宿主代理，直接发起不可见的外网请求
}
```

---

## 4. 扩展点注入 (`contributes` / `cowen_extension`)

**定义**：声明插件**能为宿主带来什么新的功能**，主动向宿主的控制面（CLI、HTTP 网关、定时任务等）注册挂载点。

*   **本质**：这是**需要宿主识别并调用的插件能力**（Host <- Plugin）。当宿主解析到这些声明时，会在自己的生命周期中预留坑位（例如注册一个新的 CLI 子命令），当外部流量命中坑位时，宿主会负责把请求路由给插件处理。
*   **交互方向**：宿主调用（路由到）插件。
*   **安全属性**：无直接关联。它仅仅是功能的自然扩展暴露。

**典型声明示例**：
```json
"contributes": {
  "providers": [
    {
      "type": "SearchEmbedding",
      "version": "1.0",
      "description": "基于 ONNX 的本地向量 Embedding 服务扩展"
    }
  ],
  "cli_commands": [
    {
      "name": "mcp",
      "description": "MCP 协议支持命令行扩展"
    }
  ]
}
```

*   **设计解惑 (Design FAQ)**:
    *   **Q: 为什么 `SearchEmbedding` 和 `MCP Server` 属于 `contributes` 而不是 `required_capabilities`？**
        *   A: 因为它们是**插件赋予宿主的新能力**。宿主在启动时，会扫描 `contributes.providers` 和 `contributes.cli_commands`，一旦识别到 `SearchEmbedding`，宿主就会知道：“哦，如果有人要查向量，我可以通过 `transport`（比如 stdio）把任务委派给这个插件”。它属于“扩展点”（Extension Point），而不是插件对宿主的“依赖”。
    *   **Q: 为什么 `stdio` 要独立成 `transport` 而不是 `capabilities` 的一部分？**
        *   A: 因为 `stdio` 只是一种“数据传输方式”（像网线一样）。宿主通过标准输入/输出唤起并传输 JSON-RPC 消息给插件。插件并不“依赖” stdio 这个业务能力，而是双方约定采用这种**传输协议**来完成请求的收发。未来如果是 WebAssembly 插件，`transport` 就会变成 `wasm_bindgen`，但它们可能贡献的 `contributes` (扩展点) 是一模一样的。

---

## 5. 总结：通俗类比的联动关系

如果把整个 Cowen 系统比作一家**公司**，插件就是一个外来的**外包团队**，那么 `plugin.json` 就是签署的**外包合同**：

1. **`transport` (传输协议)** 是外包团队使用的**沟通方式**：
   * “我们需要通过对讲机 (stdio) 跟总台联系。”
2. **`required_capabilities` (能力依赖)** 是外包团队自带的**工单需求**：
   * “我们需要公司提供 V1 版本的办公桌和内网代理接口（`native.api.proxy`），否则我们没法办公。”
   * **作用**：宿主检查自己的核心库是否能提供这些 API，如果不能提供，就拒签合同（拒绝加载）。
3. **`requested_permissions` (安全权限)** 是外包团队向保安部提交的**通行证申请**：
   * “我们需要权限去检索本地的 API 元数据 (`native.api.registry:search`) 和修改磁盘文件 (`sys.fs:write`)。”
   * **作用**：保安部（用户）在运行时进行拦截或放行，保护公司底层物理环境和隐私。
4. **`contributes` (扩展点注入)** 是外包团队在公司大堂挂出的**业务招牌**：
   * “我们可以对外提供『向量分析 (`SearchEmbedding`)』和『MCP 服务器接入 (`cli_command: mcp`)』的专属服务，请前台（CLI/Daemon 宿主）把有对应需求的任务派发给我们。”
   * **作用**：宿主读取招牌后，对外暴露新能力，并负责把命中该能力的客流请求，顺着 `transport` 通道（如 stdio）路由给插件处理。
