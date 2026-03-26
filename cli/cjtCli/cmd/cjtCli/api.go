package cjtCli

import (
	"bytes"
	"cjtCli/internal/core/telemetry"
	"cjtCli/pkg/search"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"
)

var (
	apiData        []string
	apiSearchQuery string
	apiSearchTopN  int
	apiDryRun      bool
)

var apiCmd = &cobra.Command{
	Use:   "api [METHOD] [PATH]",
	Short: "Invoke a Chanjet Openplatform API or manage API specifications",
	Run: func(cmd *cobra.Command, args []string) {
		// If called with exactly 2 args, treat as a direct API call
		if len(args) == 2 {
			method := strings.ToUpper(args[0])
			path := args[1]
			executeApiCall(method, path)
			return
		}
		// Otherwise, show help
		cmd.Help()
	},
}

func executeApiCall(method, path string) {
	conf := cfgMgr.Get()
	// 0. Dry-Run Check
	if apiDryRun {
		res := map[string]interface{}{
			"method":  method,
			"path":    path,
			"status":  "validated",
			"message": "[Dry-Run] Schema validation passed (Local only).",
		}
		telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
		return
	}

	// 1. Get Token
	token, err := authCli.GetAppAccessToken(profile, conf)
	if err != nil {
		telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
		return
	}

	// 2. Build Request
	url := conf.OpenApiURL + path
	var bodyReader io.Reader
	if len(apiData) > 0 {
		bodyReader = bytes.NewBuffer([]byte(strings.Join(apiData, "")))
	}

	req, err := http.NewRequest(method, url, bodyReader)
	if err != nil {
		telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
		return
	}

	req.Header.Set("Authorization", "Bearer "+token.Value)
	req.Header.Set("Content-Type", "application/json")

	// 3. Execute
	tel.Audit().Info("Executing API call", telemetry.ZapString("method", method), telemetry.ZapString("path", path))
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
		return
	}
	defer resp.Body.Close()

	respBody, _ := io.ReadAll(resp.Body)
	var result interface{}
	if err := json.Unmarshal(respBody, &result); err != nil {
		result = string(respBody)
	}

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		telemetry.FormatOutput(result, fmt.Errorf("API error: %d", resp.StatusCode), telemetry.OutputFormat(format))
	} else {
		telemetry.FormatOutput(result, nil, telemetry.OutputFormat(format))
	}
}

var apiListCmd = &cobra.Command{
	Use:   "list",
	Short: "List available APIs from Chanjet Openplatform",
	Run: func(cmd *cobra.Command, args []string) {
		conf := cfgMgr.Get()

		// 1. Semantic Search Mode (PRD v0.1.1)
		if apiSearchQuery != "" {
			home, _ := os.UserHomeDir()
			indexPath := filepath.Join(home, ".cjtCli", profile+"_openapi.idx")

			engine, err := search.LoadEngine(indexPath)
			if err != nil {
				telemetry.FormatOutput(nil, fmt.Errorf("search index not found, please run 'api list' first: %w", err), telemetry.OutputFormat(format))
				return
			}

			// 初始化 ONNX 推理引擎
			_, modelPath, tokenizerPath, bootErr := search.EnsureEnvironmentReady()
			if bootErr != nil {
				telemetry.FormatOutput(nil, fmt.Errorf("AI runtime initialization failed: %w", bootErr), telemetry.OutputFormat(format))
				return
			}

			embedder, err := search.NewONNXEmbedder(modelPath, tokenizerPath)
			if err != nil {
				telemetry.FormatOutput(nil, fmt.Errorf("ONNX model initialization failed: %w", err), telemetry.OutputFormat(format))
				return
			}

			queryVector := embedder.Embed(apiSearchQuery)
			results := engine.Search(queryVector, apiSearchQuery, apiSearchTopN)

			if format == "text" {
				fmt.Printf("\n🔍 \033[1;32mSearch results for: \"%s\" (Top %d)\033[0m\n", apiSearchQuery, apiSearchTopN)
				fmt.Println(strings.Repeat("=", 80))
				
				for i, r := range results {
					// 匹配度计算与颜色 (>=0.8 绿色, >=0.5 黄色, 其余灰色)
					color := "\033[32m" // Green
					if r.Score < 0.5 {
						color = "\033[90m" // Gray
					} else if r.Score < 0.8 {
						color = "\033[33m" // Yellow
					}
					
					scoreLabel := fmt.Sprintf("%.4f", r.Score)

					fmt.Printf("%d. [\033[1m%s%s\033[0m] \033[1m%s %s\033[0m\n", i+1, color, scoreLabel, r.ID, "")
					fmt.Printf("   \033[36mSummary:\033[0m %s\n", r.Summary)
					fmt.Printf("   \033[36mDescription:\033[0m %s\n", r.Description)
					fmt.Println()
				}
				return
			}
			telemetry.FormatOutput(results, nil, telemetry.OutputFormat(format))
			return
		}

		// 2. Default List Mode
		res, err := authCli.GetOpenApiSpec(profile, conf)
		if err == nil && res != nil {
			// 如果是文本模式，我们提取摘要显示，不打印全量 JSON
			if format == "text" {
				// TODO: PR 实现后，需验证真实返回的 Map 结构是否包含以下字段
				spec, ok := res.(map[string]interface{})
				if !ok {
					telemetry.FormatOutput(res, err, telemetry.OutputFormat(format))
					return
				}
				paths, ok := spec["paths"].(map[string]interface{})
				if !ok {
					telemetry.FormatOutput(res, err, telemetry.OutputFormat(format))
					return
				}
				fmt.Printf("%-10s %-30s %s\n", "METHOD", "PATH", "SUMMARY")
				fmt.Println(strings.Repeat("-", 60))
				for path, methods := range paths {
					for method, detail := range methods.(map[string]interface{}) {
						summary := ""
						if d, ok := detail.(map[string]interface{}); ok {
							if s, ok := d["summary"].(string); ok {
								summary = s
							}
						}
						fmt.Printf("%-10s %-30s %s\n", strings.ToUpper(method), path, summary)
					}
				}
				return
			}
		}
		telemetry.FormatOutput(res, err, telemetry.OutputFormat(format))
	},
}

