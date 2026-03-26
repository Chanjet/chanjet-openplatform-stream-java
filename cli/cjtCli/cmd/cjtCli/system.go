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
			// In a real impl, we'd check for a PID file or socket
			res := map[string]interface{}{
				"profile": profile,
				"status":  "active",
				"daemon":  "unknown (check log/sys.log)",
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

func init() {
	logCmd.AddCommand(logListCmd)
	rootCmd.AddCommand(resetCmd)
	rootCmd.AddCommand(configCmd)
	rootCmd.AddCommand(logCmd)
	rootCmd.AddCommand(statusCmd)
	rootCmd.AddCommand(checkUpdateCmd)
}
