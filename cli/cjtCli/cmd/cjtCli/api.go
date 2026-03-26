package cjtCli

import (
	"bytes"
	"cjtCli/internal/core/telemetry"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"

	"github.com/spf13/cobra"
)

var (
	apiData []string
)

var apiCmd = &cobra.Command{
	Use:   "api <METHOD> <PATH>",
	Short: "Invoke a Chanjet Openplatform API with automatic authentication",
	Args:  cobra.ExactArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		method := strings.ToUpper(args[0])
		path := args[1]
		conf := cfgMgr.Get()

		// 1. Get Token
		token, err := authCli.GetAppAccessToken(profile, conf)
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		// 2. Build Request
		url := conf.AuthURL + path
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
		
		// 4. Output
		var result interface{}
		if err := json.Unmarshal(respBody, &result); err != nil {
			// If not JSON, return as raw string
			result = string(respBody)
		}

		if resp.StatusCode < 200 || resp.StatusCode >= 300 {
			telemetry.FormatOutput(result, fmt.Errorf("API error: %d", resp.StatusCode), telemetry.OutputFormat(format))
		} else {
			telemetry.FormatOutput(result, nil, telemetry.OutputFormat(format))
		}
	},
}

var apiListCmd = &cobra.Command{
	Use:   "list",
	Short: "List available APIs from Chanjet Openplatform",
	Run: func(cmd *cobra.Command, args []string) {
		conf := cfgMgr.Get()
		// api list 同样获取全量 Spec，但在展示时仅提取列表
		res, err := authCli.GetOpenApiSpec(profile, conf)
		telemetry.FormatOutput(res, err, telemetry.OutputFormat(format))
	},
}

var apiSpecCmd = &cobra.Command{
	Use:   "spec",
	Short: "Get OpenAPI 3.0 specification for the current application",
	Run: func(cmd *cobra.Command, args []string) {
		conf := cfgMgr.Get()
		res, err := authCli.GetOpenApiSpec(profile, conf)
		telemetry.FormatOutput(res, err, telemetry.OutputFormat(format))
	},
}

func init() {
	apiCmd.Flags().StringSliceVarP(&apiData, "data", "d", []string{}, "HTTP request body (can be specified multiple times)")
	apiCmd.AddCommand(apiListCmd)
	apiCmd.AddCommand(apiSpecCmd)
	rootCmd.AddCommand(apiCmd)
}
