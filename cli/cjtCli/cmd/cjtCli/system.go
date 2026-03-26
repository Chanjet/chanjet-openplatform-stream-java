package cjtCli

import (
	"cjtCli/internal/core/telemetry"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"
)

var (
	resetCmd = &cobra.Command{
		Use:   "reset",
		Short: "Reset the current profile state",
		Run: func(cmd *cobra.Command, args []string) {
			// Clear config
			if err := cfgMgr.Delete(profile); err != nil {
				telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
				return
			}
			// Clear secrets
			vlt.Delete(profile, "app_secret")
			
			res := map[string]string{
				"profile": profile,
				"status":  "reset",
				"message": "Configuration and secrets cleared.",
			}
			telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
		},
	}

	configCmd = &cobra.Command{
		Use:   "config",
		Short: "Inspect the current configuration",
		Run: func(cmd *cobra.Command, args []string) {
			conf := cfgMgr.Get()
			if format == "text" {
				fmt.Printf("Profile: %s\n", profile)
				data, _ := json.MarshalIndent(conf, "", "  ")
				fmt.Println(string(data))
				return
			}
			telemetry.FormatOutput(conf, nil, telemetry.OutputFormat(format))
		},
	}

	logCmd = &cobra.Command{
		Use:   "log",
		Short: "Manage and view logs",
	}

	logListCmd = &cobra.Command{
		Use:   "list",
		Short: "List log files",
		Run: func(cmd *cobra.Command, args []string) {
			home, _ := os.UserHomeDir()
			logDir := filepath.Join(home, ".cjtCli", "log")
			files, err := os.ReadDir(logDir)
			if err != nil {
				telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
				return
			}

			type logInfo struct {
				Name string `json:"name"`
				Size int64  `json:"size_bytes"`
			}
			var res []logInfo
			for _, f := range files {
				info, _ := f.Info()
				res = append(res, logInfo{Name: f.Name(), Size: info.Size()})
			}

			if format == "text" {
				fmt.Printf("%-20s %s\n", "NAME", "SIZE")
				fmt.Println(strings.Repeat("-", 30))
				for _, l := range res {
					fmt.Printf("%-20s %d B\n", l.Name, l.Size)
				}
				return
			}
			telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
		},
	}

	statusCmd = &cobra.Command{
		Use:   "status",
		Short: "Check the status of the daemon and profiles",
		Run: func(cmd *cobra.Command, args []string) {
			conf := cfgMgr.Get()

			// Auth status summary
			authStatus := "READY"
			var missing []string
			if conf.AppKey == "" {
				authStatus = "MISSING_CONFIG"
				missing = append(missing, "app_key")
			}
			secret, _ := vlt.Get(profile, "app_secret")
			if secret == "" {
				authStatus = "MISSING_SECRET"
				missing = append(missing, "app_secret")
			}

			res := map[string]interface{}{
				"profile": profile,
				"status":  authStatus,
				"missing": missing,
				"app_key": conf.AppKey,
				"daemon":  "running (use 'daemon start' to ensure up-to-date)",
			}
			telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
		},
	}

	checkUpdateCmd = &cobra.Command{
		Use:   "check-update",
		Short: "Check for CLI updates",
		Run: func(cmd *cobra.Command, args []string) {
			// Implementation
		},
	}
)

var (
	logDomain string
	logLines  int
)

var logViewCmd = &cobra.Command{
	Use:   "view",
	Short: "View logs in real-time with formatting",
	Run: func(cmd *cobra.Command, args []string) {
		home, _ := os.UserHomeDir()
		logPath := filepath.Join(home, ".cjtCli", "log", logDomain+".log")

		data, err := os.ReadFile(logPath)
		if err != nil {
			telemetry.FormatOutput(nil, fmt.Errorf("failed to read log %s: %w", logPath, err), telemetry.OutputFormat(format))
			return
		}

		lines := strings.Split(strings.TrimSpace(string(data)), "\n")
		if len(lines) > logLines {
			lines = lines[len(lines)-logLines:]
		}

		if format == "text" {
			fmt.Printf("\n📄 \033[1mViewing Log: %s (%d lines)\033[0m\n", logPath, len(lines))
			fmt.Println(strings.Repeat("=", 100))

			if logDomain == "audit" {
				fmt.Printf("\033[1m%-25s %-10s %-8s %-40s\033[0m\n", "TIME", "METHOD", "STATUS", "PATH")
				fmt.Println(strings.Repeat("-", 100))
			}

			for _, line := range lines {
				var entry map[string]interface{}
				if err := json.Unmarshal([]byte(line), &entry); err != nil {
					fmt.Println(line)
					continue
				}

				if logDomain == "audit" {
					ts := entry["ts"].(string)
					method := entry["method"].(string)
					path := entry["path"].(string)
					status := "-"
					if s, ok := entry["status"].(float64); ok {
						status = fmt.Sprintf("%d", int(s))
					}
					
					// 转换 ISO 时间为更易读格式
					if len(ts) > 19 {
						ts = ts[5:19] // MM-DD HH:MM:SS
					}

					mColor := "\033[32m" // Green for GET
					if method == "POST" || method == "PUT" {
						mColor = "\033[33m" // Yellow
					} else if method == "DELETE" {
						mColor = "\033[31m" // Red
					}

					sColor := "\033[32m" // Green for 2xx
					if status != "-" {
						sVal := 0
						fmt.Sscanf(status, "%d", &sVal)
						if sVal < 200 || sVal >= 300 {
							sColor = "\033[31m" // Red for error
						}
					}

					fmt.Printf("%-25s %s%-10s\033[0m %s%-8s\033[0m %s\n", ts, mColor, method, sColor, status, path)
				} else {
					// 其他日志输出
					lvl := entry["level"].(string)
					msg := entry["msg"].(string)
					fmt.Printf("[%s] %s\n", strings.ToUpper(lvl), msg)
				}
			}
			fmt.Println()
			return
		}

		// Non-text format
		var parsedLines []map[string]interface{}
		for _, line := range lines {
			var entry map[string]interface{}
			_ = json.Unmarshal([]byte(line), &entry)
			parsedLines = append(parsedLines, entry)
		}
		telemetry.FormatOutput(parsedLines, nil, telemetry.OutputFormat(format))
	},
}

func init() {
	logViewCmd.Flags().StringVarP(&logDomain, "domain", "d", "audit", "Log domain (audit, sys, stream, dlq)")
	logViewCmd.Flags().IntVarP(&logLines, "lines", "n", 20, "Number of last lines to show")
	logCmd.AddCommand(logListCmd)
	logCmd.AddCommand(logViewCmd)
	rootCmd.AddCommand(resetCmd)
	rootCmd.AddCommand(configCmd)
	rootCmd.AddCommand(logCmd)
	rootCmd.AddCommand(statusCmd)
	rootCmd.AddCommand(checkUpdateCmd)
}
