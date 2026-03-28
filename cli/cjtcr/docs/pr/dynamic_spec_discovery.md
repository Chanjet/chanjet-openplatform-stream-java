# PR: 自建应用动态 OpenAPI 接口发现功能需求

## 问题描述 (Problem Statement)

目前 `cjtc` CLI 依赖于物理维护的 `mock_openapi.json` 文件来提供命令自动补全、请求体生成和接口文档展示（通过 `api spec`）。这种方式存在以下弊端：
- **维护成本高**：每次平台增加或更新接口，CLI 侧的 Mock 文件都必须手动同步更新。
- **一致性风险**：Mock 文件极易与生产环境的实际接口定义脱节。
- **权限局限性**：用户只能看到被手动录入 Mock 的接口，无法直观看到当前 Token 实际拥有的完整权限集合。

## 提议方案 (Proposed Solution)

建议开发者平台提供一个专用接口，根据调用方的 `AppKey` 和 `accessToken` 动态返回其拥有权限的 OpenAPI 3.0.1 规范文档。

### 接口规范 (API Specification)

- **接口地址**: `GET /v1/common/auth/selfBuiltApp/getPermittedSpec`
- **功能描述**: 返回一个完整的 OpenAPI 3.0.1 格式的 JSON，其中仅包含调用方已获授权访问的路径（Paths）和模型（Schemas）。

#### 请求头 (Request Headers)

| 参数名 | 必选 | 描述 |
| :--- | :--- | :--- |
| `appKey` | 是 | 开发者控制台获取的 AppKey。 |
| `accessToken` | 是 | 通过 `generateToken` 接口获取的有效访问令牌。 |

#### 响应格式 (Response Format)

响应应为标准标准的 OpenAPI 3.0.1 JSON 对象。

```json
{
  "openapi": "3.0.1",
  "info": {
    "title": "畅捷通授权接口文档",
    "version": "1.0.0"
  },
  "paths": {
    "/accounting/openapi/cc/book/findByEnterpriseId": {
      "get": {
        "summary": "查询账套列表",
        "parameters": [...],
        "responses": {...}
      }
    }
  },
  "components": {
    "schemas": {...}
  }
}
```

#### 核心过滤逻辑 (Filtering Logic)

为了确保接口的精准度，平台侧在生成 OpenAPI 规范时，**必须根据当前自建应用所依赖的主应用环境进行二次过滤**。
- **背景**：一个 AppKey 可能在不同环境下对应不同的主应用（如：产研测试环境可能是“三好业财”，而另一个环境可能是“T+”）。
- **要求**：平台应识别当前 Token 绑定的业务上下文。如果当前是“好业财”环境，则返回的 Spec 中应仅包含“好业财”相关的 OpenApi；禁止混入 T+ 或其他不相关主应用的接口定义。

## 核心价值 (Value Proposition)

- **对 CLI 用户**：无需更新二进制文件，即可即时访问和查看平台上线的新接口。
- **对开发者**：CLI 变成了“活”的在线文档工具，降低了查阅静态文档和咨询接口规范的成本。
- **对平台团队**：实现服务端驱动的接口治理，CLI 自然遵循服务端定义的权限策略。

## CLI 侧集成逻辑 (Implementation on CLI)

CLI (`cjtcr`) 已经准备好集成此接口：
1. **自动抓取**：在本地缓存失效（1 小时 TTL）时，CLI 会主动调用此新接口。
2. **本地存储**：结果将保存为 `~/.cjtc/{profile}_openapi.json`。
3. **稳健降级**：若接口返回 404 或超时，cli记录错误日志，如果是在交互页面可以给出用户错误警告。
