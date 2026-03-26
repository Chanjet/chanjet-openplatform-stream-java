package cjtCli

import (
	"cjtCli/internal/core/telemetry"

	"github.com/spf13/cobra"
)

var authCmd = &cobra.Command{
	Use:   "auth",
	Short: "Manage authentication and credentials",
}

var authStatusCmd = &cobra.Command{
	Use:   "status",
	Short: "Check current authentication status",
	Run: func(cmd *cobra.Command, args []string) {
		conf := cfgMgr.Get()
		
		status := "READY"
		var missing []string
		if conf.AppKey == "" {
			status = "MISSING_CONFIG"
			missing = append(missing, "app_key")
		}
		if conf.Certificate == "" {
			missing = append(missing, "certificate")
		}
		
		secret, err := vlt.Get(profile, "app_secret")
		if err != nil || secret == "" {
			status = "MISSING_SECRET"
			missing = append(missing, "app_secret")
		}

		res := map[string]interface{}{
			"profile":  profile,
			"status":   status,
			"missing":  missing,
			"app_key":  conf.AppKey,
			"app_mode": conf.AppMode,
		}
		telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
	},
}

var authResetCmd = &cobra.Command{
	Use:   "reset",
	Short: "Reset all credentials and configuration for the current profile",
	Run: func(cmd *cobra.Command, args []string) {
		// Clear Vault
		vlt.Delete(profile, "app_secret")
		vlt.Delete(profile, "app_ticket")
		vlt.Delete(profile, "access_token")

		// Clear Config
		conf := cfgMgr.Get()
		conf.AppKey = ""
		conf.Certificate = ""
		conf.WebhookTarget = ""
		cfgMgr.Save(profile)

		res := map[string]string{
			"profile": profile,
			"status":  "reset",
			"message": "All credentials and configuration for this profile have been cleared.",
		}
		telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
	},
}

func init() {
	authCmd.AddCommand(authStatusCmd)
	authCmd.AddCommand(authResetCmd)
	rootCmd.AddCommand(authCmd)
}
