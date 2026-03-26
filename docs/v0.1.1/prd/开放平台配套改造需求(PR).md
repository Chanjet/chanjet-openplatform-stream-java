# 开放平台配套改造需求 (PR) - v0.1.1

| 维度 | 内容 |
| --- | --- |
| **文档状态** | 正式版 (配合 cjtCli v0.1.1) |
| **生效版本** | v0.1.1 |
| **核心协作项** | OpenAPI 规范发现 |

---

## 1. 概述 (Overview)
为支持 `cjtCli` 的“动态 API 调用”与“语义化检索”能力，开放平台（Core）需要新增一个标准的元数据发现接口。该接口将赋能 CLI 实现无感鉴权、离线搜索及入参自动校验。

---

## 2. 平台侧新增需求 (New Requirements)

### 2.1 获取 OpenAPI 3.0 规范文件 (getOpenApiSpec)
提供该应用有权访问的完整 OpenAPI 规范文件。

- **接口路径**: `GET /metadata/v1/openapi/spec?app_key={AppKey}`
- **说明**: 
    - 返回标准完整的 OpenAPI 3.0+ JSON 规范文件。
    - 必须包含 `paths` 定义以及 `components/schemas` 数据模型。
- **响应体示例**:
  ```json
  {
    "result": true,
    "error": null,
    "value": {
      "openapi": "3.0.1",
      "info": { "title": "Chanjet Open API", "version": "1.0.0" },
      "paths": {
        "/v1/orders": {
          "get": { "summary": "查询订单", "responses": { "200": { "description": "OK" } } }
        }
      }
    }
  }
  ```

---

## 3. 安全与可用性约束
1. **鉴权要求**: 接口需通过 Header 校验 `appKey` 与 `openToken`（即 `accessToken`）。
2. **数据范围限制**: 开放平台返回的规范文件 SHALL **仅包含**当前 `appKey` 且由该 `openToken` 授权范围内可访问的接口定义。严禁下发未授权接口元数据。
3. **缓存建议**: 由于响应体较大，强烈建议支持 `ETag` 机制，cjtCli 会在本地缓存该规范文件以减少带宽消耗。


---
**更新日期**: 2026-03-26
