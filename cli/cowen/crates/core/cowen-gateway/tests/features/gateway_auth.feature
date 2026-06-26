Feature: 代理拦截与鉴权
  Scenario: 缺少有效 Token 的请求应被拦截并返回 401
    Given 客户端未配置有效的 App Token
    And 网关已启动并监听在 "127.0.0.1:0"
    When 客户端向网关发送 "GET /api/v1/protected" 请求
    Then 网关应返回状态码 "401"

  Scenario: 携带 OAuth Code 的请求应完成交换并重定向
    Given 客户端配置了有效的 App Token
    And 网关已启动并监听在 "127.0.0.1:0"
    When 客户端向网关发送 "GET /api/v1/protected?code=123" 请求
    Then 网关应返回状态码 "302"

  Scenario: 访问 Bypass 路由应直接放行 (由于没有 upstream 返回 502)
    Given 客户端未配置有效的 App Token
    And 网关已启动并监听在 "127.0.0.1:0"
    When 客户端向网关发送 "GET /api/v1/health" 请求
    Then 网关应返回状态码 "502"
