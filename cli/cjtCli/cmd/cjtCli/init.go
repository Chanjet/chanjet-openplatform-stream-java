package cjtCli

import (
	"cjtCli/internal/core/telemetry"
	"fmt"

	"github.com/spf13/cobra"
)

var (
	initAppKey      string
	initAppSecret   string
	initCertificate string
	initEncryptKey  string
	initWebhook     string
	initOpenApiURL  string
	initStreamURL   string
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Initialize application configuration and credentials",
	Run: func(cmd *cobra.Command, args []string) {
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
		conf.EncryptKey = initEncryptKey
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
	initCmd.Flags().StringVar(&initEncryptKey, "encrypt-key", "", "Message Secret (AES Encrypt Key)")
	initCmd.Flags().StringVar(&initWebhook, "webhook-target", "", "Local Webhook Target URL")
	initCmd.Flags().StringVar(&initOpenApiURL, "openapi-url", "", "OpenAPI Base URL")
	initCmd.Flags().StringVar(&initStreamURL, "stream-url", "", "Stream Gateway Base URL")
	rootCmd.AddCommand(initCmd)
}