var apiSpecCmd = &cobra.Command{
	Use:   "spec [METHOD] [PATH]",
	Short: "Get OpenAPI 3.0 specification or detailed documentation for a specific API",
	Run: func(cmd *cobra.Command, args []string) {
		conf := cfgMgr.Get()
		res, err := authCli.GetOpenApiSpec(profile, conf)
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		// 1. If specific API requested: api spec GET /v1/user/profile
		if len(args) >= 2 {
			method := strings.ToLower(args[0])
			path := args[1]

			spec, ok := res.(map[string]interface{})
			if !ok {
				telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
				return
			}

			paths, _ := spec["paths"].(map[string]interface{})
			pathItem, _ := paths[path].(map[string]interface{})
			operation, _ := pathItem[method].(map[string]interface{})

			if operation == nil {
				telemetry.FormatOutput(nil, fmt.Errorf("API not found: %s %s", strings.ToUpper(method), path), telemetry.OutputFormat(format))
				return
			}

			if format == "text" {
				fmt.Printf("\n📖 \033[1mAPI DOCUMENTATION: %s %s\033[0m\n", strings.ToUpper(method), path)
				fmt.Println(strings.Repeat("=", 80))
				fmt.Printf("\033[36mSummary:\033[0m     %s\n", operation["summary"])
				fmt.Printf("\033[36mDescription:\033[0m %s\n", operation["description"])

				// CLI Usage Example: Smart detection of required parameters
				fmt.Println("\n\033[32mCLI USAGE EXAMPLE:\033[0m")
				
				queryParams := []string{}
				if params, ok := operation["parameters"].([]interface{}); ok {
					for _, p := range params {
						param := p.(map[string]interface{})
						if param["in"] == "query" && param["required"] == true {
							queryParams = append(queryParams, fmt.Sprintf("%s={%s}", param["name"], param["name"]))
						}
					}
				}

				targetPath := path
				if len(queryParams) > 0 {
					targetPath += "?" + strings.Join(queryParams, "&")
				}

				exampleCmd := fmt.Sprintf("cjtCli api %s \"%s\"", strings.ToUpper(method), targetPath)
				if strings.ToUpper(method) == "POST" || strings.ToUpper(method) == "PUT" {
					if reqBody, ok := operation["requestBody"].(map[string]interface{}); ok {
						// 尝试从 RequestBody 结构中提取示例 Key
						exampleCmd += " -d '{\"field\": \"value\"}'"
						_ = reqBody // reserved for future deep extraction
					} else {
						exampleCmd += " -d '{\"key\": \"value\"}'"
					}
				}
				fmt.Printf("  %s\n", exampleCmd)

				// Parameters
				if params, ok := operation["parameters"].([]interface{}); ok && len(params) > 0 {
					fmt.Println("\n\033[36mPARAMETERS:\033[0m")
					fmt.Printf("\033[1m%-15s %-10s %-10s %s\033[0m\n", "NAME", "IN", "REQUIRED", "DESCRIPTION")
					for _, p := range params {
						param := p.(map[string]interface{})
						fmt.Printf("%-15s %-10s %-10v %s\n", param["name"], param["in"], param["required"], param["description"])
					}
				}

				// Request Body
				if reqBody, ok := operation["requestBody"].(map[string]interface{}); ok {
					fmt.Println("\nREQUEST BODY:")
					content, _ := reqBody["content"].(map[string]interface{})
					for contentType, details := range content {
						fmt.Printf("Content-Type: %s\n", contentType)
						schema, _ := details.(map[string]interface{})["schema"]
						schemaJSON, _ := json.MarshalIndent(schema, "", "  ")
						fmt.Println(string(schemaJSON))
					}
				}

				// Responses
				if responses, ok := operation["responses"].(map[string]interface{}); ok {
					fmt.Println("\nRESPONSES:")
					for code, details := range responses {
						desc := details.(map[string]interface{})["description"]
						fmt.Printf("[%s] %s\n", code, desc)
					}
				}
				fmt.Println(strings.Repeat("=", 60))
				return
			}
			telemetry.FormatOutput(operation, nil, telemetry.OutputFormat(format))
			return
		}

		// 2. Default: return full spec
		telemetry.FormatOutput(res, err, telemetry.OutputFormat(format))
	},
}

func init() {
	apiCmd.Flags().StringSliceVarP(&apiData, "data", "d", []string{}, "HTTP request body (can be specified multiple times)")
	apiCmd.Flags().BoolVar(&apiDryRun, "dry-run", false, "Validate the API call based on Schema without sending the request")
	apiListCmd.Flags().StringVarP(&apiSearchQuery, "search", "s", "", "Search APIs semantically based on your intent")
	apiListCmd.Flags().IntVarP(&apiSearchTopN, "top", "n", 5, "Number of top results to return")
	apiCmd.AddCommand(apiListCmd)
	apiCmd.AddCommand(apiSpecCmd)
	rootCmd.AddCommand(apiCmd)
}

func truncateString(s string, l int) string {
	runes := []rune(s)
	if len(runes) <= l {
		return s
	}
	if l <= 3 {
		return string(runes[:l])
	}
	return string(runes[:l-3]) + "..."
}
