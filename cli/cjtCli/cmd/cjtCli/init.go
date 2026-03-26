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
	Short: "初始化应用配置与安全凭据",
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
		// conf.Certificate and conf.EncryptKey will be filled by Vault in root PreRun
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
		if err := vlt.Set(profile, "certificate", initCertificate); err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}
		if initEncryptKey != "" {
			if err := vlt.Set(profile, "encrypt_key", initEncryptKey); err != nil {
				telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
				return
			}
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
	initCmd.Flags().StringVar(&initAppKey, "app-key", "", "开放平台 AppKey")
	initCmd.Flags().StringVar(&initAppSecret, "app-secret", "", "开放平台 AppSecret (将被安全加密存储)")
	initCmd.Flags().StringVar(&initCertificate, "certificate", "", "自建应用证书 (Certificate)")
	initCmd.Flags().StringVar(&initEncryptKey, "encrypt-key", "", "消息加解密密钥 (AES Encrypt Key)")
	initCmd.Flags().StringVar(&initWebhook, "webhook-target", "", "本地 Webhook 接收地址")
	initCmd.Flags().StringVar(&initOpenApiURL, "openapi-url", "", "OpenAPI 基础 URL 覆盖")
	initCmd.Flags().StringVar(&initStreamURL, "stream-url", "", "Stream Gateway 基础 URL 覆盖")
	rootCmd.AddCommand(initCmd)
}
