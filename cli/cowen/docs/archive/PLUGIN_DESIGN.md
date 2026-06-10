# 三方多语言插件设计契约 (Plugin Design)

在 `cowen` 的通用插件宿主架构中，虽然宿主可以拉起任何语言的子进程，但我们**强烈推荐使用 Go 语言**来开发官方或标准的“空壳中继 (Generic Relay)”插件。

---

## 1. 为什么选择 Go (Why Go?)

在以“空壳中继”为核心的架构下，插件的业务逻辑极薄，主要作为标准输入输出 (Stdio) 的 JSON-RPC 翻译器。选用 Go 语言具有压倒性的部署优势：

*   **零环境依赖 (Zero Dependencies)**：Go 编译出的单一静态二进制文件可以直接挂载运行。无需强迫用户在本地电脑预装 Python 环境或 Node.js 运行时，极大提升了 `cowen` CLI 的开箱即用体验。
*   **毫秒级冷启动 (Instant Cold Start)**：宿主 `spawn` 拉起 Go 进程的开销在几毫秒内，完美契合随用随起、甚至频繁重载的多租户隔离场景。
*   **极低内存水位 (Low Memory Footprint)**：在共享模式（`shared`）下，Go 常驻进程占用的内存可以控制在 10MB 左右，对宿主系统的资源侵入极小。

---

## 2. 核心职责：渐进式空壳中继

插件开发者只需编写极少量的 Go 代码，负责 JSON-RPC 与宿主 RESTful API 之间的翻译：

*   **零业务硬编码**：插件中不硬编码具体工具（如 `github_create_issue`）的 URL 或处理逻辑。
*   **统一序列化约束 (RPC Serialization)**：不论底层采用 Dylib 还是独立进程沙箱，插件绝不直接操作宿主的内存结构。所有交互必须通过 JSON-RPC 或 Protobuf 等标准协议序列化后，经由 HTTP/RPC 协议交由宿主底层网关进行一致性鉴权。
*   **路由代理透传**：
    1.  当接收到调用方（如 MCP Agent）的 `tools/list` 请求时，插件直接向宿主发起 `GET /v1/api/registry` 请求，并将返回的动态工具注册表响应给 Agent。
    2.  当接收到 `tools/call` 请求时，插件作为无状态的空壳，将 `tool_name` 和 `arguments` 打包成 HTTP 请求发往宿主的本地代理接口。宿主返回结果后，插件再原样通过 JSON-RPC 抛回给调用方。
*   **严禁破坏标准输出 (Stdout)**：插件的主干道是 Stdio，任何调试日志或错误堆栈**绝对禁止**打印到标准输出（会导致 JSON-RPC 解析崩溃）。所有调试信息必须写入 `stderr`，由宿主统一截获并审计。

### 2.1 Go 语言空壳中继伪代码示例

```go
package main

import (
	"bytes"
	"encoding/json"
	"net/http"
	"os"
)

func main() {
	// 1. 一键读取宿主在拉起瞬间透传的安全环境变量
	apiEndpoint := os.Getenv("COWEN_API_ENDPOINT")
	bridgeToken := os.Getenv("COWEN_BRIDGE_TOKEN")

	// 2. 启动 JSON-RPC server 监听 Stdio (省略框架代码)
	// rpcServer.StartStdio()
}

// 处理 tools/call 的核心回调函数
func handleToolCall(toolName string, args map[string]interface{}) (interface{}, error) {
	// 组装发往宿主的纯粹 Intent (空壳转发)
	payload := map[string]interface{}{
		"tool_name": toolName,
		"arguments": args,
	}
	body, _ := json.Marshal(payload)

	req, _ := http.NewRequest("POST", apiEndpoint+"/v1/api/call", bytes.NewBuffer(body))
	req.Header.Set("X-Bridge-Token", bridgeToken)
	req.Header.Set("Content-Type", "application/json")

	// 宿主代为完成 URL 组装、鉴权计算和外网请求
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	// 将宿主返回的真实数据（或密文解密后的明文）透明抛回给 Agent
	var result interface{}
	json.NewDecoder(resp.Body).Decode(&result)
	return result, nil
}
```

---

## 3. 声明式自治理契约 (`plugin.json`)

虽然 Go 插件代码被简化为“空壳”，但它需要在其同级目录的 `plugin.json` 中向宿主**“声明”**它的属性。由宿主的 `system_orchestrator.rs` 在启动时扫描 `$COWEN_HOME/plugins/` (默认 `~/.cowen/plugins/`) 目录并装载。

> [!NOTE]
> 详细的概念边界、权限与能力配置的区别，请参考最新的 [插件清单规范 (Plugin Manifest Spec)](./PLUGIN_MANIFEST_SPEC.md)。


### 3.1 插件目录结构示例

