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
	Short: "Chanjet Openplatform Stream Connector CLI",
	Long:  `A high-reliability CLI tool for Chanjet Openplatform Stream Connector.`,
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
		
		// Fill AppSecret from Vault if exists
		conf := cfgMgr.Get()
		if secret, err := vlt.Get(profile, "app_secret"); err == nil {
			conf.AppSecret = secret
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
	rootCmd.PersistentFlags().StringVar(&profile, "profile", "default", "Configuration profile name")
	rootCmd.PersistentFlags().StringVar(&format, "format", "text", "Output format (text, json, yaml)")
	rootCmd.PersistentFlags().StringVar(&logLevel, "log-level", "info", "Log level (debug, info, warn, error)")
}
