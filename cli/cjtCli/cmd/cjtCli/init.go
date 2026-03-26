package cjtCli

import (
	"cjtCli/internal/core/telemetry"
	"cjtCli/internal/core/ui"
	"fmt"

	"github.com/spf13/cobra"
)

var (
	initAppKey      string
	initAppSecret   string
	initCertificate string
	initWebhook     string
	initOpenApiURL  string
	initStreamURL   string
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Initialize application configuration and credentials",
	Run: func(cmd *cobra.Command, args []string) {
		// 0. Pre-site construction confirmation (PRD v0.1.1)
		if format == "text" { // Only show interactive prompts in text mode
			confirmed := ui.Confirm("是否已于开放平台后台创建自建应用？")
			if !confirmed {
				ui.Info("创建与取参新手指南", "https://open.chanjet.com/docs/guide/create-app")
				return
			}
		}

		if initAppKey == "" || initAppSecret == "" || initCertificate == "" {
			fmt.Println("Error: --app-key, --app-secret, and --certificate are required for init.")
			return
		}

		// 1. Create empty config file if not exists
		if err := cfgMgr.CreateEmpty(profile); err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		// 2. Update config and Vault
		conf := cfgMgr.Get()
		conf.AppKey = initAppKey
		conf.Certificate = initCertificate
		conf.WebhookTarget = initWebhook
		if initOpenApiURL != "" {
			conf.OpenApiURL = initOpenApiURL
		}
		if initStreamURL != "" {
			conf.StreamURL = initStreamURL
		}

		if err := vlt.Set(profile, "app_secret", initAppSecret); err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		// 3. Save config
		if err := cfgMgr.Save(profile); err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		res := map[string]string{
			"profile": profile,
			"status":  "initialized",
			"message": "Configuration and secrets saved successfully.",
		}
		telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
	},
}

func init() {
	initCmd.Flags().StringVar(&initAppKey, "app-key", "", "Application Key")
	initCmd.Flags().StringVar(&initAppSecret, "app-secret", "", "Application Secret")
	initCmd.Flags().StringVar(&initCertificate, "certificate", "", "Self-built Application Certificate")
	initCmd.Flags().StringVar(&initWebhook, "webhook-target", "", "Local Webhook Target URL")
	initCmd.Flags().StringVar(&initOpenApiURL, "openapi-url", "", "OpenAPI Base URL")
	initCmd.Flags().StringVar(&initStreamURL, "stream-url", "", "Stream Gateway Base URL")
	rootCmd.AddCommand(initCmd)
}
