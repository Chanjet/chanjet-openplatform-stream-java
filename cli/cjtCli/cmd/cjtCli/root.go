package cjtCli

import (
	"cjtCli/internal/auth"
	"cjtCli/internal/core/config"
	"cjtCli/internal/core/security"
	"cjtCli/internal/core/telemetry"
	"cjtCli/internal/core/vault"
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"
)

var (
	profile  string
	format   string
	logLevel string

	cfgMgr    config.Manager
	vlt       vault.Vault
	tel       *telemetry.Telemetry
	tokenPool auth.TokenPool
	barrier   auth.Barrier
	authCli   auth.Client
)

var rootCmd = &cobra.Command{
	Use:   "cjtCli",
	Short: "畅捷通开放平台 Stream Connector 命令行工具 (CLI)",
	Long: `畅捷通开放平台官方 CLI 治理工具。

核心能力 (Core Capabilities):
- 🔍 语义搜索 (api list --search): 基于 NLP 实现企业级 API 的智能检索与意向发现。
- 🛡️ 自动鉴权 (auth/init): 自动化托管 AppTicket 与 AccessToken 周期，无需手动刷新。
- 📦 接口调试 (api execute): 支持声明式 API 调用，自动注入安全头并实时审计。
- 🛠️ 系统治理 (system/log): 全面的日志追踪、状态监控与 Vault 安全存储管理。`,
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		// Initialize Telemetry
		var err error
		tel, err = telemetry.NewTelemetry("", logLevel)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Failed to initialize telemetry: %v\n", err)
			os.Exit(1)
		}

		// Initialize Config
		cfgMgr = config.NewManager()
		if err := cfgMgr.Load(profile); err != nil {
			tel.Sys().Error("Failed to load config", telemetry.Err(err))
		}

		// Initialize Vault
		home, _ := os.UserHomeDir()
		sealPath := filepath.Join(home, ".cjtCli", ".seal")
		
		fingerprint, _ := security.GetMachineFingerprint()
		vlt, err = vault.NewVault("cjtCli", sealPath, fingerprint)
		if err != nil {
			tel.Sys().Error("Failed to initialize vault", telemetry.Err(err))
		}

		// Initialize Auth
		tokenPool = auth.NewTokenPool(vlt)
		barrier = auth.NewBarrier()
		authCli = auth.NewClient(tokenPool, barrier, tel)
		
		// Fill AppSecret and other sensitive keys from Vault if exists
		conf := cfgMgr.Get()
		if secret, err := vlt.Get(profile, "app_secret"); err == nil {
			conf.AppSecret = secret
		}
		if cert, err := vlt.Get(profile, "certificate"); err == nil {
			conf.Certificate = cert
		}
		if ekey, err := vlt.Get(profile, "encrypt_key"); err == nil {
			conf.EncryptKey = ekey
		}

		// Auth Integrity Check (Skip for init, system, and help commands)
		if isGuarded(cmd) {
			_, err := authCli.GetAppAccessToken(profile, conf)
			if err != nil {
				res := map[string]interface{}{
					"profile": profile,
					"status":  "UNAUTHORIZED",
					"error":   fmt.Sprintf("Profile is not properly initialized or credentials expired: %v", err),
					"hint":    "Please run 'cjtCli init' to configure your credentials.",
				}
				telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))
				os.Exit(1)
			}
		}
	},
}

func isGuarded(cmd *cobra.Command) bool {
	// Skip for specific base commands
	skipCommands := map[string]bool{
		"init":   true,
		"system": true,
		"help":   true,
		"auth":   true,   // Allow auth subcommands to handle their own state (e.g., status/reset)
		"daemon": true,   // Allow daemon to start without token (to receive AppTicket)
		"status": true,   // Allow global status check
	}

	for c := cmd; c != nil; c = c.Parent() {
		if skipCommands[c.Name()] {
			return false
		}
	}
	return true
}

func Execute() {
	// Global Panic Recovery
	defer telemetry.Recover(telemetry.OutputFormat(format))

	if err := rootCmd.Execute(); err != nil {
		telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
		os.Exit(1)
	}
}

func init() {
	rootCmd.PersistentFlags().StringVar(&profile, "profile", "default", "配置环境名称 (default/inte/prod)")
	rootCmd.PersistentFlags().StringVar(&format, "format", "text", "输出格式 (text, json, yaml)")
	rootCmd.PersistentFlags().StringVar(&logLevel, "log-level", "info", "日志输出级别 (debug, info, warn, error)")
}
