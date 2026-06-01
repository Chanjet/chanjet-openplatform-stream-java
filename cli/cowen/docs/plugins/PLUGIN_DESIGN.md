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
*   **路由代理透传**：
    1.  当接收到调用方（如 MCP Agent）的 `tools/list` 请求时，插件直接向宿主发起 `GET /v1/plugin/registry` 或 `GET /v1/mcp/tools` 请求，并将返回的动态工具注册表响应给 Agent。
    2.  当接收到 `tools/call` 请求时，插件作为无状态的空壳，将 `tool_name` 和 `arguments` 打包成 HTTP 请求发往宿主的本地代理接口。宿主返回结果后，插件再原样通过 JSON-RPC 抛回给调用方。

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

	req, _ := http.NewRequest("POST", apiEndpoint+"/v1/mcp/tools/call", bytes.NewBuffer(body))
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

虽然 Go 插件代码被简化为“空壳”，但它需要在其同级目录的 `plugin.json` 中向宿主**“声明”**它的属性。

### 3.1 基础与运行声明
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
*   `"tenant_mode": "shared"`（Go 语言极其推荐）：声明该插件是轻量级、无状态的。宿主在全局仅维持该 Go 进程的一个实例，利用 Goroutine 的高并发处理多租户请求。插件请求宿主时携带显式的租户路径（如 `/v1/tenant_a/mcp/tools/call`），由宿主核心网关动态审查越权行为。