```bash
$COWEN_HOME/plugins/
  ├─ cowen-go-relay/
  │   ├─ plugin.json
  │   └─ cowen-go-relay (编译好的二进制可执行文件)
```

### 3.2 基础与运行声明
```json
{
  "id": "cowen-go-relay",
  "name": "Universal Go Relay Plugin",
  "version": "1.0.0",
  // 指向编译好的 Go 二进制可执行文件
  "command": "./cowen-go-relay", 
  "default_config": {
    "log_level": "INFO"
  },
  "requested_permissions": {
    "allow_search_index": true
  }
}
```

### 3.2 动态 CLI 命令挂载契约
如果插件希望在原生的宿主 CLI 终端中也能被使用：
```json
  "cowen_extension": {
    "cli_commands": [
      {
        "name": "github-issue",
        "description": "向当前活跃的 GitHub 流连接器创建 issue",
        "args": [
          { "name": "title", "type": "string", "required": true }
        ]
      }
    ]
  }
```
宿主的 `Fallback Parser` 将依赖这些声明来进行参数的前置校验与物理进程拦截拉起。

### 3.3 租户隔离模式声明 (`tenant_mode`)
*   `"tenant_mode": "exclusive"`（默认）：宿主在切换不同的 Profile 时，会强制为该插件重新拉起独立的 Go 进程，避免内存中的租户上下文串流。
*   `"tenant_mode": "shared"`（Go 语言极其推荐）：声明该插件是轻量级、无状态的。宿主在全局仅维持该 Go 进程的一个实例，利用 Goroutine 的高并发处理多租户请求。插件请求宿主时携带显式的租户路径（如 `/v1/tenant_a/api/call`），由宿主核心网关动态审查越权行为。

### 3.4 细粒度能力依赖声明 (Capability-Based Contract)
为了避免宿主非兼容性升级导致历史插件无辜失效，彻底废弃了单体版本号（Monolithic Versioning）。插件必须在 `plugin.json` 中显式声明自身需要的底层网关能力矩阵：
```json
  "transport": "stdio",
  "required_capabilities": {
    "native.api.proxy": "^1.0.0",    // 可选：依赖本地 HTTP 代理时声明 (若宿主开启了 --no-proxy，此能力将不可用)
    "native.api.search": "v2"        // 可选：依赖内置搜索服务时声明
  }
```
宿主的 `PluginManager` 将在扫描阶段作为“能力适配漏斗”，如果宿主无法提供相应的能力和版本，将在加载前直接拒绝拉起该插件，从而 100% 防止运行时兼容性崩溃。特别地，当宿主关闭了本地代理（`proxy_enabled = false`）时，声明 `native.api.proxy` 的插件将被拒绝加载；插件若声明 `transport: "stdio"` 则可正常通过 RPC 通道调用宿主内部能力。

---

## 4. 声明式扩展点注入 (Declarative Contributions)

插件不仅可以作为被动的 MCP 工具提供者，还可以通过在 `plugin.json` 中定义 `contributes` 块，主动向 `cowen` 宿主的控制面（如 CLI、HTTP 服务器、定时调度器）注入自己的扩展能力。

### 4.1 多态扩展目标配置示例

```json
{
  "id": "my-monitor-plugin",
  "contributes": {
    "cli_commands": [
      { "name": "github-issue", "target_mcp_tool": "create_issue" }
    ],
    "http_routes": [
      {
        "port": "monitor",                 // 注入到宿主的监控端点组
        "path": "/v1/metrics/custom",
        "target": {
          "type": "http_tunnel",           // 【核心】HTTP 隧道协议透传
          "method": "cowen/http_tunnel"    // 映射到插件内部的 JSON-RPC 处理器
        }
      }
    ],
    "cron_jobs": [
      { "schedule": "*/5 * * * *", "target_mcp_tool": "health_check" }
    ]
  }
}
```

### 4.2 零端口沙箱与 HTTP 隧道透传 (HTTP Tunneling)

为捍卫插件隔离底线，本架构**严禁**插件在本地绑定任何 TCP 端口、Unix Domain Socket 或命名管道。
所有 `http_routes` 的扩展注入，必须使用 `http_tunnel` 多态网关代理：

1. **宿主反向代理**：外部 HTTP 流量由宿主统一在 `monitor_port` 等端口接管并执行鉴权拦截。
2. **协议坍缩**：宿主将 HTTP Header、Body 等信息全量序列化打包，转化为一条普通的 JSON-RPC 消息通过 **Stdio** 传递给插件。
3. **协议重组**：插件仅需按特定格式在 Stdio 吐出 JSON 结果，宿主负责将其“升维”为标准的 HTTP Response 返回给外网。

通过此设计，插件能在保持 100% 网络隔离与跨平台一致性的前提下，实现暴露富媒体网页、流式下载、SSE 等高级 Web 能力。
