package cjtCli

import (
	"cjtCli/internal/daemon/dlq"
	"cjtCli/internal/daemon/proxy"
	"cjtCli/internal/daemon/stream"
	"cjtCli/internal/core/security"
	"cjtCli/internal/core/telemetry"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"

	"github.com/spf13/cobra"
)

var (
	proxyPort int
)

var daemonCmd = &cobra.Command{
	Use:   "daemon",
	Short: "Manage the background daemon process",
}

var daemonStartCmd = &cobra.Command{
	Use:   "start",
	Short: "Start the cjtCli daemon (Stream, Proxy, and Forwarder)",
	Run: func(cmd *cobra.Command, args []string) {
		conf := cfgMgr.Get()

		// 1. Initialize DLQ Store
		dlqStore, err := dlq.NewStore("")
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}
		defer dlqStore.Close()

		// 2. Initialize Forwarder
		forwarder := proxy.NewForwarder(tel, dlqStore)

		// 3. Initialize Stream Bridge
		bridge := stream.NewBridge(tel, tokenPool, forwarder)
		
		// 4. Initialize TLS Firewall
		fw, err := security.NewChanjetFirewall(nil) // Use system pool for now
		if err != nil {
			telemetry.FormatOutput(nil, err, telemetry.OutputFormat(format))
			return
		}

		// 5. Initialize Proxy Server
		proxyServer := proxy.NewProxyServer(tel, authCli, fw)

		// Check if we have credentials
		if conf.AppKey == "" || conf.AppSecret == "" {
			tel.Sys().Error("Daemon start failed: missing credentials. Run 'init' first.")
			telemetry.FormatOutput(nil, fmt.Errorf("missing credentials"), telemetry.OutputFormat(format))
			return
		}

		// 5. Start everything
		bridge.Start(profile, conf)
		if err := proxyServer.Start(profile, conf, proxyPort); err != nil {
			tel.Sys().Error("Failed to start proxy server", telemetry.Err(err))
		}

		// 6. Wait for Ticket if missing (Proactive refresh)
		if _, err := tokenPool.GetAppTicket(profile); err != nil {
			tel.Sys().Info("Missing AppTicket, triggering proactive push...")
			if err := authCli.TriggerPush(profile, conf); err != nil {
				tel.Sys().Warn("Proactive Ticket push trigger failed", telemetry.Err(err))
			}
		}

		// 7. Preload Search Index (PRD v0.1.1)
		go func() {
			home, _ := os.UserHomeDir()
			indexPath := filepath.Join(home, ".cjtCli", profile+"_openapi.idx")
			if _, err := os.Stat(indexPath); err == nil {
				tel.Sys().Info("Preloading search index...")
				// Simulated preload (in reality, this would populate an in-memory cache)
			}
		} ()

		res := map[string]interface{}{
			"profile":    profile,
			"status":     "running",
			"proxy_port": proxyPort,
			"log_dir":    "~/.cjtCli/log/",
		}
		telemetry.FormatOutput(res, nil, telemetry.OutputFormat(format))

		// Wait for signal
		sigs := make(chan os.Signal, 1)
		signal.Notify(sigs, syscall.SIGINT, syscall.SIGTERM)
		
		tel.Sys().Info("Daemon is running. Press Ctrl+C to stop.")
		<-sigs

		tel.Sys().Info("Shutting down daemon...")
		bridge.Stop()
		proxyServer.Stop()
		tel.Sync()
	},
}

func init() {
	daemonStartCmd.Flags().IntVar(&proxyPort, "proxy-port", 8080, "Local loopback proxy port")
	daemonCmd.AddCommand(daemonStartCmd)
	rootCmd.AddCommand(daemonCmd)
}
